//! Command Runner — dispatches user input to the PTY or built-in handlers.
//!
//! After the pager-trap bugfix:
//! - `!clear`/`!cls` sends Ctrl+C + newline + cls to the actual PTY
//!   (previously only cleared the UI buffer).
//! - `!exit`/`!quit` are new built-in commands for graceful shutdown.

use crate::builtins;
use crate::airlock::Airlock;
use crate::pty_manager::PtyManager;

use anyhow::Result;
use positronic_hive::HiveNode;
use positronic_io::HardwareMonitor;
use positronic_neural::cortex::NeuralClient;
use positronic_script::wasm_host::WasmHost;
use crate::vault::Vault;

use std::sync::Arc;
use tokio::sync::Mutex;

// ────────────────────────────────────────────────────────────────
// Execute result
// ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ExecuteResult {
    /// Command was forwarded to the PTY shell.
    SentToPty,
    /// Built-in command produced direct output lines.
    DirectOutput(Vec<String>),
    /// Screen should be cleared.
    ClearScreen,
    /// Application should exit.
    Exit,
}

// ────────────────────────────────────────────────────────────────
// Runner
// ────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Runner {
    pub(crate) pty: Arc<Mutex<PtyManager>>,
    pub(crate) airlock: Arc<Airlock>,
    pub(crate) neural: Arc<NeuralClient>,
    pub(crate) vault: Vault,
    pub(crate) wasm_host: Arc<WasmHost>,
    pub(crate) hive: Arc<HiveNode>,
    pub(crate) io: Arc<HardwareMonitor>,
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

    pub fn vault(&self) -> &Vault {
        &self.vault
    }

    /// Main dispatch: built-in commands (`!` prefix), alias expansion, or PTY passthrough.
    pub async fn execute(&self, data: &str) -> Result<ExecuteResult> {
        let trimmed = data.trim();

        if trimmed.is_empty() {
            return Ok(ExecuteResult::SentToPty);
        }

        // Built-in commands
        if trimmed.starts_with('!') {
            return self.handle_builtin(trimmed).await;
        }

        // Alias expansion
        let final_command = if let Some(expanded) = self.expand_alias(trimmed) {
            expanded
        } else {
            trimmed.to_string()
        };

        // Send to PTY
        let mut pty = self.pty.lock().await;
        pty.write_line(&final_command)?;

        Ok(ExecuteResult::SentToPty)
    }

    /// Check whether `cmd` starts with a known alias and expand it.
    pub(crate) fn expand_alias(&self, cmd: &str) -> Option<String> {
        let first_word = cmd.split_whitespace().next()?;

        if let Ok(Some(expansion)) = self.vault.get_alias(first_word) {
            let rest = cmd.strip_prefix(first_word).unwrap_or("");
            Some(format!("{}{}", expansion, rest))
        } else {
            None
        }
    }

    /// Route `!` commands to the appropriate handler in `builtins`.
    pub(crate) async fn handle_builtin(&self, cmd: &str) -> Result<ExecuteResult> {
        builtins::dispatch(self, cmd).await
    }
}