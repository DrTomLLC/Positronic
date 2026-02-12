use anyhow::{Context, Result};
use portable_pty::{Child, CommandBuilder, NativePtySystem, PtyPair, PtySize, PtySystem};
use std::io::{Read, Write};
use tokio::sync::mpsc;

pub struct PtyManager {
    pair: PtyPair,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send>,
}

impl std::fmt::Debug for PtyManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtyManager")
            .field("pair", &"PtyPair")
            .field("writer", &"Box<dyn Write + Send>")
            .field("child", &"Box<dyn Child + Send>")
            .finish()
    }
}

impl PtyManager {
    pub fn new(cols: u16, rows: u16) -> Result<Self> {
        let pty_system = NativePtySystem::default();
        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system.openpty(size).context("Failed to open PTY")?;

        let cmd = default_shell_command();
        let child = pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn shell process")?;

        let writer = pair
            .master
            .take_writer()
            .context("Failed to take PTY writer")?;

        Ok(Self { pair, writer, child })
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.pair
            .master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("Failed to resize PTY")
    }

    /// Back-compat: some code calls `write()`.
    pub fn write(&mut self, data: &str) -> Result<()> {
        self.write_raw(data)
    }

    /// Raw write (no newline).
    pub fn write_raw(&mut self, data: &str) -> Result<()> {
        self.writer
            .write_all(data.as_bytes())
            .context("Failed to write to PTY")?;
        self.writer.flush().context("Failed to flush PTY writer")
    }

    /// Write a line and press Enter.
    /// Windows ConPTY wants CRLF or you get “typed but not executed” behavior.
    pub fn write_line(&mut self, line: &str) -> anyhow::Result<()> {
    // Prevent double-newlines if callers already include CR/LF.
    let s = line.trim_end_matches(&['\r', '\n'][..]);

    self.write_raw(s.as_bytes())?;

    #[cfg(windows)]
    {
        // ConPTY / PowerShell expects CRLF as "Enter" reliably.
        self.write_raw(b"\r\n")?;
    }

    #[cfg(not(windows))]
    {
        self.write_raw(b"\n")?;
    }

    Ok(())
}

    /// Print output into the terminal stream (not as keystrokes).
    pub fn print_line(&mut self, text: &str) -> Result<()> {
        if cfg!(windows) {
            // PowerShell: single quotes escape by doubling
            let escaped = text.replace('\'', "''");
            self.write_line(&format!("Write-Output '{}'", escaped))?;
            return Ok(());
        }

        // POSIX shells: escape single quotes safely
        let escaped = text.replace('\'', r#"'"'"'"#);
        self.write_line(&format!("printf '%s\n' '{}'", escaped))?;
        Ok(())
    }

    pub fn child_is_alive(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => false,
            Err(_) => true,
        }
    }

    pub fn start_reader(&mut self) -> Result<mpsc::Receiver<Vec<u8>>> {
        let mut reader = self
            .pair
            .master
            .try_clone_reader()
            .context("Failed to clone PTY reader")?;

        let (tx, rx) = mpsc::channel(256);

        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.blocking_send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(rx)
    }
}

fn default_shell_command() -> CommandBuilder {
    if cfg!(windows) {
        let mut cmd = CommandBuilder::new("powershell.exe");
        cmd.arg("-NoLogo");
        cmd.arg("-NoExit");
        cmd
    } else {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        CommandBuilder::new(shell)
    }
}
