// positronic-core/src/bin/pty_probe.rs

use anyhow::{Context, Result};
use positronic_core::pty_manager::PtyManager;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<()> {
    // Make sure *something* always prints even if tracing isn't set up elsewhere.
    eprintln!("[pty_probe] starting… (Ctrl+C to exit)");

    let cols: u16 = 120;
    let rows: u16 = 30;

    // Build PTY manager + reader before wrapping in Arc/Mutex.
    let mut pty_mgr = PtyManager::new(cols, rows).context("PtyManager::new failed")?;
    let mut rx = pty_mgr
        .start_reader()
        .context("PtyManager::start_reader failed")?;

    let pty = Arc::new(Mutex::new(pty_mgr));

    // Task: dump PTY output to stdout as raw bytes (best for Windows PTY).
    tokio::spawn(async move {
        let mut out = tokio::io::stdout();
        while let Some(bytes) = rx.recv().await {
            // Write exactly what the PTY produced.
            if out.write_all(&bytes).await.is_err() {
                break;
            }
            let _ = out.flush().await;
        }
        let _ = out.flush().await;
        eprintln!("\n[pty_probe] reader task ended");
    });

    // Kick the shell and run a few commands that work in both cmd and PowerShell.
    {
        let mut p = pty.lock().await;
        // Ensure prompt appears.
        let _ = p.write_line("");
        let _ = p.write_line("echo PTY_PROBE_OK");
        let _ = p.write_line("whoami");
        let _ = p.write_line("cd");
        let _ = p.write_line("ver");
        let _ = p.write_line("echo Type commands here. Type 'exit' to quit.");
    }

    // Interactive loop: forward stdin lines to PTY.
    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                eprintln!("\n[pty_probe] Ctrl+C received, exiting…");
                break;
            }

            line = lines.next_line() => {
                let Some(line) = line.context("stdin read failed")? else {
                    eprintln!("\n[pty_probe] stdin closed, exiting…");
                    break;
                };

                let trimmed = line.trim();
                if trimmed.eq_ignore_ascii_case("exit") {
                    eprintln!("[pty_probe] exit requested, exiting…");
                    break;
                }

                let mut p = pty.lock().await;
                p.write_line(&line).context("PTY write_line failed")?;
            }
        }
    }

    Ok(())
}
