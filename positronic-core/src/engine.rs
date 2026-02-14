use crate::airlock::Airlock;
use crate::pty_manager::PtyManager;
use crate::state_machine::StateMachine;
use crate::vault::Vault;

use anyhow::{Context, Result};
use positronic_hive::{HiveEvent, HiveNode};
use positronic_io::{HardwareEvent, HardwareMonitor};
use positronic_neural::cortex::NeuralClient;
use positronic_script::wasm_host::WasmHost;

use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

#[derive(Debug, Clone)]
pub enum ExecuteResult {
    SentToPty,
    DirectOutput(Vec<String>),
    ClearScreen,
}

#[derive(Debug)]
pub struct Runner {
    pty: Arc<Mutex<PtyManager>>,
    airlock: Arc<Airlock>,
    neural: Arc<NeuralClient>,
    vault: Vault,
    wasm_host: Arc<WasmHost>,
    hive: Arc<HiveNode>,
    io: Arc<HardwareMonitor>,
}

impl Runner {
    pub fn new(
        pty: Arc<Mutex<PtyManager>>,
        airlock: Arc<Airlock>,
        neural: Arc<NeuralClient>,
        vault: Vault,
        wasm_host: Arc<WasmHost>,
        hive: Arc<HiveNode>,
        io: Arc<HardwareMonitor>,
    ) -> Self {
        Self {
            pty,
            airlock,
            neural,
            vault,
            wasm_host,
            hive,
            io,
        }
    }

    pub fn vault(&self) -> &Vault {
        &self.vault
    }

    pub async fn execute(&self, data: &str) -> Result<ExecuteResult> {
        let trimmed = data.trim();

        if trimmed.is_empty() {
            return Ok(ExecuteResult::SentToPty);
        }

        if trimmed.starts_with('!') {
            return self.handle_builtin(trimmed).await;
        }

        let final_command = if let Some(expanded) = self.expand_alias(trimmed) {
            expanded
        } else {
            trimmed.to_string()
        };

        let mut pty = self.pty.lock().await;
        pty.write_line(&final_command)?;

        Ok(ExecuteResult::SentToPty)
    }

    fn expand_alias(&self, cmd: &str) -> Option<String> {
        let first_word = cmd.split_whitespace().next()?;

        if let Ok(Some(expansion)) = self.vault.get_alias(first_word) {
            let rest = cmd.strip_prefix(first_word).unwrap_or("");
            Some(format!("{}{}", expansion, rest))
        } else {
            None
        }
    }

