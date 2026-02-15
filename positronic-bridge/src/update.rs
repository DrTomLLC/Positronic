//! Update logic — the central message handler.
//!
//! Pager-trap bugfix changes:
//! - `ClearScreen`: sends interrupt → flush → cls to the actual PTY.
//! - `SendInterrupt` / `SendEscape` / `SendEof`: new handlers that forward
//!   control characters directly to the PTY.
//! - `InputSent` for `!exit`/`!quit`: triggers graceful shutdown.

use crate::app::{AppState, PositronicApp};
use crate::completer;
use crate::cwd::{track_cd_command, update_cwd_from_snapshot};
use crate::helpers::{get_alias_names, hash_snapshot};
use crate::messages::{push_direct, Message};
use crate::renderer::ThemeName;

use positronic_core::engine::ExecuteResult;

use std::time::Duration;

use iced::Task;

pub fn update(app: &mut PositronicApp, message: Message) -> Task<Message> {
    match message {
        // ────────────────────────────────────────────────────
        // Engine lifecycle
        // ────────────────────────────────────────────────────

        Message::EngineReady(engine, redraw) => {
            eprintln!("[UI] Engine ready!");

            // Hydrate persistent history from the Vault
            match engine.runner.vault().recent_unique(100) {
                Ok(history) => {
                    app.cmd_history = history.into_iter().rev().collect::<Vec<String>>();
                    eprintln!("[UI] Hydrated {} commands from Vault.", app.cmd_history.len());
                }
                Err(e) => eprintln!("[UI] Failed to load history: {}", e),
            }

            app.engine = Some(engine.clone());
            app.redraw = Some(redraw);
            app.state = AppState::Active;
            app.direct_output.clear();
            app.last_snapshot = None;

            // Load saved theme from vault config
            if let Ok(Some(saved_theme)) = engine.runner.vault().get_config("theme") {
                if let Some(t) = ThemeName::from_str(&saved_theme) {
                    app.theme_name = t;
                }
            }

            push_direct(app, "⚡ Positronic v0.2.0 Online.  Type !help for commands.");
            Task::none()
        }

        Message::EngineFailed(err) => {
            app.state = AppState::Error(err.clone());
            push_direct(app, &format!("❌ BOOT FAILED: {}", err));
            Task::none()
        }

        // ────────────────────────────────────────────────────
        // PTY redraw
        // ────────────────────────────────────────────────────

        Message::Redraw => {
            if let Some(engine) = &app.engine {
                let snapshot = engine.state.snapshot();
                let new_hash = hash_snapshot(&snapshot);

                if new_hash != app.last_screen_hash {
                    app.last_screen_hash = new_hash;

                    // CWD tracking from prompt
                    update_cwd_from_snapshot(&snapshot, &mut app.cwd);

                    app.last_snapshot = Some(snapshot);

                    // Clear boot message once PTY is active
                    if !app.direct_output.is_empty() && app.direct_output.starts_with("⚡") {
                        app.direct_output.clear();
                    }
                }
            }
            Task::none()
        }

        // ────────────────────────────────────────────────────
        // Input
        // ────────────────────────────────────────────────────

        Message::InputChanged(val) => {
            app.input = val;
            app.history_cursor = None;
            app.tab_state = None;
            Task::none()
        }

        Message::InputSent => {
            let trimmed = app.input.trim().to_string();
            if trimmed.is_empty() {
                return Task::none();
            }

            app.cmd_history.push(trimmed.clone());
            app.history_cursor = None;
            app.tab_state = None;
            app.session_cmd_count += 1;
            app.input.clear();

            // CWD tracking from cd commands
            track_cd_command(&trimmed, &mut app.cwd);

            // ── Local theme handling (no runner round-trip needed) ──
            if trimmed.starts_with("!theme") {
                let args: Vec<&str> = trimmed.split_whitespace().collect();
                if args.len() < 2 {
                    let names: Vec<&str> = ThemeName::all().iter().map(|t| t.label()).collect();
                    push_direct(app, &format!("🎨 Current theme: {}", app.theme_name.label()));
                    push_direct(app, &format!("   Available: {}", names.join(", ")));
                } else if let Some(new_theme) = ThemeName::from_str(args[1]) {
                    app.theme_name = new_theme;
                    push_direct(app, &format!("🎨 Theme switched to: {}", new_theme.label()));
                    if let Some(engine) = &app.engine {
                        let _ = engine.runner.vault().set_config("theme", new_theme.label());
                    }
                } else {
                    push_direct(app, &format!("❌ Unknown theme: {}", args[1]));
                    let names: Vec<&str> = ThemeName::all().iter().map(|t| t.label()).collect();
                    push_direct(app, &format!("   Available: {}", names.join(", ")));
                }
                return Task::none();
            }

            // ── Local !pwd handling ──
            if trimmed == "!pwd" {
                push_direct(app, &format!("📂 {}", app.cwd));
                return Task::none();
            }

            // ── !exit / !quit — handled locally for immediate shutdown ──
            if trimmed == "!exit" || trimmed == "!quit" {
                push_direct(app, "👋 Exiting Positronic...");
                // Use iced::exit() to cleanly shut down the application
                return iced::exit();
            }

            let Some(engine) = app.engine.clone() else {
                push_direct(app, "❌ Engine not ready.");
                return Task::none();
            };

            Task::perform(
                async move { engine.send_input(&trimmed).await },
                move |r| match r {
                    Ok(result) => Message::CommandResult(result),
                    Err(e) => Message::CommandError(format!("{:#}", e)),
                },
            )
        }

        Message::CommandResult(result) => {
            match result {
                ExecuteResult::SentToPty => {}
                ExecuteResult::DirectOutput(lines) => {
                    push_direct(app, &lines.join("\n"));
                }
                ExecuteResult::ClearScreen => {
                    app.direct_output.clear();
                    app.last_snapshot = None;
                }
                ExecuteResult::Exit => {
                    push_direct(app, "👋 Exiting Positronic...");
                    return iced::exit();
                }
            }
            Task::none()
        }

        Message::CommandError(err) => {
            push_direct(app, &format!("❌ {}", err));
            Task::none()
        }

        // ────────────────────────────────────────────────────
        // History navigation
        // ────────────────────────────────────────────────────

        Message::HistoryUp => {
            if app.cmd_history.is_empty() {
                return Task::none();
            }
            let idx = match app.history_cursor {
                Some(i) if i > 0 => i - 1,
                Some(i) => i,
                None => app.cmd_history.len() - 1,
            };
            app.history_cursor = Some(idx);
            app.input = app.cmd_history[idx].clone();
            app.tab_state = None;
            Task::none()
        }

        Message::HistoryDown => {
            let Some(cursor) = app.history_cursor else {
                return Task::none();
            };
            if cursor + 1 < app.cmd_history.len() {
                let idx = cursor + 1;
                app.history_cursor = Some(idx);
                app.input = app.cmd_history[idx].clone();
            } else {
                app.history_cursor = None;
                app.input.clear();
            }
            app.tab_state = None;
            Task::none()
        }

        // ────────────────────────────────────────────────────
        // Screen clear  (BUGFIX: actually clears the PTY)
        // ────────────────────────────────────────────────────

        Message::ClearScreen => {
            let Some(engine) = app.engine.clone() else {
                return Task::none();
            };
            app.direct_output.clear();
            app.last_snapshot = None;
            Task::perform(
                async move {
                    // Break out of pager/continuation first
                    let _ = engine.send_interrupt().await;
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    let _ = engine.send_raw("\r\n").await;
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    // Actually clear the PTY
                    if cfg!(windows) {
                        engine.send_input("cls").await
                    } else {
                        engine.send_input("clear").await
                    }
                },
                |r| match r {
                    Ok(_) => Message::Redraw,
                    Err(e) => Message::CommandError(format!("{:#}", e)),
                },
            )
        }

        // ────────────────────────────────────────────────────
        // Clipboard
        // ────────────────────────────────────────────────────

        Message::CopyToClipboard => {
            let mut clipboard_text = String::new();

            if let Some(ref snapshot) = app.last_snapshot {
                clipboard_text = crate::renderer::snapshot_to_plain(snapshot);
            } else if !app.direct_output.is_empty() {
                clipboard_text = app.direct_output.clone();
            }

            if let Ok(mut ctx) = copypasta::ClipboardContext::new() {
                use copypasta::ClipboardProvider;
                let _ = ctx.set_contents(clipboard_text);
                push_direct(app, "  📋 Copied to clipboard.");
            } else {
                push_direct(app, "  ⚠ Clipboard unavailable.");
            }
            Task::none()
        }

        // ────────────────────────────────────────────────────
        // Tab Completion
        // ────────────────────────────────────────────────────

        Message::TabComplete => {
            if app.input.trim().is_empty() {
                return Task::none();
            }

            if let Some(ref mut state) = app.tab_state {
                let next = state.next().to_string();
                app.input = next;
            } else {
                let aliases = get_alias_names(app);
                if let Some(state) = completer::complete(&app.input, &aliases, &app.cwd) {
                    let first = state.current().to_string();
                    let count = state.len();
                    app.tab_state = Some(state);
                    app.input = first;

                    if count > 1 {
                        let all: Vec<String> = app
                            .tab_state
                            .as_ref()
                            .unwrap()
                            .completions
                            .iter()
                            .map(|c| {
                                c.rsplit_once(' ')
                                    .map(|(_, r)| r)
                                    .or_else(|| c.strip_prefix('!'))
                                    .unwrap_or(c)
                                    .to_string()
                            })
                            .collect();
                        push_direct(
                            app,
                            &format!("  💡 {} matches: {}", count, all.join("  ")),
                        );
                    }
                }
            }
            Task::none()
        }

        // ────────────────────────────────────────────────────
        // Window resize
        // ────────────────────────────────────────────────────

        Message::WindowResized(width, _height) => {
            let cols = ((width as f32 - 20.0) / 8.0).max(40.0) as u16;
            let rows = 24u16;
            if let Some(engine) = app.engine.clone() {
                Task::perform(
                    async move {
                        let _ = engine.resize(cols, rows).await;
                    },
                    |_| Message::Redraw,
                )
            } else {
                Task::none()
            }
        }

        Message::Tick => Task::none(),

        Message::ThemeChanged(new_theme) => {
            app.theme_name = new_theme;
            Task::none()
        }

        // ────────────────────────────────────────────────────
        // PTY control signals  (PAGER-TRAP BUGFIX)
        // ────────────────────────────────────────────────────

        Message::SendInterrupt => {
            let Some(engine) = app.engine.clone() else {
                return Task::none();
            };
            Task::perform(
                async move { engine.send_interrupt().await },
                |r| match r {
                    Ok(_) => Message::Redraw,
                    Err(e) => Message::CommandError(format!("Interrupt failed: {:#}", e)),
                },
            )
        }

        Message::SendEscape => {
            let Some(engine) = app.engine.clone() else {
                return Task::none();
            };
            Task::perform(
                async move { engine.send_escape().await },
                |r| match r {
                    Ok(_) => Message::Redraw,
                    Err(e) => Message::CommandError(format!("Escape failed: {:#}", e)),
                },
            )
        }

        Message::SendEof => {
            let Some(engine) = app.engine.clone() else {
                return Task::none();
            };
            Task::perform(
                async move { engine.send_eof().await },
                |r| match r {
                    Ok(_) => Message::Redraw,
                    Err(e) => Message::CommandError(format!("EOF failed: {:#}", e)),
                },
            )
        }
    }
}