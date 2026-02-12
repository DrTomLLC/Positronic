use crate::airlock::Airlock;
use crate::pty_manager::PtyManager;
use crate::runtime::parser::{CommandParser, CommandType, HiveCommand, IOCommand};
use crate::vault::Vault;

use anyhow::Result;
use positronic_hive::HiveNode;
use positronic_io::HardwareMonitor;
use positronic_neural::cortex::NeuralClient;
use positronic_script::wasm_host::WasmHost;

use std::sync::Arc;
use tokio::sync::Mutex;

/// Result of executing a command — tells the UI what to do.
#[derive(Debug, Clone)]
pub enum ExecuteResult {
    /// Command was sent to the PTY; the UI should wait for snapshot updates.
    SentToPty,
    /// Command produced output directly (! commands). Display this text.
    Output(String),
    /// Clear the screen.
    ClearScreen,
}

#[derive(Debug)]
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

    /// Execute user input — parse it and route to the appropriate subsystem.
    pub async fn execute(&self, data: &str) -> Result<ExecuteResult> {
        let mut normalized = data.replace("\r\n", "\n");
        while normalized.ends_with('\n') {
            normalized.pop();
        }

        if normalized.trim().is_empty() {
            let mut pty = self.pty.lock().await;
            let _ = pty.write_line("");
            return Ok(ExecuteResult::SentToPty);
        }

        // ---- Intercept clear/cls before parsing ----
        let lower = normalized.trim().to_lowercase();
        if lower == "clear" || lower == "cls" || lower == "!clear" {
            return Ok(ExecuteResult::ClearScreen);
        }

        // ---- Parse and route ----
        let parsed = CommandParser::parse(&normalized);

        match parsed {
            // Regular shell commands → PTY
            CommandType::Legacy(cmd) => {
                let _ = self.vault.log_command(&cmd, None, None, ".", None);
                let mut pty = self.pty.lock().await;
                pty.write_line(&cmd)?;
                Ok(ExecuteResult::SentToPty)
            }

            // !ver, !help, !history, etc. → direct output
            CommandType::Native(cmd, args) => {
                let output = self.handle_native(&cmd, &args).await;
                Ok(ExecuteResult::Output(output))
            }

            // !ai <prompt> or !ask <prompt> → Neural
            CommandType::Neural(prompt) => {
                let mut lines = vec!["🧠 Asking Neural...".to_string()];
                match self.neural.ask(&prompt).await {
                    Ok(response) => {
                        for line in response.lines() {
                            lines.push(format!("  {}", line));
                        }
                    }
                    Err(e) => {
                        lines.push(format!("❌ Neural error: {}", e));
                        lines.push(
                            "   (Is Lemonade running on localhost:8000?)".to_string(),
                        );
                    }
                }
                Ok(ExecuteResult::Output(lines.join("\n")))
            }

            // sandbox <cmd> → Airlock
            CommandType::Sandboxed(cmd) => {
                let mut lines = vec![format!("🔒 Sandboxing: {}", cmd)];
                match self.airlock.run_sandboxed(&cmd).await {
                    Ok(output) => {
                        for line in output.lines() {
                            lines.push(line.to_string());
                        }
                    }
                    Err(e) => {
                        lines.push(format!("❌ Airlock error: {}", e));
                    }
                }
                Ok(ExecuteResult::Output(lines.join("\n")))
            }

            // !run <path> or !wasm <path> → Script
            CommandType::Script(kind, path) => {
                let output = self.handle_script(&kind, &path).await;
                Ok(ExecuteResult::Output(output))
            }

            // !hive scan, !hive status, !chat <msg>
            CommandType::Hive(hive_cmd) => {
                let output = self.handle_hive(hive_cmd).await;
                Ok(ExecuteResult::Output(output))
            }

            // !io scan, !io connect <port> <baud>
            CommandType::IO(io_cmd) => {
                let output = self.handle_io(io_cmd).await;
                Ok(ExecuteResult::Output(output))
            }
        }
    }

    // ----------------------------------------------------------------
    // Native (!-prefixed built-in) commands
    // ----------------------------------------------------------------

    async fn handle_native(&self, cmd: &str, args: &[String]) -> String {
        match cmd {
            "ver" | "version" => {
                "⚡ Positronic v0.1.0 — Local-First Terminal".to_string()
            }

            "help" => [
                "⚡ Positronic Commands:",
                "",
                "  !ver                          Version info",
                "  !help                         This help message",
                "  !history <query>              Search command history",
                "  !clear                        Clear the screen",
                "",
                "  !ai <prompt>                  Ask the Neural engine",
                "  !ask <prompt>                 Alias for !ai",
                "",
                "  !run <path.rs>                Run a Rust script",
                "  !wasm <path.wasm>             Run a WASM plugin",
                "",
                "  !hive scan                    Scan for peers",
                "  !hive status                  Hive network status",
                "  !chat <message>               Send message to mesh",
                "",
                "  !io scan                      Scan serial ports",
                "  !io list                      Alias for !io scan",
                "  !io connect <port> <baud>     Connect to device",
                "",
                "  sandbox <cmd>                 Run command in Airlock",
                "  clear / cls                   Clear the screen",
                "",
                "  (Anything else is sent to your system shell.)",
            ]
                .join("\n"),

            "history" => {
                let query = args.join(" ");
                let search = if query.is_empty() { "%" } else { &query };
                match self.vault.search_history(search) {
                    Ok(records) => {
                        if records.is_empty() {
                            "📜 No history found.".to_string()
                        } else {
                            let mut lines =
                                vec![format!("📜 Found {} result(s):", records.len())];
                            for r in records.iter().take(25) {
                                let code = r
                                    .exit_code
                                    .map(|c| c.to_string())
                                    .unwrap_or_else(|| "?".into());
                                lines.push(format!("  [{}] {}", code, r.command));
                            }
                            lines.join("\n")
                        }
                    }
                    Err(e) => format!("❌ History error: {}", e),
                }
            }

            other => {
                format!(
                    "❓ Unknown command: !{}\n   Type !help for available commands.",
                    other
                )
            }
        }
    }

    // ----------------------------------------------------------------
    // Script execution
    // ----------------------------------------------------------------

    async fn handle_script(&self, kind: &str, path: &str) -> String {
        match kind {
            "run" => {
                let mut lines = vec![format!("🚀 Running script: {}", path)];
                let p = std::path::Path::new(path);
                match positronic_script::execute_script(p, &[]).await {
                    Ok(output) => {
                        for line in output.lines() {
                            lines.push(line.to_string());
                        }
                    }
                    Err(e) => lines.push(format!("❌ Script error: {}", e)),
                }
                lines.join("\n")
            }
            "wasm" => {
                let mut lines = vec![format!("🧩 Loading WASM: {}", path)];
                match std::fs::read(path) {
                    Ok(bytes) => match self.wasm_host.run_plugin(&bytes) {
                        Ok(()) => lines.push("✅ WASM plugin executed.".to_string()),
                        Err(e) => lines.push(format!("❌ WASM error: {}", e)),
                    },
                    Err(e) => lines.push(format!("❌ Failed to read {}: {}", path, e)),
                }
                lines.join("\n")
            }
            _ => format!("❓ Unknown script type: {}", kind),
        }
    }

    // ----------------------------------------------------------------
    // Hive / P2P
    // ----------------------------------------------------------------

    async fn handle_hive(&self, cmd: HiveCommand) -> String {
        match cmd {
            HiveCommand::Scan => match self.hive.start_discovery().await {
                Ok(()) => "📡 Discovery active. Scanning for peers...".to_string(),
                Err(e) => format!("❌ Discovery error: {}", e),
            },
            HiveCommand::Status => {
                let peer = &self.hive.local_peer;
                [
                    format!("📡 Hive Node: {} ({})", peer.name, peer.id),
                    format!("   Address: {}", peer.address),
                    format!("   Capabilities: {:?}", peer.capabilities),
                ]
                    .join("\n")
            }
            HiveCommand::Chat(msg) => {
                match self.hive.broadcast_block(msg.clone().into_bytes()).await {
                    Ok(()) => format!("💬 Sent: {}", msg),
                    Err(e) => format!("❌ Broadcast error: {}", e),
                }
            }
        }
    }

    // ----------------------------------------------------------------
    // Hardware IO
    // ----------------------------------------------------------------

    async fn handle_io(&self, cmd: IOCommand) -> String {
        match cmd {
            IOCommand::Scan | IOCommand::List => match self.io.scan_ports().await {
                Ok(()) => {
                    "🔌 Scanning serial ports... (results appear as devices respond)"
                        .to_string()
                }
                Err(e) => format!("❌ Scan error: {}", e),
            },
            IOCommand::Connect(port, baud) => match self.io.connect(&port, baud).await {
                Ok(()) => format!("🔌 Connected to {} @ {} baud", port, baud),
                Err(e) => format!("❌ Connect error: {}", e),
            },
        }
    }
}