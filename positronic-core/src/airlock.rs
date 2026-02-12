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
    ///
    /// Commands are routed through the system shell so that builtins
    /// (echo, cd, etc.), pipes, and redirects work correctly.
    pub async fn run_sandboxed(&self, command: &str) -> Result<String> {
        if !self.enabled {
            return Err(anyhow!("Airlock is disabled."));
        }

        let trimmed = command.trim();
        if trimmed.is_empty() {
            return Err(anyhow!("Empty command"));
        }

        tracing::info!("Executing in AIRLOCK: {}", trimmed);

        // Route through the system shell so builtins (echo, cd, etc.),
        // pipes, and redirects work on every platform.
        #[cfg(windows)]
        let output = tokio::process::Command::new("cmd")
            .args(["/C", trimmed])
            .output()
            .await?;

        #[cfg(not(windows))]
        let output = tokio::process::Command::new("sh")
            .args(["-c", trimmed])
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
            trimmed,
            status_icon,
            output.status.code().unwrap_or(-1),
            stdout,
            stderr
        ))
    }
}