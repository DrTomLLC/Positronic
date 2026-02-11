use anyhow::{Context, Result};
use portable_pty::{CommandBuilder, NativePtySystem, PtyPair, PtySize, PtySystem};
use std::io::{Read, Write};
use tokio::sync::mpsc;

/// Manages the OS-level Pseudo-Terminal.
/// Manages the OS-level Pseudo-Terminal.
pub struct PtyManager {
    pair: PtyPair,
    writer: Box<dyn Write + Send>,
}

impl std::fmt::Debug for PtyManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtyManager")
            .field("pair", &"PtyPair")
            .field("writer", &"Box<dyn Write>")
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

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "powershell.exe".to_string());
        let cmd = CommandBuilder::new(shell);

        let _child = pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn shell process")?;

        // FIX: portable-pty's `take_writer` gives us a writer we can keep.
        let writer = pair
            .master
            .take_writer()
            .context("Failed to take PTY writer")?;

        Ok(Self { pair, writer })
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
        write!(self.writer, "{}", data).context("Failed to write to PTY")?;
        self.writer.flush().context("Failed to flush PTY writer")
    }

    pub fn start_reader(&mut self) -> Result<mpsc::Receiver<Vec<u8>>> {
        // FIX: portable-pty readers are tricky. We use try_clone_reader.
        let mut reader = self
            .pair
            .master
            .try_clone_reader()
            .context("Failed to clone PTY reader")?;

        let (tx, rx) = mpsc::channel(100);

        std::thread::spawn(move || {
            let mut buffer = [0u8; 4096];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        let bytes = buffer[0..n].to_vec();
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
}
