use anyhow::{Context, Result};
use std::path::Path;

pub mod wasm_host;

/// Executes a Rust script file using the installed `rust-script` binary.
///
/// # Arguments
/// * `path` - The path to the .rs file.
/// * `args` - Arguments to pass to the script itself.
pub async fn execute_script(path: &Path, args: &[String]) -> Result<String> {
    // Validate path exists before trying to run
    if !path.exists() {
        anyhow::bail!("Script file not found: {:?}", path);
    }

    // We use tokio::process::Command instead of std::process::Command
    // so we don't block the async runtime waiting for the script.
    let output = tokio::process::Command::new("rust-script")
        .arg(path)
        .args(args)
        .output()
        .await
        .context(
            "Failed to execute rust-script binary. Ensure 'cargo install rust-script' is run.",
        )?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Script execution failed:\n{}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(stdout)
}
