use crate::airlock::Airlock;
use crate::pty_manager::PtyManager;
use crate::runner::Runner;
use crate::state_machine::StateMachine;
use crate::vault::Vault;

use anyhow::{Context, Result};
use positronic_hive::{HiveEvent, HiveNode};
use positronic_io::{HardwareEvent, HardwareMonitor};
use positronic_neural::cortex::NeuralClient;
use positronic_script::wasm_host::WasmHost;

use std::borrow::Cow;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

/// The main entry point for the Positronic Core.
/// The UI holds one instance of this.
#[derive(Debug)]
pub struct PositronicEngine {
    pub pty: Arc<Mutex<PtyManager>>,
    pub state: Arc<StateMachine>,
    pub runner: Arc<Runner>,
    pub airlock: Arc<Airlock>,

    /// Channel to notify UI that the screen changed (needs redraw)
    redraw_notifier: mpsc::Sender<()>,
}

impl PositronicEngine {
    /// Starts the engine: Spawns PTY, starts background tasks, returns the instance.
    pub async fn start(cols: u16, rows: u16, redraw_tx: mpsc::Sender<()>) -> Result<Self> {
        eprintln!("[ENGINE] Starting Positronic Engine...");

        // 1) Create the PTY
        eprintln!("[ENGINE] Creating PTY ({cols}x{rows})...");
        let pty_manager = PtyManager::new(cols, rows).context("Failed to create PTY")?;
        let pty = Arc::new(Mutex::new(pty_manager));
        eprintln!("[ENGINE] PTY created.");

        // 2) Create the State Machine (headless terminal)
        let state = Arc::new(StateMachine::new(cols, rows));
        eprintln!("[ENGINE] State machine created.");

        // 3) Create subsystems
        let airlock = Arc::new(Airlock::new());

        let neural = Arc::new(NeuralClient::new(
            "http://localhost:8000/v1",
            "gpt-3.5-turbo",
        ));
        eprintln!("[ENGINE] Neural client created (will connect on first use).");

        let vault = Vault::open("positronic.db").context("Failed to open Vault database")?;
        eprintln!("[ENGINE] Vault opened.");

        let wasm_host = Arc::new(WasmHost::new().context("Failed to initialize WASM host")?);
        eprintln!("[ENGINE] WASM host initialized.");

        // Initialize Hive
        let (hive_node, mut hive_rx) = HiveNode::new("PositronicUser");
        let hive = Arc::new(hive_node);
        eprintln!("[ENGINE] Hive node created.");

        // Spawn Hive Event Listener -> writes to PTY
        {
            let pty_for_hive = pty.clone();
            tokio::spawn(async move {
                while let Ok(event) = hive_rx.recv().await {
                    let msg = match event {
                        HiveEvent::PeerDiscovered { peer_id, name } => {
                            format!("📡 Peer Found: {} ({})", name, peer_id)
                        }
                        HiveEvent::PeerLost { peer_id } => format!("📡 Peer Lost: {}", peer_id),
                        HiveEvent::BlockReceived { from, content } => {
                            let text = String::from_utf8_lossy(&content);
                            format!("💬 [{}]: {}", from, text)
                        }
                        HiveEvent::LiveSessionInvite { from, session_id } => {
                            format!("📞 Invite from {}: Join {}", from, session_id)
                        }
                        HiveEvent::Error(e) => format!("⚠️ Hive Error: {}", e),
                    };

                    let mut p_lock = pty_for_hive.lock().await;
                    let _ = p_lock.write(&format!("\n{}\n", msg));
                }
            });
        }

        // Initialize Hardware IO
        let (hardware_monitor, mut io_rx) = HardwareMonitor::start();
        let io = Arc::new(hardware_monitor);
        eprintln!("[ENGINE] Hardware IO started.");

        // Spawn IO Event Listener -> writes to PTY
        {
            let pty_for_io = pty.clone();
            tokio::spawn(async move {
                while let Some(event) = io_rx.recv().await {
                    let msg = match event {
                        HardwareEvent::DeviceConnected(name) => {
                            format!("🔌 Device Connected: {}", name)
                        }
                        HardwareEvent::DeviceDisconnected(name) => {
                            format!("🔌 Device Disconnected: {}", name)
                        }
                        HardwareEvent::DataBatch(_) => continue,
                        HardwareEvent::SerialOutput(s) => s,
                        HardwareEvent::Error(e) => format!("⚠️ IO Error: {}", e),
                    };

                    let mut p_lock = pty_for_io.lock().await;
                    let _ = p_lock.write(&msg);
                }
            });
        }

        let runner = Arc::new(Runner::new(
            pty.clone(),
            airlock.clone(),
            neural,
            vault,
            wasm_host,
            hive,
            io,
        ));
        eprintln!("[ENGINE] Runner created.");

        // 4) Start the PTY reader
        let mut rx_ptr = {
            let mut guard = pty.lock().await;
            guard
                .start_reader()
                .context("Failed to start PTY reader")?
        };
        eprintln!("[ENGINE] PTY reader started.");

        let engine = Self {
            pty,
            state: state.clone(),
            runner,
            airlock,
            redraw_notifier: redraw_tx,
        };

        // 5) Spawn the "Pump" task (PTY -> State Machine -> UI redraw notify)
        {
            let state_clone = state.clone();
            let notifier = engine.redraw_notifier.clone();

            tokio::spawn(async move {
                eprintln!("[PUMP] PTY pump task running.");
                while let Some(bytes) = rx_ptr.recv().await {
                    state_clone.process_bytes(&bytes);

                    // Drain any immediately-available burst data
                    while let Ok(more_bytes) = rx_ptr.try_recv() {
                        state_clone.process_bytes(&more_bytes);
                    }

                    let _ = notifier.send(()).await;
                }
                eprintln!("[PUMP] PTY pump task exited.");
            });
        }

        eprintln!("[ENGINE] ✅ Engine started successfully.");
        Ok(engine)
    }

