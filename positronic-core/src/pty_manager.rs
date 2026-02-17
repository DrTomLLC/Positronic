//! Cross-platform PTY manager with native implementations
//!
//! Windows: `conpty`
//! Unix: `nix` PTY + fork/exec
//!
//! Includes minimal shell integration to emit OSC 133 (prompt markers)
//! and OSC 7 (cwd) so Intelli-Input can automatically gate itself.

use anyhow::Result;
use tokio::sync::mpsc;

#[cfg(windows)]
use windows_impl::WindowsPty as PlatformPty;

#[cfg(unix)]
use unix_impl::UnixPty as PlatformPty;

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
    pub fn new(cols: u16, rows: u16) -> Result<Self> {
        eprintln!(
            "[PTY_MANAGER] Creating new PTY ({}x{}) on {}",
            cols,
            rows,
            std::env::consts::OS
        );
        let inner = PlatformPty::new(cols, rows)?;
        eprintln!("[PTY_MANAGER] PTY created successfully");
        Ok(Self { inner })
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        eprintln!("[PTY_MANAGER] Resizing to {}x{}", cols, rows);
        self.inner.resize(cols, rows)
    }

    pub fn write_raw(&mut self, data: &str) -> Result<()> {
        eprintln!("[PTY_WRITER] write_raw: {} bytes", data.len());
        self.inner.write_raw(data)
    }

    pub fn write_line(&mut self, line: &str) -> Result<()> {
        eprintln!("[PTY_WRITER] write_line: {:?}", line);
        self.inner.write_line(line)
    }

    pub fn write(&mut self, data: &str) -> Result<()> {
        self.write_raw(data)
    }

    pub fn print_line(&mut self, text: &str) -> Result<()> {
        eprintln!("[PTY_WRITER] print_line: {:?}", text);
        self.inner.print_line(text)
    }

    pub fn child_is_alive(&mut self) -> bool {
        self.inner.child_is_alive()
    }

    pub fn start_reader(&mut self) -> Result<mpsc::Receiver<Vec<u8>>> {
        eprintln!("[PTY_MANAGER] Starting reader pump");
        self.inner.start_reader()
    }
}

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use anyhow::Context;
    use std::io::{Read, Write};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, OnceLock};
    use tokio::sync::mpsc;

    struct SendProcess(conpty::Process);
    unsafe impl Send for SendProcess {}

    static PS_PROFILE: OnceLock<PathBuf> = OnceLock::new();

    fn ensure_ps_profile() -> Result<PathBuf> {
        if let Some(p) = PS_PROFILE.get() {
            return Ok(p.clone());
        }

        let mut path = std::env::temp_dir();
        path.push("positronic_profile.ps1");

        // Write once (best-effort overwrite is fine during dev)
        let script = r#"
# Positronic PowerShell Integration
# Emits OSC 133 markers and OSC 7 cwd updates.

try { Import-Module PSReadLine -ErrorAction SilentlyContinue } catch {}

# Emit CommandStart (OSC 133;B) right before executing a line
try {
  Set-PSReadLineKeyHandler -Chord Enter -ScriptBlock {
    [Console]::Write("`e]133;B`a")
    [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()
  }
} catch {}

function global:prompt {
  # Exit code (best effort)
  $ec = 0
  try {
    if ($global:LASTEXITCODE -ne $null) { $ec = [int]$global:LASTEXITCODE }
    elseif (-not $?) { $ec = 1 }
  } catch { $ec = 0 }

  # CommandFinished (OSC 133;D;<exit>)
  [Console]::Write("`e]133;D;$ec`a")

  # CWD (OSC 7;file://localhost/<path>)
  $p = $PWD.Path
  $uri = $p -replace '\\','/'
  [Console]::Write("`e]7;file://localhost/$uri`a")

  # PromptStart (OSC 133;A)
  [Console]::Write("`e]133;A`a")

  return "PS $p> "
}
"#;

        std::fs::write(&path, script).context("Failed to write PowerShell profile")?;
        let _ = PS_PROFILE.set(path.clone());
        Ok(path)
    }

    pub struct WindowsPty {
        writer: Arc<Mutex<conpty::io::PipeWriter>>,
        reader: Option<conpty::io::PipeReader>,
        _process: Arc<Mutex<SendProcess>>,
        _cols: u16,
        _rows: u16,
    }

    impl WindowsPty {
        pub fn new(cols: u16, rows: u16) -> Result<Self> {
            eprintln!("[WINDOWS_PTY] Initializing ConPTY");

            let profile = ensure_ps_profile()?;
            let cmd = format!(
                "powershell.exe -NoLogo -NoExit -ExecutionPolicy Bypass -File \"{}\"",
                profile.display()
            );

            let mut process = conpty::spawn(&cmd).context("Failed to spawn ConPTY")?;

            eprintln!("[WINDOWS_PTY] Getting I/O handles");
            let reader = process.output().context("Failed to get reader")?;
            let writer = process.input().context("Failed to get writer")?;

            eprintln!("[WINDOWS_PTY] ConPTY initialized successfully");

            Ok(Self {
                writer: Arc::new(Mutex::new(writer)),
                reader: Some(reader),
                _process: Arc::new(Mutex::new(SendProcess(process))),
                _cols: cols,
                _rows: rows,
            })
        }

        pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
            self._cols = cols;
            self._rows = rows;
            eprintln!("[WINDOWS_PTY] Resize to {}x{}", cols, rows);
            Ok(())
        }

        pub fn write_raw(&mut self, data: &str) -> Result<()> {
            let mut writer = self.writer.lock().unwrap();
            writer.write_all(data.as_bytes()).context("Write failed")?;
            writer.flush().context("Flush failed")?;
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

            std::thread::spawn(move || {
                eprintln!("[PTY_READER] Windows ConPTY reader started");
                let mut buf = vec![0u8; 8192];

                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            if tx.blocking_send(buf[..n].to_vec()).is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::WouldBlock {
                                std::thread::sleep(std::time::Duration::from_millis(10));
                                continue;
                            }
                            break;
                        }
                    }
                }

                eprintln!("[PTY_READER] Windows reader exiting");
            });

            Ok(rx)
        }
    }
}