    async fn handle_builtin(&self, cmd: &str) -> Result<ExecuteResult> {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let command = parts[0];

        match command {
            "!clear" | "!cls" => Ok(ExecuteResult::ClearScreen),

            "!help" => {
                let help_text = vec![
                    "╔══════════════════════════════════════════════════════════╗".to_string(),
                    "║          Positronic Built-in Commands                   ║".to_string(),
                    "╚══════════════════════════════════════════════════════════╝".to_string(),
                    "".to_string(),
                    "  !help              Show this help".to_string(),
                    "  !clear, !cls       Clear the screen".to_string(),
                    "  !history [n]       Show last n commands (default: 20)".to_string(),
                    "  !search <query>    Search command history".to_string(),
                    "  !stats             Show vault statistics".to_string(),
                    "  !top [n]           Show most-used commands (default: 10)".to_string(),
                    "".to_string(),
                    "  !alias             List all aliases".to_string(),
                    "  !alias <name> <expansion>  Create/update alias".to_string(),
                    "  !unalias <name>    Remove an alias".to_string(),
                    "".to_string(),
                    "  !bookmark [label]  Bookmark last command".to_string(),
                    "  !bookmarks         List all bookmarks".to_string(),
                    "".to_string(),
                    "  !theme <name>      Change color theme (handled by UI)".to_string(),
                    "  !pwd               Show current directory (handled by UI)".to_string(),
                    "".to_string(),
                    "  Regular shell commands are sent directly to the PTY.".to_string(),
                ];
                Ok(ExecuteResult::DirectOutput(help_text))
            },

            "!history" => {
                let limit = parts.get(1)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(20);

                match self.vault.recent_unique(limit) {
                    Ok(history) => {
                        if history.is_empty() {
                            return Ok(ExecuteResult::DirectOutput(vec![
                                "No command history yet.".to_string()
                            ]));
                        }

                        let mut lines = vec![
                            format!("📜 Last {} unique commands:", limit),
                            "".to_string(),
                        ];
                        for (i, cmd) in history.iter().enumerate() {
                            lines.push(format!("  {:3}  {}", i + 1, cmd));
                        }
                        Ok(ExecuteResult::DirectOutput(lines))
                    },
                    Err(e) => Ok(ExecuteResult::DirectOutput(vec![
                        format!("❌ Error reading history: {}", e)
                    ])),
                }
            },

            "!search" => {
                if parts.len() < 2 {
                    return Ok(ExecuteResult::DirectOutput(vec![
                        "Usage: !search <query>".to_string()
                    ]));
                }

                let query = parts[1..].join(" ");
                match self.vault.search_history(&query) {
                    Ok(results) => {
                        if results.is_empty() {
                            return Ok(ExecuteResult::DirectOutput(vec![
                                format!("No matches found for: {}", query)
                            ]));
                        }

                        let mut lines = vec![
                            format!("🔍 Search results for '{}': {} matches", query, results.len()),
                            "".to_string(),
                        ];
                        for record in results.iter().take(20) {
                            let time = chrono::DateTime::from_timestamp(record.timestamp, 0)
                                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                                .unwrap_or_else(|| "unknown".to_string());
                            lines.push(format!("  [{}] {}", time, record.command));
                        }
                        if results.len() > 20 {
                            lines.push(format!("  ... and {} more", results.len() - 20));
                        }
                        Ok(ExecuteResult::DirectOutput(lines))
                    },
                    Err(e) => Ok(ExecuteResult::DirectOutput(vec![
                        format!("❌ Error searching history: {}", e)
                    ])),
                }
            },

            "!top" => {
                let limit = parts.get(1)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(10);

                match self.vault.top_commands(limit) {
                    Ok(top) => {
                        if top.is_empty() {
                            return Ok(ExecuteResult::DirectOutput(vec![
                                "No command history yet.".to_string()
                            ]));
                        }

                        let mut lines = vec![
                            format!("🏆 Top {} most-used commands:", limit),
                            "".to_string(),
                        ];
                        for (i, cmd) in top.iter().enumerate() {
                            lines.push(format!("  {:2}. {:4}× {}", i + 1, cmd.count, cmd.command));
                        }
                        Ok(ExecuteResult::DirectOutput(lines))
                    },
                    Err(e) => Ok(ExecuteResult::DirectOutput(vec![
                        format!("❌ Error reading top commands: {}", e)
                    ])),
                }
            },

            "!stats" => {
                match self.vault.stats() {
                    Ok(stats) => {
                        let earliest = stats.earliest_timestamp
                            .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
                            .map(|dt| dt.format("%Y-%m-%d").to_string())
                            .unwrap_or_else(|| "N/A".to_string());

                        let lines = vec![
                            "╔══════════════════════════════════════════════════════════╗".to_string(),
                            "║                   Vault Statistics                      ║".to_string(),
                            "╚══════════════════════════════════════════════════════════╝".to_string(),
                            "".to_string(),
                            format!("  Total commands:      {:8}", stats.total_commands),
                            format!("  Session commands:    {:8}", stats.session_commands),
                            format!("  Unique commands:     {:8}", stats.unique_commands),
                            format!("  Total sessions:      {:8}", stats.total_sessions),
                            "".to_string(),
                            format!("  Aliases defined:     {:8}", stats.alias_count),
                            format!("  Bookmarks saved:     {:8}", stats.bookmark_count),
                            "".to_string(),
                            format!("  First command:       {}", earliest),
                            format!("  Database size:       {:8} KB", stats.db_size_bytes / 1024),
                        ];
                        Ok(ExecuteResult::DirectOutput(lines))
                    },
                    Err(e) => Ok(ExecuteResult::DirectOutput(vec![
                        format!("❌ Error reading stats: {}", e)
                    ])),
                }
            },

            "!alias" => {
                if parts.len() == 1 {
                    match self.vault.list_aliases() {
                        Ok(aliases) => {
                            if aliases.is_empty() {
                                Ok(ExecuteResult::DirectOutput(vec![
                                    "No aliases defined.".to_string(),
                                    "".to_string(),
                                    "Usage: !alias <name> <expansion>".to_string(),
                                ]))
                            } else {
                                let mut lines = vec![
                                    "📝 Defined aliases:".to_string(),
                                    "".to_string(),
                                ];
                                for alias in aliases {
                                    lines.push(format!("  {} = {}", alias.name, alias.expansion));
                                }
                                Ok(ExecuteResult::DirectOutput(lines))
                            }
                        },
                        Err(e) => Ok(ExecuteResult::DirectOutput(vec![
                            format!("❌ Error listing aliases: {}", e)
                        ])),
                    }
                } else if parts.len() >= 3 {
                    let name = parts[1];
                    let expansion = parts[2..].join(" ");
                    match self.vault.set_alias(name, &expansion) {
                        Ok(_) => Ok(ExecuteResult::DirectOutput(vec![
                            format!("✓ Alias set: {} = {}", name, expansion)
                        ])),
                        Err(e) => Ok(ExecuteResult::DirectOutput(vec![
                            format!("❌ Error setting alias: {}", e)
                        ])),
                    }
                } else {
                    Ok(ExecuteResult::DirectOutput(vec![
                        "Usage: !alias [name expansion...]".to_string(),
                        "".to_string(),
                        "  !alias              List all aliases".to_string(),
                        "  !alias ll ls -la    Create alias".to_string(),
                    ]))
                }
            },

            "!unalias" => {
                if parts.len() != 2 {
                    return Ok(ExecuteResult::DirectOutput(vec![
                        "Usage: !unalias <name>".to_string()
                    ]));
                }

                let name = parts[1];
                match self.vault.remove_alias(name) {
                    Ok(true) => Ok(ExecuteResult::DirectOutput(vec![
                        format!("✓ Alias removed: {}", name)
                    ])),
                    Ok(false) => Ok(ExecuteResult::DirectOutput(vec![
                        format!("Alias not found: {}", name)
                    ])),
                    Err(e) => Ok(ExecuteResult::DirectOutput(vec![
                        format!("❌ Error removing alias: {}", e)
                    ])),
                }
            },

            "!bookmark" => {
                let label = if parts.len() > 1 {
                    Some(parts[1..].join(" "))
                } else {
                    None
                };

                match self.vault.recent_unique(1) {
                    Ok(history) if !history.is_empty() => {
                        let last_cmd = &history[0];
                        match self.vault.add_bookmark(last_cmd, label.as_deref()) {
                            Ok(id) => Ok(ExecuteResult::DirectOutput(vec![
                                format!("✓ Bookmarked (#{}):", id),
                                format!("  {}", last_cmd),
                                if let Some(l) = label {
                                    format!("  Label: {}", l)
                                } else {
                                    "".to_string()
                                }
                            ])),
                            Err(e) => Ok(ExecuteResult::DirectOutput(vec![
                                format!("❌ Error saving bookmark: {}", e)
                            ])),
                        }
                    },
                    Ok(_) => Ok(ExecuteResult::DirectOutput(vec![
                        "No commands to bookmark.".to_string()
                    ])),
                    Err(e) => Ok(ExecuteResult::DirectOutput(vec![
                        format!("❌ Error reading history: {}", e)
                    ])),
                }
            },

            "!bookmarks" => {
                match self.vault.list_bookmarks() {
                    Ok(bookmarks) => {
                        if bookmarks.is_empty() {
                            return Ok(ExecuteResult::DirectOutput(vec![
                                "No bookmarks saved.".to_string(),
                                "".to_string(),
                                "Usage: !bookmark [label]  (bookmarks last command)".to_string(),
                            ]));
                        }

                        let mut lines = vec![
                            "🔖 Saved bookmarks:".to_string(),
                            "".to_string(),
                        ];
                        for bm in bookmarks {
                            let label_str = bm.label
                                .map(|l| format!(" [{}]", l))
                                .unwrap_or_default();
                            lines.push(format!("  #{}{}:", bm.id, label_str));
                            lines.push(format!("    {}", bm.command));
                        }
                        Ok(ExecuteResult::DirectOutput(lines))
                    },
                    Err(e) => Ok(ExecuteResult::DirectOutput(vec![
                        format!("❌ Error listing bookmarks: {}", e)
                    ])),
                }
            },

            _ => {
                Ok(ExecuteResult::DirectOutput(vec![
                    format!("❌ Unknown command: {}", command),
                    "".to_string(),
                    "Type !help for available commands.".to_string(),
                ]))
            }
        }
    }
}

