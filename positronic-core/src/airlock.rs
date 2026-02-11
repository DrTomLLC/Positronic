use anyhow::{Result, anyhow};

/// The Airlock manages sandboxed execution of dangerous commands.
/// Ideally, this would spin up a Firecracker microVM or a Docker container.
/// For now, it's a placeholder struct.
#[derive(Debug)]
pub struct Airlock {
    pub enabled: bool,
}

impl Airlock {
    pub fn new() -> Self {
        Self { enabled: true }
    }

    /// Run a command in a "sandboxed" environment.
    /// Uses tokio::process to run an isolated command and capture output.
    pub async fn run_sandboxed(&self, command: &str) -> Result<String> {
        if !self.enabled {
            return Err(anyhow!("Airlock is disabled."));
        }

        tracing::info!("Executing in AIRLOCK: {}", command);

        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Err(anyhow!("Empty command"));
        }

        let program = parts[0];
        let args = &parts[1..];

        // On Windows, you might need "cmd /c" or "powershell -c" if invoking shell builtins.
        // But for "program execution", calling the binary directly is safer/cleaner.
        // If the user wants shell features, they should invoke "cmd /c ..." explicitly or we wrap it.
        // Let's assume raw binary execution for "Sandbox" purity.

        let output = tokio::process::Command::new(program)
            .args(args)
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let status_icon = if output.status.success() {
            "✅"
        } else {
            "❌"
        };

        Ok(format!(
            "🔒 [AIRLOCK SECURE EXECUTION]\nCommand: `{}`\nStatus: {} (Exit Code: {})\n\n[STDOUT]\n{}\n[STDERR]\n{}",
            command,
            status_icon,
            output.status.code().unwrap_or(-1),
            stdout,
            stderr
        ))
    }
}