#[cfg(unix)]
mod unix_impl {
    use super::*;
    use anyhow::Context;
    use nix::pty::openpty;
    use nix::unistd::{fork, ForkResult};
    use std::ffi::CString;
    use std::path::PathBuf;
    use std::sync::OnceLock;
    use tokio::sync::mpsc;

    static BASH_RC: OnceLock<PathBuf> = OnceLock::new();

    fn ensure_bash_rc() -> Result<PathBuf> {
        if let Some(p) = BASH_RC.get() {
            return Ok(p.clone());
        }

        let mut path = std::env::temp_dir();
        path.push("positronic_bashrc");

        let script = r#"
# Positronic bash integration: OSC 133 markers + OSC 7 cwd
# PromptStart + CommandFinished on prompt render
__positronic_prompt() {
  local ec="$?"
  printf "\e]133;D;%s\a" "$ec"
  printf "\e]7;file://localhost%s\a" "$PWD"
  printf "\e]133;A\a"
}
PROMPT_COMMAND="__positronic_prompt"

# CommandStart before each command
trap 'printf "\e]133;B\a"' DEBUG
"#;

        std::fs::write(&path, script).context("Failed to write bash rc")?;
        let _ = BASH_RC.set(path.clone());
        Ok(path)
    }

    pub struct UnixPty {
        master_fd: i32,
        child_pid: nix::unistd::Pid,
        _cols: u16,
        _rows: u16,
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

            match unsafe { fork() }.context("Fork failed")? {
                ForkResult::Parent { child } => {
                    nix::unistd::close(slave_fd).ok();
                    Ok(Self {
                        master_fd,
                        child_pid: child,
                        _cols: cols,
                        _rows: rows,
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

                    if shell.ends_with("bash") {
                        if let Ok(rc) = ensure_bash_rc() {
                            let bash = CString::new(shell.as_str()).unwrap();
                            let arg0 = bash.clone();
                            let i = CString::new("-i").unwrap();
                            let noprofile = CString::new("--noprofile").unwrap();
                            let rcfile = CString::new("--rcfile").unwrap();
                            let rcpath = CString::new(rc.to_string_lossy().to_string()).unwrap();
                            nix::unistd::execv(
                                &bash,
                                &[&arg0, &i, &noprofile, &rcfile, &rcpath],
                            )
                                .expect("exec bash failed");
                        }
                    }

                    let c_shell = CString::new(shell.as_str()).unwrap();
                    nix::unistd::execv(&c_shell, &[&c_shell]).expect("exec failed");
                    std::process::exit(1);
                }
            }
        }

        pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
            self._cols = cols;
            self._rows = rows;
            Ok(())
        }

        pub fn write_raw(&mut self, data: &str) -> Result<()> {
            nix::unistd::write(self.master_fd, data.as_bytes()).context("Write failed")?;
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
                let mut buf = vec![0u8; 8192];
                loop {
                    match nix::unistd::read(fd, &mut buf) {
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

    impl Drop for UnixPty {
        fn drop(&mut self) {
            nix::unistd::close(self.master_fd).ok();
            let _ = nix::sys::signal::kill(self.child_pid, nix::sys::signal::SIGTERM);
        }
    }
}