    /// User typed something in the UI.
    ///
    /// - Commands starting with ':' are treated as "internal" and routed to Runner.
    /// - Everything else is sent to the PTY (shell).
    ///
    /// On Windows PTY backends, we normalize `\n` -> `\r\n` so Enter actually executes.
    pub async fn send_input(&self, data: &str) -> Result<()> {
        let trimmed = data.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        // Internal commands (engine/runner domain)
        if trimmed.starts_with(':') {
            let res = self.runner.execute(data).await;
            // Ensure UI gets a chance to refresh even if output isn't PTY-based yet.
            let _ = self.redraw_notifier.send(()).await;
            return res;
        }

        // Shell commands -> PTY
        let normalized = normalize_for_pty(data);

        let mut pty = self.pty.lock().await;
        // Keep this compatible even if `write` is `Result<_>` or not.
        let _ = pty.write(normalized.as_ref());

        Ok(())
    }

    /// UI Window Resized -> Resize PTY and terminal grid
    pub async fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        let mut pty = self.pty.lock().await;
        pty.resize(cols, rows).context("Failed to resize PTY")?;
        self.state.resize(cols, rows);

        // Resize impacts display; request redraw.
        let _ = self.redraw_notifier.send(()).await;

        Ok(())
    }
}

/// Normalize newline behavior for PTY backends.
///
/// On Windows PTY/ConPTY style backends, "Enter" commonly requires CRLF.
/// If input already contains '\r', we leave it unchanged.
fn normalize_for_pty(input: &str) -> Cow<'_, str> {
    #[cfg(windows)]
    {
        if input.contains('\r') {
            return Cow::Borrowed(input);
        }
        if !input.contains('\n') {
            return Cow::Borrowed(input);
        }

        let extra = input.as_bytes().iter().filter(|&&b| b == b'\n').count();
        let mut out = String::with_capacity(input.len() + extra);

        for ch in input.chars() {
            if ch == '\n' {
                out.push('\r');
                out.push('\n');
            } else {
                out.push(ch);
            }
        }

        Cow::Owned(out)
    }

    #[cfg(not(windows))]
    {
        Cow::Borrowed(input)
    }
}
