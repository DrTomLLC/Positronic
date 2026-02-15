//! Built-in `!` command handlers.
//!
//! Pager-trap bugfix changes:
//! - `!clear`/`!cls`: sends Ctrl+C → newline → `cls`/`clear` to the actual PTY
//!   so the PTY itself is reset, not just the UI buffer.
//! - `!exit`/`!quit`: new commands for graceful shutdown.
//! - `!help`: updated with keyboard shortcut documentation.

use crate::runner::{ExecuteResult, Runner};
use anyhow::Result;

/// Central dispatch for all `!` commands.
pub async fn dispatch(runner: &Runner, cmd: &str) -> Result<ExecuteResult> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let command = parts[0];

    match command {
        // ── Screen clear (BUGFIX: now sends control chars to PTY) ──
        "!clear" | "!cls" => {
            let mut pty = runner.pty.lock().await;
            // Break out of any pager or continuation prompt
            let _ = pty.write_raw("\x03");
            let _ = pty.write_raw("\r\n");
            // Actually clear the PTY
            if cfg!(windows) {
                let _ = pty.write_line("cls");
            } else {
                let _ = pty.write_line("clear");
            }
            Ok(ExecuteResult::ClearScreen)
        }

        // ── Graceful exit (NEW) ──
        "!exit" | "!quit" => Ok(ExecuteResult::Exit),

        // ── Help (updated with keyboard shortcuts) ──
        "!help" => {
            let help_text = vec![
                "╔══════════════════════════════════════════════════════════╗".to_string(),
                "║          Positronic Built-in Commands                   ║".to_string(),
                "╚══════════════════════════════════════════════════════════╝".to_string(),
                "".to_string(),
                "  !help              Show this help".to_string(),
                "  !clear, !cls       Clear screen (breaks pager/continuation)".to_string(),
                "  !exit, !quit       Exit Positronic".to_string(),
                "  !history [n]       Show last n commands (default: 20)".to_string(),
                "  !search <query>    Search command history".to_string(),
                "  !stats             Show vault statistics".to_string(),
                "  !top [n]           Show most-used commands (default: 10)".to_string(),
                "".to_string(),
                "  !alias             List all aliases".to_string(),
                "  !alias <n> <expansion>  Create/update alias".to_string(),
                "  !unalias <n>       Remove an alias".to_string(),
                "".to_string(),
                "  !bookmark [label]  Bookmark last command".to_string(),
                "  !bookmarks         List all bookmarks".to_string(),
                "".to_string(),
                "  !theme <n>         Change color theme (handled by UI)".to_string(),
                "  !pwd               Show current directory (handled by UI)".to_string(),
                "".to_string(),
                "  ┌─ Keyboard Shortcuts ─────────────────────────────────┐".to_string(),
                "  │  Ctrl+C           Send interrupt (break pager/cmd)   │".to_string(),
                "  │  Ctrl+Shift+C     Copy to clipboard                  │".to_string(),
                "  │  Ctrl+D           Send EOF                           │".to_string(),
                "  │  Ctrl+L           Clear screen                       │".to_string(),
                "  │  Escape           Send escape (exit vi-pager)        │".to_string(),
                "  │  Tab              Cycle completions                   │".to_string(),
                "  │  Up/Down          Navigate command history            │".to_string(),
                "  └──────────────────────────────────────────────────────┘".to_string(),
                "".to_string(),
                "  Regular shell commands are sent directly to the PTY.".to_string(),
            ];
            Ok(ExecuteResult::DirectOutput(help_text))
        }

        // ── History ──
        "!history" => {
            let limit = parts.get(1)
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(20);

            match runner.vault.recent_unique(limit) {
                Ok(history) => {
                    if history.is_empty() {
                        return Ok(ExecuteResult::DirectOutput(vec![
                            "No command history yet.".to_string()
                        ]));
                    }

                    let mut lines = vec![
                        format!("📜 Last {} unique commands:", history.len()),
                        "".to_string(),
                    ];
                    for (i, cmd) in history.iter().enumerate() {
                        lines.push(format!("  {:>3}. {}", i + 1, cmd));
                    }
                    Ok(ExecuteResult::DirectOutput(lines))
                }
                Err(e) => Ok(ExecuteResult::DirectOutput(vec![
                    format!("❌ Error reading history: {}", e)
                ])),
            }
        }

        // ── Search ──
        "!search" => {
            if parts.len() < 2 {
                return Ok(ExecuteResult::DirectOutput(vec![
                    "Usage: !search <query>".to_string(),
                ]));
            }
            let query = parts[1..].join(" ");

            match runner.vault.search_history(&query) {
                Ok(results) => {
                    if results.is_empty() {
                        return Ok(ExecuteResult::DirectOutput(vec![
                            format!("🔍 No results for '{}'", query)
                        ]));
                    }

                    let mut lines = vec![
                        format!("🔍 {} results for '{}':", results.len(), query),
                        "".to_string(),
                    ];
                    for r in &results {
                        lines.push(format!("  {} (exit {})", r.command, r.exit_code.unwrap_or(-1)));
                    }
                    Ok(ExecuteResult::DirectOutput(lines))
                }
                Err(e) => Ok(ExecuteResult::DirectOutput(vec![
                    format!("❌ Search error: {}", e)
                ])),
            }
        }

        // ── Stats ──
        "!stats" => {
            let session_count = runner.vault.session_command_count().unwrap_or(0);
            let recent = runner.vault.recent_unique(1000).unwrap_or_default();

            let lines = vec![
                "📊 Vault Statistics:".to_string(),
                "".to_string(),
                format!("  Session commands:  {}", session_count),
                format!("  Unique commands:   {}", recent.len()),
            ];
            Ok(ExecuteResult::DirectOutput(lines))
        }

        // ── Top commands ──
        "!top" => {
            let limit = parts.get(1)
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(10);

            match runner.vault.top_commands(limit) {
                Ok(top) => {
                    if top.is_empty() {
                        return Ok(ExecuteResult::DirectOutput(vec![
                            "No commands in history yet.".to_string()
                        ]));
                    }

                    let mut lines = vec![
                        format!("🏆 Top {} commands:", top.len()),
                        "".to_string(),
                    ];
                    for (i, t) in top.iter().enumerate() {
                        lines.push(format!("  {:>3}. {} (×{})", i + 1, t.command, t.count));
                    }
                    Ok(ExecuteResult::DirectOutput(lines))
                }
                Err(e) => Ok(ExecuteResult::DirectOutput(vec![
                    format!("❌ Error: {}", e)
                ])),
            }
        }

        // ── Aliases ──
        "!alias" => {
            if parts.len() < 2 {
                // List all aliases
                match runner.vault.list_aliases() {
                    Ok(aliases) => {
                        if aliases.is_empty() {
                            return Ok(ExecuteResult::DirectOutput(vec![
                                "No aliases defined.".to_string(),
                                "".to_string(),
                                "Usage: !alias <name> <expansion>".to_string(),
                            ]));
                        }

                        let mut lines = vec![
                            "📝 Aliases:".to_string(),
                            "".to_string(),
                        ];
                        for a in aliases {
                            lines.push(format!("  {} → {}", a.name, a.expansion));
                        }
                        Ok(ExecuteResult::DirectOutput(lines))
                    }
                    Err(e) => Ok(ExecuteResult::DirectOutput(vec![
                        format!("❌ Error: {}", e)
                    ])),
                }
            } else if parts.len() < 3 {
                Ok(ExecuteResult::DirectOutput(vec![
                    "Usage: !alias <name> <expansion>".to_string(),
                ]))
            } else {
                let name = parts[1];
                let expansion = parts[2..].join(" ");
                match runner.vault.set_alias(name, &expansion) {
                    Ok(_) => Ok(ExecuteResult::DirectOutput(vec![
                        format!("✓ Alias set: {} → {}", name, expansion)
                    ])),
                    Err(e) => Ok(ExecuteResult::DirectOutput(vec![
                        format!("❌ Error: {}", e)
                    ])),
                }
            }
        }

        "!unalias" => {
            if parts.len() < 2 {
                return Ok(ExecuteResult::DirectOutput(vec![
                    "Usage: !unalias <name>".to_string(),
                ]));
            }
            let name = parts[1];
            match runner.vault.remove_alias(name) {
                Ok(true) => Ok(ExecuteResult::DirectOutput(vec![
                    format!("✓ Removed alias: {}", name)
                ])),
                Ok(false) => Ok(ExecuteResult::DirectOutput(vec![
                    format!("No alias named '{}'", name)
                ])),
                Err(e) => Ok(ExecuteResult::DirectOutput(vec![
                    format!("❌ Error: {}", e)
                ])),
            }
        }

        // ── Bookmarks ──
        "!bookmark" | "!bm" => {
            let label = if parts.len() >= 2 {
                Some(parts[1..].join(" "))
            } else {
                None
            };

            match runner.vault.recent_unique(1) {
                Ok(recent) if !recent.is_empty() => {
                    let cmd = &recent[0];
                    match runner.vault.add_bookmark(cmd, label.as_deref()) {
                        Ok(_) => Ok(ExecuteResult::DirectOutput(vec![
                            format!("🔖 Bookmarked: {}", cmd)
                        ])),
                        Err(e) => Ok(ExecuteResult::DirectOutput(vec![
                            format!("❌ Error saving bookmark: {}", e)
                        ])),
                    }
                }
                Ok(_) => Ok(ExecuteResult::DirectOutput(vec![
                    "No commands to bookmark.".to_string()
                ])),
                Err(e) => Ok(ExecuteResult::DirectOutput(vec![
                    format!("❌ Error reading history: {}", e)
                ])),
            }
        }

        "!bookmarks" => {
            match runner.vault.list_bookmarks() {
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
                }
                Err(e) => Ok(ExecuteResult::DirectOutput(vec![
                    format!("❌ Error listing bookmarks: {}", e)
                ])),
            }
        }

        // ── Unknown ──
        _ => {
            Ok(ExecuteResult::DirectOutput(vec![
                format!("❌ Unknown command: {}", command),
                "".to_string(),
                "Type !help for available commands.".to_string(),
            ]))
        }
    }
}