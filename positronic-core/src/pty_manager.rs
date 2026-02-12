use anyhow::{Context, Result};
use portable_pty::{Child, CommandBuilder, NativePtySystem, PtyPair, PtySize, PtySystem};
use std::borrow::Cow;
use std::io::{Read, Write};
use tokio::sync::mpsc;

/// Manages the OS-level Pseudo-Terminal (PTY).
pub struct PtyManager {
    pair: PtyPair,
    writer: Box<dyn Write + Send>,
    /// IMPORTANT: Keep the spawned child handle alive.
    /// Dropping this early can cause the PTY session to die (notably on Windows/ConPTY).
    child: Box<dyn Child + Send>,
    reader_started: bool,
}

impl std::fmt::Debug for PtyManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtyManager")
            .field("pair", &"PtyPair")
            .field("writer", &"Box<dyn Write>")
            .field("child", &"Box<dyn Child>")
            .field("reader_started", &self.reader_started)
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

        let pair = pty_system
            .openpty(size)
            .context("Failed to open PTY system")?;

        // Choose a reasonable default shell.
        let mut cmd = default_shell_command();

        // Spawn the shell into the PTY *and keep the Child handle*.
        let child = pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn shell process into PTY")?;

        // Keep a writer for stdin->pty.
        let writer = pair
            .master
            .take_writer()
            .context("Failed to take PTY writer")?;

        Ok(Self {
            pair,
            writer,
            child,
            reader_started: false,
        })
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.pair
            .master
            .resize(PtySize {
                rows,
                cols,
                ..Default::default()
            })
            .context("Failed to resize PTY")
    }

    pub fn write(&mut self, data: &str) -> Result<()> {
        let data = normalize_newlines(data);
        self.writer
            .write_all(data.as_bytes())
            .context("Failed to write to PTY")?;
        self.writer.flush().context("Failed to flush PTY writer")
    }

    /// Starts a background thread that reads the PTY output and forwards it into a tokio channel.
    pub fn start_reader(&mut self) -> Result<mpsc::Receiver<Vec<u8>>> {
        if self.reader_started {
            anyhow::bail!("PTY reader already started");
        }
        self.reader_started = true;

        let mut reader = self
            .pair
            .master
            .try_clone_reader()
            .context("Failed to clone PTY reader")?;

        let (tx, rx) = mpsc::channel(256);

        std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        let bytes = buffer[..n].to_vec();
                        if tx.blocking_send(bytes).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(rx)
    }

    /// Debug helper: is the shell still alive?
    pub fn child_is_alive(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => false,
            Err(_) => false,
        }
    }
}

fn default_shell_command() -> CommandBuilder {
    #[cfg(windows)]
    {
        // Prefer PowerShell (Windows PowerShell) by default.
        // -NoExit helps keep it interactive inside PTY.
        let mut cmd = CommandBuilder::new("powershell.exe");
        cmd.arg("-NoLogo");
        cmd.arg("-NoExit");
        cmd.arg("-NoProfile");
        cmd
    }

    #[cfg(not(windows))]
    {
        // Use $SHELL if present, otherwise /bin/sh.
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        CommandBuilder::new(shell)
    }
}

/// On Windows, many console apps behave better with CRLF.
/// This converts lone '\n' into "\r\n" while preserving existing "\r\n".
fn normalize_newlines(s: &str) -> Cow<'_, str> {
    #[cfg(windows)]
    {
        if !s.contains('\n') {
            return Cow::Borrowed(s);
        }

        let mut out = String::with_capacity(s.len() + 8);
        let mut prev_was_cr = false;

        for ch in s.chars() {
            match ch {
                '\r' => {
                    prev_was_cr = true;
                    out.push('\r');
                }
                '\n' => {
                    if prev_was_cr {
                        // already have \r\n
                        out.push('\n');
                    } else {
                        out.push('\r');
                        out.push('\n');
                    }
                    prev_was_cr = false;
                }
                _ => {
                    prev_was_cr = false;
                    out.push(ch);
                }
            }
        }

        Cow::Owned(out)
    }

    #[cfg(not(windows))]
    {
        Cow::Borrowed(s)
    }
}
