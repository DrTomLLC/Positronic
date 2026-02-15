//! Positronic Engine — the core coordinator.
//!
//! Owns the PTY, state machine, runner, airlock and all subsystem handles.
//! After the pager-trap bugfix, this module also exposes low-level PTY
//! control signals (`send_interrupt`, `send_escape`, `send_eof`, `send_raw`)
//! so the UI can break out of pagers and continuation prompts.

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

use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

// Re-export so `positronic_core::engine::ExecuteResult` keeps working.
pub use crate::runner::ExecuteResult;

#[derive(Debug)]
pub struct PositronicEngine {
    pub pty: Arc<Mutex<PtyManager>>,
    pub state: Arc<StateMachine>,
    pub runner: Arc<Runner>,
    pub airlock: Arc<Airlock>,
    pub pty_output_buf: Arc<std::sync::Mutex<Vec<u8>>>,
    redraw_notifier: mpsc::Sender<()>,
}

impl PositronicEngine {
    pub async fn start(cols: u16, rows: u16, redraw_tx: mpsc::Sender<()>) -> Result<Self> {
        let mut pty_manager = PtyManager::new(cols, rows).context("Failed to create PTY")?;
        let mut rx_ptr = pty_manager
            .start_reader()
            .context("Failed to start PTY reader")?;

        let pty = Arc::new(Mutex::new(pty_manager));
        let state = Arc::new(StateMachine::new(cols, rows));
        let pty_output_buf: Arc<std::sync::Mutex<Vec<u8>>> =
            Arc::new(std::sync::Mutex::new(Vec::with_capacity(8192)));

        // PTY reader pump — feeds bytes into state machine and output buffer
        {
            let state_clone = state.clone();
            let buf_clone = pty_output_buf.clone();
            let notifier = redraw_tx.clone();
            tokio::spawn(async move {
                while let Some(bytes) = rx_ptr.recv().await {
                    if let Ok(mut buf) = buf_clone.lock() {
                        buf.extend_from_slice(&bytes);
                    }
                    state_clone.process_bytes(&bytes);

                    // Drain any immediately-available follow-up chunks
                    while let Ok(more) = rx_ptr.try_recv() {
                        if let Ok(mut buf) = buf_clone.lock() {
                            buf.extend_from_slice(&more);
                        }
                        state_clone.process_bytes(&more);
                    }
                    let _ = notifier.try_send(());
                }
            });
        }

        // Kick the shell so the initial prompt appears
        {
            let mut p = pty.lock().await;
            let _ = p.write_line("");
        }
        let _ = redraw_tx.try_send(());

        let airlock = Arc::new(Airlock::new());

        let neural = Arc::new(NeuralClient::new(
            "http://localhost:8000/api/v1",
            "auto",
        ));

        let vault = Vault::open("positronic.db").context("Failed to open Vault")?;
        let wasm_host = Arc::new(WasmHost::new().context("Failed to init WASM host")?);

        let (hive_node, mut hive_rx) = HiveNode::new("PositronicUser");
        let hive = Arc::new(hive_node);

        // Hive event pump — echo peer events into the PTY
        {
            let pty_for_hive = pty.clone();
            let (tx, mut rx) = mpsc::channel::<String>(32);

            tokio::spawn(async move {
                while let Some(cmd) = rx.recv().await {
                    let mut p = pty_for_hive.lock().await;
                    let _ = p.write_line(&cmd);
                }
            });

            tokio::spawn(async move {
                while let Ok(event) = hive_rx.recv().await {
                    let msg = match event {
                        HiveEvent::PeerDiscovered { peer_id, name } => {
                            format!("📡 Peer: {} ({})", name, peer_id)
                        }
                        HiveEvent::PeerLost { peer_id } => {
                            format!("📡 Peer Lost: {}", peer_id)
                        }
                        HiveEvent::BlockReceived { from, content } => {
                            let text = String::from_utf8_lossy(&content);
                            format!("💬 [{}]: {}", from, text)
                        }
                        HiveEvent::LiveSessionInvite { from, session_id } => {
                            format!("📞 Invite from {}: {}", from, session_id)
                        }
                        HiveEvent::Error(e) => format!("⚠️ Hive: {}", e),
                    };
                    let _ = tx.send(shell_echo_cmd(&msg)).await;
                }
            });
        }

        // Hardware I/O pump
        let (hardware_monitor, mut io_rx) = HardwareMonitor::start();
        let io = Arc::new(hardware_monitor);

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
            pty_output_buf,
            redraw_notifier: redraw_tx,
        })
    }

    // ────────────────────────────────────────────────────────────────
    // High-level command interface
    // ────────────────────────────────────────────────────────────────

    pub async fn send_input(&self, data: &str) -> Result<ExecuteResult> {
        self.runner.execute(data).await
    }

    pub fn drain_pty_output(&self) -> Vec<u8> {
        match self.pty_output_buf.lock() {
            Ok(mut buf) => std::mem::take(&mut *buf),
            Err(poisoned) => {
                let mut buf = poisoned.into_inner();
                std::mem::take(&mut *buf)
            }
        }
    }

    pub async fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        let mut pty = self.pty.lock().await;
        pty.resize(cols, rows)?;
        self.state.resize(cols, rows);
        let _ = self.redraw_notifier.try_send(());
        Ok(())
    }

    // ────────────────────────────────────────────────────────────────
    // Low-level PTY control signals  (PAGER-TRAP BUGFIX)
    //
    // These allow the UI to send raw control characters to the PTY
    // without going through the Runner command pipeline.
    // ────────────────────────────────────────────────────────────────

    /// Send Ctrl+C (ETX / 0x03) — interrupt the running process / exit pager.
    pub async fn send_interrupt(&self) -> Result<()> {
        let mut pty = self.pty.lock().await;
        pty.write_raw("\x03")?;
        let _ = self.redraw_notifier.try_send(());
        Ok(())
    }

    /// Send Escape (0x1b) — exit vi-style pagers, cancel prompts.
    pub async fn send_escape(&self) -> Result<()> {
        let mut pty = self.pty.lock().await;
        pty.write_raw("\x1b")?;
        let _ = self.redraw_notifier.try_send(());
        Ok(())
    }

    /// Send Ctrl+D (EOT / 0x04) — signal end-of-input.
    pub async fn send_eof(&self) -> Result<()> {
        let mut pty = self.pty.lock().await;
        pty.write_raw("\x04")?;
        let _ = self.redraw_notifier.try_send(());
        Ok(())
    }

    /// Send arbitrary raw data to the PTY (no newline appended).
    pub async fn send_raw(&self, data: &str) -> Result<()> {
        let mut pty = self.pty.lock().await;
        pty.write_raw(data)?;
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