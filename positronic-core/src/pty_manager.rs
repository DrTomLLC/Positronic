//! Cross-platform PTY manager with native implementations
//!
//! - Windows: Uses `conpty` crate (native Windows ConPTY)
//! - Unix: Uses `nix` crate (native POSIX PTY)
//!
//! This provides robust, non-blocking I/O with proper async support.

use anyhow::Result;
use tokio::sync::mpsc;

// ════════════════════════════════════════════════════════════════════
// Platform-specific implementations
// ════════════════════════════════════════════════════════════════════

#[cfg(windows)]
use windows_impl::WindowsPty as PlatformPty;

#[cfg(unix)]
use unix_impl::UnixPty as PlatformPty;

// ════════════════════════════════════════════════════════════════════
// Cross-platform PTY Manager
// ════════════════════════════════════════════════════════════════════

pub struct PtyManager {
    inner: PlatformPty,
}

impl std::fmt::Debug for PtyManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtyManager")
            .field("platform", &std::env::consts::OS)
            .finish()
    }
}

impl PtyManager {
    /// Create a new PTY with the specified dimensions
    pub fn new(cols: u16, rows: u16) -> Result<Self> {
        eprintln!("[PTY_MANAGER] Creating new PTY ({}x{}) on {}", cols, rows, std::env::consts::OS);
        let inner = PlatformPty::new(cols, rows)?;
        eprintln!("[PTY_MANAGER] PTY created successfully");
        Ok(Self { inner })
    }

    /// Resize the PTY
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        eprintln!("[PTY_MANAGER] Resizing to {}x{}", cols, rows);
        self.inner.resize(cols, rows)
    }

    /// Write raw bytes to the PTY (no newline)
    pub fn write_raw(&mut self, data: &str) -> Result<()> {
        eprintln!("[PTY_WRITER] write_raw: {} bytes", data.len());
        self.inner.write_raw(data)
    }

    /// Write a line with platform-appropriate newline
    pub fn write_line(&mut self, line: &str) -> Result<()> {
        eprintln!("[PTY_WRITER] write_line: {:?}", line);
        self.inner.write_line(line)
    }

    /// Legacy compatibility
    pub fn write(&mut self, data: &str) -> Result<()> {
        self.write_raw(data)
    }

    /// Print a line of output (executed as a command)
    pub fn print_line(&mut self, text: &str) -> Result<()> {
        eprintln!("[PTY_WRITER] print_line: {:?}", text);
        self.inner.print_line(text)
    }

    /// Check if the child process is still alive
    pub fn child_is_alive(&mut self) -> bool {
        self.inner.child_is_alive()
    }

    /// Start the async reader that pumps PTY output to a channel
    pub fn start_reader(&mut self) -> Result<mpsc::Receiver<Vec<u8>>> {
        eprintln!("[PTY_MANAGER] Starting reader pump");
        self.inner.start_reader()
    }
}

