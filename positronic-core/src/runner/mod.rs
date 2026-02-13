use crate::airlock::Airlock;
use crate::pty_manager::PtyManager;
use crate::runtime::parser::{CommandParser, CommandType, HiveCommand, IOCommand};
use crate::vault::Vault;

use anyhow::Result;
use positronic_hive::HiveNode;
use positronic_io::HardwareMonitor;
use positronic_neural::cortex::NeuralClient;
use positronic_neural::reflex::ReflexEngine;
use positronic_script::wasm_host::WasmHost;

use std::sync::Arc;
use tokio::sync::Mutex;

/// Result of executing a command — tells the UI what to display.
#[derive(Debug, Clone)]
pub enum ExecuteResult {
    /// Command was sent to the PTY shell. UI waits for snapshot redraws.
    SentToPty,
    /// Command produced direct output (bypass PTY). Display these lines.
    DirectOutput(Vec<String>),
    /// Clear the screen (PTY has already been sent cls).
    ClearScreen,
}

/// Confidence threshold for Reflex auto-correction.
const AUTO_CORRECT_THRESHOLD: f64 = 0.8;

#[derive(Debug)]
pub struct Runner {
    pty: Arc<Mutex<PtyManager>>,
    #[allow(dead_code)]
    airlock: Arc<Airlock>,
    neural: Arc<NeuralClient>,
    vault: Vault,
    #[allow(dead_code)]
    wasm_host: Arc<WasmHost>,
    #[allow(dead_code)]
    hive: Arc<HiveNode>,
    #[allow(dead_code)]
    io: Arc<HardwareMonitor>,
    reflex: ReflexEngine,
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
            reflex: ReflexEngine::new(),
        }
    }

    pub async fn execute(&self, data: &str) -> Result<ExecuteResult> {
        let normalized = data
            .replace("\r\n", "\n")
            .trim_end_matches('\n')
            .to_string();

        if normalized.trim().is_empty() {
            let mut pty = self.pty.lock().await;
            let _ = pty.write_line("");
            return Ok(ExecuteResult::SentToPty);
        }

        // Intercept clear/cls — send cls to the ACTUAL PTY so its
        // screen buffer is wiped, then tell the UI to clear too.
        let lower = normalized.trim().to_lowercase();
        if lower == "clear" || lower == "cls" || lower == "!clear" {
            let mut pty = self.pty.lock().await;
            // Send the real cls/clear to the PTY shell so the
            // underlying terminal buffer is emptied.
            if cfg!(windows) {
                pty.write_line("cls")?;
            } else {
                pty.write_line("clear")?;
            }
            return Ok(ExecuteResult::ClearScreen);
        }

        let parsed = CommandParser::parse(&normalized);

        match parsed {
            // Shell commands → Reflex check → PTY
            CommandType::Legacy(cmd) => self.execute_shell_command(&cmd).await,

            // Built-in ! commands
            CommandType::Native(cmd, args) => {
                let lines = self.handle_native(&cmd, &args).await;
                Ok(ExecuteResult::DirectOutput(lines))
            }

            // !ai / !ask → Neural
            CommandType::Neural(prompt) => self.handle_neural(&prompt).await,

            // STUB: sandbox
            CommandType::Sandboxed(_cmd) => Ok(ExecuteResult::DirectOutput(vec![
                "🔒 Airlock sandboxing is not yet implemented.".to_string(),
            ])),

            // STUB: !run / !wasm
            CommandType::Script(kind, path) => Ok(ExecuteResult::DirectOutput(vec![
                format!("🚀 !{} {} — not yet implemented.", kind, path),
            ])),

            // STUB: !hive
            CommandType::Hive(hive_cmd) => {
                let msg = match hive_cmd {
                    HiveCommand::Scan => {
                        "📡 Hive peer discovery — not yet implemented (loopback only)."
                    }
                    HiveCommand::Status => "📡 Hive is in loopback simulation mode.",
                    HiveCommand::Chat(_) => {
                        "💬 Hive mesh chat — not yet implemented (loopback only)."
                    }
                };
                Ok(ExecuteResult::DirectOutput(vec![msg.to_string()]))
            }

            // STUB: !io
            CommandType::IO(io_cmd) => {
                let msg = match io_cmd {
                    IOCommand::Scan | IOCommand::List => {
                        "🔌 Hardware IO scanning — not yet implemented."
                    }
                    IOCommand::Connect(_, _) => {
                        "🔌 Serial port connection — not yet implemented."
                    }
                };
                Ok(ExecuteResult::DirectOutput(vec![msg.to_string()]))
            }
        }
    }

    // ----------------------------------------------------------------
    // Shell command with Reflex typo correction
    // ----------------------------------------------------------------

    async fn execute_shell_command(&self, cmd: &str) -> Result<ExecuteResult> {
        if let Some(suggestion) = self.reflex.fix_command(cmd) {
            if suggestion.confidence >= AUTO_CORRECT_THRESHOLD {
                // High confidence → auto-correct
                let lines = vec![format!(
                    "  💡 Auto-corrected → {} ({:.0}%, {:?})",
                    suggestion.corrected,
                    suggestion.confidence * 100.0,
                    suggestion.source
                )];

                let _ =
                    self.vault
                        .log_command(&suggestion.corrected, None, None, ".", None);
                let mut pty = self.pty.lock().await;
                pty.write_line(&suggestion.corrected)?;

                return Ok(ExecuteResult::DirectOutput(lines));
            } else {
                // Lower confidence → hint only, execute original
                let hint = format!(
                    "  💡 Did you mean: {}? ({:.0}%)",
                    suggestion.corrected,
                    suggestion.confidence * 100.0
                );

                let _ = self.vault.log_command(cmd, None, None, ".", None);
                let mut pty = self.pty.lock().await;
                pty.write_line(cmd)?;

                return Ok(ExecuteResult::DirectOutput(vec![hint]));
            }
        }

        // No typo detected — execute as-is
        let _ = self.vault.log_command(cmd, None, None, ".", None);
        let mut pty = self.pty.lock().await;
        pty.write_line(cmd)?;
        Ok(ExecuteResult::SentToPty)
    }

    // ----------------------------------------------------------------
    // Neural
    // ----------------------------------------------------------------

    async fn handle_neural(&self, prompt: &str) -> Result<ExecuteResult> {
        if prompt.trim().is_empty() {
            return Ok(ExecuteResult::DirectOutput(vec![
                "Usage: !ai <your question>".to_string(),
                "  Example: !ai how do I list files recursively".to_string(),
            ]));
        }

        let mut lines = vec!["🧠 Sending to Neural...".to_string()];
        match self.neural.ask(prompt).await {
            Ok(response) => {
                for line in response.lines() {
                    lines.push(format!("  {}", line));
                }
            }
            Err(e) => {
                lines.push(format!("❌ Neural error: {}", e));
                lines.push(String::new());
                lines.push(
                    "   Check that Lemonade is running with a model loaded.".to_string(),
                );
                lines.push("   Verify at http://localhost:8000".to_string());
            }
        }
        Ok(ExecuteResult::DirectOutput(lines))
    }

    // ----------------------------------------------------------------
    // Native commands
    // ----------------------------------------------------------------

    async fn handle_native(&self, cmd: &str, args: &[String]) -> Vec<String> {
        match cmd {
            "ver" | "version" => vec![
                "⚡ Positronic v0.1.0 — Local-First Terminal".to_string(),
                "  Neural: http://localhost:8000/api/v1".to_string(),
                "  Reflex: active (50+ known typos + Levenshtein)".to_string(),
            ],

            "help" => vec![
                "⚡ Positronic Commands:".to_string(),
                String::new(),
                "  WORKING:".to_string(),
                "  ─────────────────────────────────────────".to_string(),
                "  !ver                    Version info".to_string(),
                "  !help                   This help message".to_string(),
                "  !history [query]        Search command history".to_string(),
                "  !clear / clear / cls    Clear the screen".to_string(),
                "  !ai <prompt>            Ask the Neural engine (Lemonade)".to_string(),
                "  !ask <prompt>           Alias for !ai".to_string(),
                "  !fix <command>          Check Reflex typo correction".to_string(),
                String::new(),
                "  ACTIVE FEATURES:".to_string(),
                "  ─────────────────────────────────────────".to_string(),
                "  Reflex Engine           Auto-corrects common typos".to_string(),
                "  Privacy Guard           Scrubs PII before AI queries".to_string(),
                "  Vault                   Command history stored locally".to_string(),
                String::new(),
                "  IN PROGRESS:".to_string(),
                "  ─────────────────────────────────────────".to_string(),
                "  !hive scan / status     P2P (loopback only)".to_string(),
                "  !chat <message>         Mesh chat (loopback only)".to_string(),
                "  !io scan / connect      Hardware IO (stub)".to_string(),
                "  !run / !wasm <path>     Script execution (stub)".to_string(),
                "  sandbox <cmd>           Airlock sandbox (stub)".to_string(),
                String::new(),
                "  Any other input goes to your system shell.".to_string(),
                "  Keyboard: Ctrl+A select all · Ctrl+C copy".to_string(),
            ],

            "history" => {
                let query = args.join(" ");
                let search = if query.is_empty() { "%" } else { &query };
                match self.vault.search_history(search) {
                    Ok(records) => {
                        if records.is_empty() {
                            vec!["📜 No history found.".to_string()]
                        } else {
                            let mut lines = vec![format!("📜 {} result(s):", records.len())];
                            for r in records.iter().take(25) {
                                let code = r
                                    .exit_code
                                    .map(|c| c.to_string())
                                    .unwrap_or_else(|| "?".into());
                                lines.push(format!("  [{}] {}", code, r.command));
                            }
                            lines
                        }
                    }
                    Err(e) => vec![format!("❌ History error: {}", e)],
                }
            }

            "fix" => {
                let input = args.join(" ");
                if input.is_empty() {
                    return vec![
                        "Usage: !fix <command>".to_string(),
                        "  Example: !fix gti status".to_string(),
                    ];
                }
                match self.reflex.fix_command(&input) {
                    Some(suggestion) => vec![
                        format!("💡 Suggestion: {}", suggestion.corrected),
                        format!(
                            "   Confidence: {:.0}%  Source: {:?}",
                            suggestion.confidence * 100.0,
                            suggestion.source
                        ),
                    ],
                    None => vec![format!("✅ No correction needed for: {}", input)],
                }
            }

            other => vec![
                format!("❓ Unknown command: !{}", other),
                "   Type !help for available commands.".to_string(),
            ],
        }
    }
}