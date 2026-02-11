use crate::airlock::Airlock;
use crate::pty_manager::PtyManager;
use crate::runner::Runner;
use crate::runtime::parser::CommandParser;
use crate::state_machine::StateMachine;
use crate::vault::Vault;
use anyhow::Result;
use positronic_hive::{HiveEvent, HiveNode};
use positronic_io::{HardwareEvent, HardwareMonitor};
use positronic_neural::cortex::NeuralClient;
use positronic_script::wasm_host::WasmHost;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

/// The main entry point for the Positronic Core.
/// The UI holds one instance of this.
#[derive(Debug)]
pub struct PositronicEngine {
    pub pty: Arc<Mutex<PtyManager>>,
    pub state: Arc<StateMachine>,
    pub runner: Arc<Runner>,
    pub airlock: Arc<Airlock>,
    // Channel to notify UI that the screen changed (needs redraw)
    redraw_notifier: mpsc::Sender<()>,
}

impl PositronicEngine {
    /// Starts the engine: Spawns PTY, starts background threads, returns the instance.
    pub async fn start(cols: u16, rows: u16, redraw_tx: mpsc::Sender<()>) -> Result<Self> {
        // 1. Create the PTY
        let pty_manager = PtyManager::new(cols, rows)?;
        let pty = Arc::new(Mutex::new(pty_manager));

        // 2. Create the State Machine (Headless Terminal)
        let state = Arc::new(StateMachine::new(cols, rows));

        // 3. Create Airlock, Cortex, Vault, WasmHost, Hive, IO and Runner
        let airlock = Arc::new(Airlock::new());
        // Use local lemonade/llama.cpp server by default
        let neural = Arc::new(NeuralClient::new(
            "http://localhost:8000/v1",
            "gpt-3.5-turbo",
        ));
        let vault = Vault::open("positronic.db").expect("Failed to open vault");
        let wasm_host = Arc::new(WasmHost::new()?);

        // Initialize Hive
        let (hive_node, mut hive_rx) = HiveNode::new("PositronicUser");
        let hive = Arc::new(hive_node);

        // Spawn Hive Event Listener
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

        // Initialize Hardware IO
        let (hardware_monitor, mut io_rx) = HardwareMonitor::start();
        let io = Arc::new(hardware_monitor);

        // Spawn IO Event Listener
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
                    HardwareEvent::DataBatch(_) => continue, // Too fast for terminal, oscilloscope only
                    HardwareEvent::SerialOutput(s) => s,     // Raw stream
                    HardwareEvent::Error(e) => format!("⚠️ IO Error: {}", e),
                };

                let mut p_lock = pty_for_io.lock().await;
                // If it's pure serial output, don't newline wrap it aggressively?
                // For now, simple write.
                let _ = p_lock.write(&msg);
            }
        });

        let runner = Arc::new(Runner::new(
            pty.clone(),
            airlock.clone(),
            neural,
            vault,
            wasm_host,
            hive,
            io,
        ));

        // 4. Start the PTY Reader Thread
        // We need to lock pty temporarily to start the reader
        let mut rx_ptr = {
            let mut guard = pty.lock().await;
            guard.start_reader()?
        };

        let engine = Self {
            pty,
            state: state.clone(),
            runner,
            airlock,
            redraw_notifier: redraw_tx,
        };

        // 5. Spawn the "Pump" Task (Connects PTY -> State Machine)
        let state_clone = state.clone();
        let notifier = engine.redraw_notifier.clone();

        tokio::spawn(async move {
            while let Some(bytes) = rx_ptr.recv().await {
                // COALESCING: Read as much as available without blocking before redrawing
                // This prevents UI floods during high-throughput (like 'ls -R')
                state_clone.process_bytes(&bytes);

                // Try to drain channel if more data is ready immediately
                while let Ok(more_bytes) = rx_ptr.try_recv() {
                    state_clone.process_bytes(&more_bytes);
                }

                let _ = notifier.send(()).await;
            }
        });

        Ok(engine)
    }

    /// User typed something in the UI -> Send to Runner
    pub async fn send_input(&self, data: &str) -> Result<()> {
        self.runner.execute(data).await
    }

    /// UI Window Resized -> Resize PTY and Grid
    pub async fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        // Resize PTY (OS level)
        let mut pty = self.pty.lock().await;
        pty.resize(cols, rows)?;

        // Resize Grid (Alacritty level)
        self.state.resize(cols, rows);

        Ok(())
    }
}