// ════════════════════════════════════════════════════════════════════
// Windows Implementation (conpty)
// ════════════════════════════════════════════════════════════════════

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use anyhow::Context;
    use std::io::{Read, Write};
    use std::sync::{Arc, Mutex};
    use tokio::sync::mpsc;

    // Wrapper to make Process Send (unsafe but needed for Arc<Mutex<>>)
    struct SendProcess(conpty::Process);
    unsafe impl Send for SendProcess {}

    pub struct WindowsPty {
        writer: Arc<Mutex<conpty::io::PipeWriter>>,
        reader: Option<conpty::io::PipeReader>,
        _process: Arc<Mutex<SendProcess>>, // Keep process alive
        cols: u16,
        rows: u16,
    }

    impl WindowsPty {
        pub fn new(cols: u16, rows: u16) -> Result<Self> {
            eprintln!("[WINDOWS_PTY] Initializing ConPTY");

            // Create ConPTY process
            let mut process = conpty::spawn("powershell.exe")
                .context("Failed to spawn ConPTY")?;

            eprintln!("[WINDOWS_PTY] Getting I/O handles");
            let reader = process.output().context("Failed to get reader")?;
            let writer = process.input().context("Failed to get writer")?;

            eprintln!("[WINDOWS_PTY] ConPTY initialized successfully");

            Ok(Self {
                writer: Arc::new(Mutex::new(writer)),
                reader: Some(reader),
                _process: Arc::new(Mutex::new(SendProcess(process))),
                cols,
                rows,
            })
        }

        pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
            self.cols = cols;
            self.rows = rows;
            eprintln!("[WINDOWS_PTY] Resize to {}x{}", cols, rows);
            Ok(())
        }

        pub fn write_raw(&mut self, data: &str) -> Result<()> {
            let mut writer = self.writer.lock().unwrap();
            writer.write_all(data.as_bytes()).context("Write failed")?;
            writer.flush().context("Flush failed")?;
            eprintln!("[WINDOWS_PTY] Wrote {} bytes", data.len());
            Ok(())
        }

        pub fn write_line(&mut self, line: &str) -> Result<()> {
            let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
            self.write_raw(trimmed)?;
            self.write_raw("\r\n")?;
            Ok(())
        }

        pub fn print_line(&mut self, text: &str) -> Result<()> {
            let escaped = text.replace('\'', "''");
            let cmd = format!("Write-Output '{}'", escaped);
            self.write_line(&cmd)
        }

        pub fn child_is_alive(&mut self) -> bool {
            true
        }

        pub fn start_reader(&mut self) -> Result<mpsc::Receiver<Vec<u8>>> {
            let mut reader = self.reader.take().context("Reader already started")?;

            let (tx, rx) = mpsc::channel(256);

            eprintln!("[WINDOWS_PTY] Spawning blocking reader thread");
            std::thread::spawn(move || {
                eprintln!("[PTY_READER] ═══════════════════════════════════════════════");
                eprintln!("[PTY_READER] Windows ConPTY reader started");
                eprintln!("[PTY_READER] ═══════════════════════════════════════════════");

                let mut buf = vec![0u8; 8192];
                let mut total_bytes = 0usize;
                let mut read_count = 0usize;

                loop {
                    read_count += 1;
                    eprintln!("[PTY_READER] Read attempt #{}", read_count);

                    match reader.read(&mut buf) {
                        Ok(0) => {
                            eprintln!("[PTY_READER] EOF after {} reads, {} bytes total", read_count, total_bytes);
                            break;
                        }
                        Ok(n) => {
                            total_bytes += n;
                            eprintln!("[PTY_READER] ───────────────────────────────────────────────");
                            eprintln!("[PTY_READER] Read #{}: {} bytes (total: {})", read_count, n, total_bytes);

                            let preview = String::from_utf8_lossy(&buf[..n.min(100)]);
                            eprintln!("[PTY_READER] Content: {:?}", preview);

                            if tx.blocking_send(buf[..n].to_vec()).is_err() {
                                eprintln!("[PTY_READER] Channel closed");
                                break;
                            }
                            eprintln!("[PTY_READER] Sent to channel");
                        }
                        Err(e) => {
                            eprintln!("[PTY_READER] Error: {} ({:?})", e, e.kind());
                            if e.kind() == std::io::ErrorKind::WouldBlock {
                                std::thread::sleep(std::time::Duration::from_millis(10));
                                continue;
                            }
                            break;
                        }
                    }
                }

                eprintln!("[PTY_READER] ═══════════════════════════════════════════════");
                eprintln!("[PTY_READER] Reader exiting ({} reads, {} bytes)", read_count, total_bytes);
                eprintln!("[PTY_READER] ═══════════════════════════════════════════════");
            });

            Ok(rx)
        }
    }
}

// ════════════════════════════════════════════════════════════════════
// Unix Implementation (nix)
// ════════════════════════════════════════════════════════════════════

#[cfg(unix)]
mod unix_impl {
    use super::*;
    use anyhow::Context;
    use nix::pty::openpty;
    use nix::unistd::{fork, ForkResult};
    use std::os::unix::io::AsRawFd;
    use tokio::sync::mpsc;

