use crate::airlock::Airlock;
use crate::pty_manager::PtyManager;
use crate::runner::{ExecuteResult, Runner};
use crate::state_machine::StateMachine;
use crate::vault::Vault;

use anyhow::{Context, Result};
use positronic_hive::{HiveEvent, HiveNode};
use positronic_io::{HardwareEvent, HardwareMonitor};
use positronic_neural::cortex::NeuralClient;
use positronic_script::wasm_host::WasmHost;

use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

#[derive(Debug)]
pub struct PositronicEngine {
    pub pty: Arc<Mutex<PtyManager>>,
    pub state: Arc<StateMachine>,
    pub runner: Arc<Runner>,
    pub airlock: Arc<Airlock>,
    redraw_notifier: mpsc::Sender<()>,
}

impl PositronicEngine {
    pub async fn start(cols: u16, rows: u16, redraw_tx: mpsc::Sender<()>) -> Result<Self> {
        // 1) PTY + State Machine
        let mut pty_manager = PtyManager::new(cols, rows).context("Failed to create PTY")?;
        let mut rx_ptr = pty_manager
            .start_reader()
            .context("Failed to start PTY reader")?;

        let pty = Arc::new(Mutex::new(pty_manager));
        let state = Arc::new(StateMachine::new(cols, rows));

        // 2) Pump: PTY output → StateMachine
        {
            let state_clone = state.clone();
            let notifier = redraw_tx.clone();
            tokio::spawn(async move {
                while let Some(bytes) = rx_ptr.recv().await {
                    state_clone.process_bytes(&bytes);
                    while let Ok(more) = rx_ptr.try_recv() {
                        state_clone.process_bytes(&more);
                    }
                    let _ = notifier.try_send(());
                }
            });
        }

        // Kick the shell for initial prompt
        {
            let mut p = pty.lock().await;
            let _ = p.write_line("");
        }
        let _ = redraw_tx.try_send(());

        // 3) Subsystems
        let airlock = Arc::new(Airlock::new());

        // ── Neural: Lemonade on localhost:8000 ──
        // CORRECT URL: http://localhost:8000/api/v1 (note /api/ prefix)
        // Model "auto" queries /api/v1/models and uses first loaded model
        let neural = Arc::new(NeuralClient::new(
            "http://localhost:8000/api/v1",
            "auto",
        ));

        let vault = Vault::open("positronic.db").context("Failed to open Vault database")?;
        let wasm_host = Arc::new(WasmHost::new().context("Failed to initialize WASM host")?);

        let (hive_node, mut hive_rx) = HiveNode::new("PositronicUser");
        let hive = Arc::new(hive_node);

        // Hive event listener
        {
            let pty_for_hive = pty.clone();
            tokio::spawn(async move {
                while let Ok(event) = hive_rx.recv().await {
                    let msg = match event {
                        HiveEvent::PeerDiscovered { peer_id, name } => {
                            format!("📡 Peer Found: {} ({})", name, peer_id)
                        }
                        HiveEvent::PeerLost { peer_id } => {
                            format!("📡 Peer Lost: {}", peer_id)
                        }
                        HiveEvent::BlockReceived { from, content } => {
                            let text = String::from_utf8_lossy(&content);
                            format!("💬 [{}]: {}", from, text)
                        }
                        HiveEvent::LiveSessionInvite { from, session_id } => {
                            format!("📞 Invite from {}: Join {}", from, session_id)
                        }
                        HiveEvent::Error(e) => format!("⚠️ Hive: {}", e),
                    };
                    let mut p = pty_for_hive.lock().await;
                    let _ = p.write_line(&shell_echo_cmd(&msg));
                }
            });
        }

        let (hardware_monitor, mut io_rx) = HardwareMonitor::start();
        let io = Arc::new(hardware_monitor);

        // Hardware event listener
        {
            let pty_for_io = pty.clone();
            tokio::spawn(async move {
                while let Some(event) = io_rx.recv().await {
                    let msg = match event {
                        HardwareEvent::DeviceConnected(n) => format!("🔌 Connected: {}", n),
                        HardwareEvent::DeviceDisconnected(n) => format!("🔌 Disconnected: {}", n),
                        HardwareEvent::DataBatch(_) => continue,
                        HardwareEvent::SerialOutput(s) => s,
                        HardwareEvent::Error(e) => format!("⚠️ IO: {}", e),
                    };
                    let mut p = pty_for_io.lock().await;
                    let _ = p.write_line(&shell_echo_cmd(&msg));
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

        Ok(Self {
            pty,
            state,
            runner,
            airlock,
            redraw_notifier: redraw_tx,
        })
    }

    pub async fn send_input(&self, data: &str) -> Result<ExecuteResult> {
        self.runner.execute(data).await
    }

    pub async fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        let mut pty = self.pty.lock().await;
        pty.resize(cols, rows)?;
        self.state.resize(cols, rows);
        let _ = self.redraw_notifier.try_send(());
        Ok(())
    }
}

fn shell_echo_cmd(text: &str) -> String {
    if cfg!(windows) {
        let escaped = text.replace('\'', "''");
        format!("Write-Output '{}'", escaped)
    } else {
        let escaped = text.replace('\'', r#"'"'"'"#);
        format!("printf '%s\\n' '{}'", escaped)
    }
}