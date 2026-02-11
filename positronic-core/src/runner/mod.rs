use crate::airlock::Airlock;
use crate::pty_manager::PtyManager;
use crate::runtime::parser::{CommandParser, CommandType, HiveCommand, IOCommand};
use crate::vault::Vault;
use anyhow::Result;
use positronic_hive::HiveNode;
use positronic_io::HardwareMonitor;
use positronic_neural::cortex::NeuralClient;
use positronic_script::wasm_host::WasmHost;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Debug)]
pub struct Runner {
    pty: Arc<Mutex<PtyManager>>,
    airlock: Arc<Airlock>,
    neural: Arc<NeuralClient>,
    vault: Vault,
    wasm_host: Arc<WasmHost>,
    hive: Arc<HiveNode>,
    io: Arc<HardwareMonitor>,
}

impl Runner {
    pub fn new(
        pty: Arc<Mutex<PtyManager>>,
        airlock: Arc<Airlock>,
        neural: Arc<NeuralClient>,
        vault: Vault,
        wasm_host: Arc<WasmHost>,
        hive: Arc<HiveNode>,
        io: Arc<HardwareMonitor>,
    ) -> Self {
        Self {
            pty,
            airlock,
            neural,
            vault,
            wasm_host,
            hive,
            io,
        }
    }

    pub async fn execute(&self, input: &str) -> Result<()> {
        let cmd_type = CommandParser::parse(input);

        // Log the command attempt
        // Note: functionality to capture exit code/output is limited in this async fire-and-forget model
        // So we log what we can.
        let _ = self
            .vault
            .log_command(input.trim(), None, None, "cwd_placeholder", None);

        match cmd_type {
            CommandType::Native(cmd, args) => {
                // Handle native commands (e.g., !ver)
                // For now, we just print to stdout (which goes to PTY effectively if we hooked it up,
                // but Native commands usually output directly to the UI channel or a special log).
                // Since our UI listens to the PTY *output*, native commands need to inject output into the stream or UI.
                // Architecture decision: Native commands write to the PTY's input stream? No, that goes to shell.
                // They should write to the PTY's *output* stream mock, OR we handle them by sending a special event.
                // FOR NOW: We will just echo to PTY for simplicity, or handle !ver specifically.
                if cmd == "ver" {
                    let mut pty = self.pty.lock().await;
                    pty.write("Positronic Core v0.1.0\n")?;
                } else if cmd == "history" {
                    let mut pty = self.pty.lock().await;
                    match self.vault.search_history(&args.join(" ")) {
                        Ok(records) => {
                            pty.write("\n--- Command History ---\n")?;
                            for record in records {
                                pty.write(&format!("[{}] {}\n", record.timestamp, record.command))?;
                            }
                            pty.write("-----------------------\n")?;
                        }
                        Err(e) => {
                            pty.write(&format!("Error retrieving history: {}\n", e))?;
                        }
                    }
                } else {
                    // Pass-through execution for standard shell commands (ls, dir, git, etc.)
                    let mut pty = self.pty.lock().await;
                    let line = if args.is_empty() {
                        format!("{}\n", cmd)
                    } else {
                        format!("{} {}\n", cmd, args.join(" "))
                    };
                    pty.write(&line)?;
                }
            }
            CommandType::Legacy(raw) => {
                let mut pty = self.pty.lock().await;
                pty.write(&raw)?;
            }
            CommandType::Sandboxed(cmd) => {
                let output = self.airlock.run_sandboxed(&cmd).await?;
                // Inject the result into the PTY stream so the user sees it?
                // Or just write it to the PTY input (which might echo it back).
                // Better: Write to PTY so it appears in the terminal flow.
                let mut pty = self.pty.lock().await;
                pty.write(&format!("\n{}\n", output))?;
            }
            CommandType::Neural(prompt) => {
                let mut pty = self.pty.lock().await;
                pty.write(&format!("\n🤖 Asking Cortex: '{}'...\n", prompt))?;
                drop(pty); // Release lock while waiting for AI

                match self.neural.ask(&prompt).await {
                    Ok(response) => {
                        let mut pty = self.pty.lock().await;
                        pty.write(&format!("\n> {}\n\n", response))?;
                    }
                    Err(e) => {
                        let mut pty = self.pty.lock().await;
                        pty.write(&format!("\n⚠️ Cortex Error: {}\n", e))?;
                    }
                }
            }
            CommandType::Script(stype, path) => {
                let mut pty = self.pty.lock().await;
                pty.write(&format!("\n📜 Executing {} script: {}\n", stype, path))?;
                drop(pty);

                let result = if stype == "run" {
                    positronic_script::execute_script(Path::new(&path), &[])
                        .await
                        .map_err(|e| e.to_string())
                } else {
                    // WASM
                    // Read file content
                    match std::fs::read(&path) {
                        Ok(bytes) => self.wasm_host.run_plugin(&bytes).map_err(|e| e.to_string()),
                        Err(e) => Err(e.to_string()),
                    }
                    .map(|_| "WASM Plugin Executed Successfully.".to_string())
                };

                let mut pty = self.pty.lock().await;
                match result {
                    Ok(out) => pty.write(&format!("\n>>> OUTPUT:\n{}\n", out))?,
                    Err(e) => pty.write(&format!("\n❌ Script Error: {}\n", e))?,
                }
            }
            CommandType::Hive(cmd) => {
                match cmd {
                    HiveCommand::Scan => {
                        let mut pty = self.pty.lock().await;
                        pty.write("\n📡 Scanning for peers...\n")?;
                        if let Err(e) = self.hive.start_discovery().await {
                            pty.write(&format!("Error starting discovery: {}\n", e))?;
                        }
                    }
                    HiveCommand::Status => {
                        let pty = self.pty.lock().await;
                        // This would typically read internal state, but for now we just ack
                        // Actually we can read local peer info from HiveNode if it was pub,
                        // but let's just say "Online"
                        // Actually HiveNode.local_peer is pub.
                        drop(pty);
                        let mut pty_write = self.pty.lock().await;

                        pty_write.write(&format!(
                            "\n🐝 Hive Node Status:\nID: {}\nName: {}\n",
                            self.hive.local_peer.id, self.hive.local_peer.name
                        ))?;
                    }
                    HiveCommand::Chat(msg) => {
                        // Broadcast block
                        if let Err(e) = self.hive.broadcast_block(msg.as_bytes().to_vec()).await {
                            let mut pty = self.pty.lock().await;
                            pty.write(&format!("\n❌ Chat Error: {}\n", e))?;
                        }
                    }
                }
            }
            CommandType::IO(cmd) => match cmd {
                IOCommand::Scan | IOCommand::List => {
                    let mut pty = self.pty.lock().await;
                    pty.write("\n🔌 Scanning for hardware...\n")?;
                    if let Err(e) = self.io.scan_ports().await {
                        pty.write(&format!("Error scanning ports: {}\n", e))?;
                    }
                }
                IOCommand::Connect(port, baud) => {
                    let mut pty = self.pty.lock().await;
                    pty.write(&format!("\n🔌 Connecting to {} @ {}...\n", port, baud))?;
                    if let Err(e) = self.io.connect(&port, baud).await {
                        pty.write(&format!("Error connecting: {}\n", e))?;
                    }
                }
            },
        }
        Ok(())
    }
}