    pub struct UnixPty {
        master_fd: i32,
        child_pid: nix::unistd::Pid,
        cols: u16,
        rows: u16,
    }

    impl UnixPty {
        pub fn new(cols: u16, rows: u16) -> Result<Self> {
            eprintln!("[UNIX_PTY] Opening PTY");

            let winsize = nix::pty::Winsize {
                ws_row: rows,
                ws_col: cols,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };

            let pty_result = openpty(Some(&winsize), None).context("Failed to open PTY")?;
            let master_fd = pty_result.master;
            let slave_fd = pty_result.slave;

            eprintln!("[UNIX_PTY] Forking to spawn shell");

            match unsafe { fork() }.context("Fork failed")? {
                ForkResult::Parent { child } => {
                    nix::unistd::close(slave_fd).ok();
                    eprintln!("[UNIX_PTY] Parent process, child PID: {}", child);
                    Ok(Self {
                        master_fd,
                        child_pid: child,
                        cols,
                        rows,
                    })
                }
                ForkResult::Child => {
                    nix::unistd::setsid().expect("setsid failed");
                    nix::unistd::close(master_fd).ok();

                    nix::unistd::dup2(slave_fd, 0).expect("dup2 stdin failed");
                    nix::unistd::dup2(slave_fd, 1).expect("dup2 stdout failed");
                    nix::unistd::dup2(slave_fd, 2).expect("dup2 stderr failed");

                    if slave_fd > 2 {
                        nix::unistd::close(slave_fd).ok();
                    }

                    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
                    let c_shell = std::ffi::CString::new(shell.as_str()).unwrap();
                    nix::unistd::execv(&c_shell, &[&c_shell]).expect("exec failed");
                    std::process::exit(1);
                }
            }
        }

        pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
            self.cols = cols;
            self.rows = rows;
            eprintln!("[UNIX_PTY] Resized to {}x{}", cols, rows);
            Ok(())
        }

        pub fn write_raw(&mut self, data: &str) -> Result<()> {
            nix::unistd::write(self.master_fd, data.as_bytes()).context("Write failed")?;
            eprintln!("[UNIX_PTY] Wrote {} bytes", data.len());
            Ok(())
        }

        pub fn write_line(&mut self, line: &str) -> Result<()> {
            let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
            self.write_raw(trimmed)?;
            self.write_raw("\n")?;
            Ok(())
        }

        pub fn print_line(&mut self, text: &str) -> Result<()> {
            let escaped = text.replace('\'', r#"'"'"'"#);
            let cmd = format!("printf '%s\n' '{}'", escaped);
            self.write_line(&cmd)
        }

        pub fn child_is_alive(&mut self) -> bool {
            use nix::sys::wait::{waitpid, WaitPidFlag};
            matches!(
                waitpid(self.child_pid, Some(WaitPidFlag::WNOHANG)),
                Ok(nix::sys::wait::WaitStatus::StillAlive)
            )
        }

        pub fn start_reader(&mut self) -> Result<mpsc::Receiver<Vec<u8>>> {
            let fd = self.master_fd;
            let (tx, rx) = mpsc::channel(256);

            std::thread::spawn(move || {
                eprintln!("[PTY_READER] Unix PTY reader started");
                let mut buf = vec![0u8; 8192];
                let mut total = 0;

                loop {
                    match nix::unistd::read(fd, &mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            total += n;
                            eprintln!("[PTY_READER] Read {} bytes (total: {})", n, total);
                            if tx.blocking_send(buf[..n].to_vec()).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                eprintln!("[PTY_READER] Unix reader exiting");
            });

            Ok(rx)
        }
    }

    impl Drop for UnixPty {
        fn drop(&mut self) {
            nix::unistd::close(self.master_fd).ok();
            nix::sys::signal::kill(self.child_pid, nix::sys::signal::SIGTERM).ok();
        }
    }
}
