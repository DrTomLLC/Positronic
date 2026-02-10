use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem, MasterPty, PtyPair};
use anyhow::{Result, Context};
use std::sync::{Arc, Mutex};
use std::io::{Read, Write};
use tokio::sync::mpsc;

/// Manages the OS-level Pseudo-Terminal (ConPTY on Windows).
pub struct PtyManager {
    pair: PtyPair,
    writer: Box<dyn Write + Send>,
}

impl PtyManager {
    /// Spawns a new shell (PowerShell/CMD/WSL).
    pub fn new(cols: u16, rows: u16) -> Result<Self> {
        let pty_system = NativePtySystem::default();
        
        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        // Open the PTY pair
        let pair = pty_system.openpty(size)
            .context("Failed to open PTY system")?;

        // Determine the shell (Default to PowerShell on Windows)
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "powershell.exe".to_string());
        let cmd = CommandBuilder::new(shell);

        // Spawn the shell process detached
        let _child = pair.slave.spawn_command(cmd)
            .context("Failed to spawn shell process")?;

        // We clone the writer immediately because we need to keep 'pair' for resizing
        let writer = pair.master.try_clone_writer()
            .context("Failed to clone PTY writer")?;

        Ok(Self {
            pair,
            writer,
        })
    }

    /// Resizes the PTY (call this when window resizes).
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.pair.master.resize(PtySize {
            rows,
            cols,
            ..Default::default()
        }).context("Failed to resize PTY")
    }

    /// Writes input (user typing) to the shell.
    pub fn write(&mut self, data: &str) -> Result<()> {
        write!(self.writer, "{}", data).context("Failed to write to PTY")
    }

    /// Starts a background thread to read output and send it to the UI.
    /// Returns a Receiver channel that the UI listens to.
    pub fn start_reader(&mut self) -> Result<mpsc::Receiver<Vec<u8>>> {
        let mut reader = self.pair.master.try_clone_reader()
            .context("Failed to clone PTY reader")?;
            
        let (tx, rx) = mpsc::channel(100);

        std::thread::spawn(move || {
            let mut buffer = [0u8; 4096]; // 4KB chunks
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break, // EOF (Shell closed)
                    Ok(n) => {
                        let bytes = buffer[0..n].to_vec();
                        if tx.blocking_send(bytes).is_err() {
                            break; // UI closed, stop reading
                        }
                    }
                    Err(_) => break, // Read error
                }
            }
        });

        Ok(rx)
    }
}