#[derive(Debug)]
pub struct PositronicEngine {
    pub pty: Arc<Mutex<PtyManager>>,
    pub state: Arc<StateMachine>,
    pub runner: Arc<Runner>,
    pub airlock: Arc<Airlock>,
    pub pty_output_buf: Arc<std::sync::Mutex<Vec<u8>>>,
    redraw_notifier: mpsc::Sender<()>,
}

impl PositronicEngine {
    pub async fn start(cols: u16, rows: u16, redraw_tx: mpsc::Sender<()>) -> Result<Self> {
        let mut pty_manager = PtyManager::new(cols, rows).context("Failed to create PTY")?;
        let mut rx_ptr = pty_manager
            .start_reader()
            .context("Failed to start PTY reader")?;

        let pty = Arc::new(Mutex::new(pty_manager));
        let state = Arc::new(StateMachine::new(cols, rows));
        let pty_output_buf: Arc<std::sync::Mutex<Vec<u8>>> =
            Arc::new(std::sync::Mutex::new(Vec::with_capacity(8192)));

        {
            let state_clone = state.clone();
            let buf_clone = pty_output_buf.clone();
            let notifier = redraw_tx.clone();
            tokio::spawn(async move {
                while let Some(bytes) = rx_ptr.recv().await {
                    if let Ok(mut buf) = buf_clone.lock() {
                        buf.extend_from_slice(&bytes);
                    }
                    state_clone.process_bytes(&bytes);

                    while let Ok(more) = rx_ptr.try_recv() {
                        if let Ok(mut buf) = buf_clone.lock() {
                            buf.extend_from_slice(&more);
                        }
                        state_clone.process_bytes(&more);
                    }
                    let _ = notifier.try_send(());
                }
            });
        }

        {
            let mut p = pty.lock().await;
            let _ = p.write_line("");
        }
        let _ = redraw_tx.try_send(());

        let airlock = Arc::new(Airlock::new());

        let neural = Arc::new(NeuralClient::new(
            "http://localhost:8000/api/v1",
            "auto",
        ));

        let vault = Vault::open("positronic.db").context("Failed to open Vault")?;
        let wasm_host = Arc::new(WasmHost::new().context("Failed to init WASM host")?);

        let (hive_node, mut hive_rx) = HiveNode::new("PositronicUser");
        let hive = Arc::new(hive_node);

        {
            let pty_for_hive = pty.clone();
            let (tx, mut rx) = mpsc::channel::<String>(32);

            tokio::spawn(async move {
                while let Some(cmd) = rx.recv().await {
                    let mut p = pty_for_hive.lock().await;
                    let _ = p.write_line(&cmd);
                }
            });

            tokio::spawn(async move {
                while let Ok(event) = hive_rx.recv().await {
                    let msg = match event {
                        HiveEvent::PeerDiscovered { peer_id, name } => {
                            format!("📡 Peer: {} ({})", name, peer_id)
                        }
                        HiveEvent::PeerLost { peer_id } => {
                            format!("📡 Peer Lost: {}", peer_id)
                        }
                        HiveEvent::BlockReceived { from, content } => {
                            let text = String::from_utf8_lossy(&content);
                            format!("💬 [{}]: {}", from, text)
                        }
                        HiveEvent::LiveSessionInvite { from, session_id } => {
                            format!("📞 Invite from {}: {}", from, session_id)
                        }
                        HiveEvent::Error(e) => format!("⚠️ Hive: {}", e),
                    };
                    let _ = tx.send(shell_echo_cmd(&msg)).await;
                }
            });
        }

        let (hardware_monitor, mut io_rx) = HardwareMonitor::start();
        let io = Arc::new(hardware_monitor);

        {
            let pty_for_io = pty.clone();
            tokio::spawn(async move {
                while let Some(event) = io_rx.recv().await {
                    let msg = match event {
                        HardwareEvent::DeviceConnected(n) => format!("🔌 Connected: {}", n),
                        HardwareEvent::DeviceDisconnected(n) => format!("🔌 Disconnected: {}", n),
                        HardwareEvent::DataBatch(_) => continue,
                        HardwareEvent::SerialOutput(s) => s,
                        HardwareEvent::Error(e) => format!("⚠️ IO: {}", e),
                    };
                    let mut p = pty_for_io.lock().await;
                    let _ = p.write_line(&shell_echo_cmd(&msg));
                }
            });
        }

        let runner = Arc::new(Runner::new(
            pty.clone(),
            airlock.clone(),
            neural,
            vault,
            wasm_host,
            hive,
            io,
        ));

        Ok(Self {
            pty,
            state,
            runner,
            airlock,
            pty_output_buf,
            redraw_notifier: redraw_tx,
        })
    }

    pub fn drain_pty_output(&self) -> Vec<u8> {
        match self.pty_output_buf.lock() {
            Ok(mut buf) => std::mem::take(&mut *buf),
            Err(poisoned) => {
                let mut buf = poisoned.into_inner();
                std::mem::take(&mut *buf)
            }
        }
    }

    pub async fn send_input(&self, data: &str) -> Result<ExecuteResult> {
        self.runner.execute(data).await
    }

    pub async fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        let mut pty = self.pty.lock().await;
        pty.resize(cols, rows)?;
        self.state.resize(cols, rows);
        let _ = self.redraw_notifier.try_send(());
        Ok(())
    }
}

fn shell_echo_cmd(text: &str) -> String {
    if cfg!(windows) {
        let escaped = text.replace('\'', "''");
        format!("Write-Output '{}'", escaped)
    } else {
        let escaped = text.replace('\'', r#"'"'"'"#);
        format!("printf '%s\\n' '{}'", escaped)
    }
}
