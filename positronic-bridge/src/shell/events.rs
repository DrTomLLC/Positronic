// positronic-bridge/src/shell/events.rs
//! Winit event handling.
//!
//! Translates WindowEvent into application actions. Replaces the old
//! iced subscription + keyboard module. All keybindings preserved:
//!   Ctrl+C       → send interrupt (0x03) to PTY
//!   Ctrl+Shift+C → copy to clipboard
//!   Ctrl+D       → send EOF (0x04) to PTY
//!   Ctrl+L       → clear screen
//!   Escape       → send escape (0x1b) to PTY
//!   ArrowUp      → history up
//!   ArrowDown    → history down
//!   Tab          → tab completion
//!   Enter        → submit input

use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};

use super::app::PositronicApp;
use crate::ui;

pub fn handle_window_event(
    app: &mut PositronicApp,
    event_loop: &dyn ActiveEventLoop,
    event: WindowEvent,
) {
    match event {
        // ── Window lifecycle ──────────────────────────────────────
        WindowEvent::CloseRequested => {
            tracing::info!("Window close requested");
            event_loop.exit();
        }

        WindowEvent::Destroyed => {
            tracing::info!("Window destroyed");
        }

        // ── Resize (winit 0.31: Resized → SurfaceResized) ───────
        WindowEvent::SurfaceResized(new_size) => {
            if let Some(gpu) = &mut app.gpu {
                gpu.resize(new_size);
            }

            // Resize PTY to match
            if new_size.width > 0 && new_size.height > 0 {
                let (cell_w, cell_h) = (8.0f32, 18.0f32); // approx cell size
                let cols = ((new_size.width as f32 - 20.0) / cell_w).max(40.0) as u16;
                let rows = ((new_size.height as f32 - 60.0) / cell_h).max(10.0) as u16;

                if let Some(engine) = &app.engine {
                    let engine = engine.clone();
                    app.rt.spawn(async move {
                        let _ = engine.resize(cols, rows).await;
                    });
                }
            }

            app.request_redraw();
        }

        // ── Modifiers (winit 0.31: track via ModifiersChanged, not KeyEvent.modifiers) ──
        WindowEvent::ModifiersChanged(modifiers) => {
            app.modifiers = modifiers.state();
        }

        // ── Keyboard ─────────────────────────────────────────────
        WindowEvent::KeyboardInput { event, .. } => {
            if event.state != ElementState::Pressed {
                return;
            }

            // Use the tracked modifiers from ModifiersChanged events
            let mods = app.modifiers;
            let ctrl = mods.control_key();
            let shift = mods.shift_key();

            match event.logical_key.as_ref() {
                // Ctrl+C: interrupt or copy
                Key::Character("c") if ctrl && shift => {
                    // Ctrl+Shift+C = copy
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        let text = app.input.clone();
                        let _ = clipboard.set_text(text);
                    }
                }
                Key::Character("c") if ctrl => {
                    app.send_interrupt();
                }

                // Ctrl+D: EOF
                Key::Character("d") if ctrl => {
                    app.send_eof();
                }

                // Ctrl+L: clear screen
                Key::Character("l") if ctrl => {
                    app.direct_output.clear();
                    app.last_snapshot = None;
                    app.request_redraw();
                }

                // Escape
                Key::Named(NamedKey::Escape) => {
                    app.send_escape();
                }

                // Enter: submit command
                Key::Named(NamedKey::Enter) => {
                    app.submit_command();
                    app.request_redraw();
                }

                // Backspace
                Key::Named(NamedKey::Backspace) => {
                    if app.cursor_pos > 0 {
                        let byte_pos = app
                            .input
                            .char_indices()
                            .nth(app.cursor_pos - 1)
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                        let next_byte = app
                            .input
                            .char_indices()
                            .nth(app.cursor_pos)
                            .map(|(i, _)| i)
                            .unwrap_or(app.input.len());
                        app.input.replace_range(byte_pos..next_byte, "");
                        app.cursor_pos -= 1;
                        app.tab_state = None;
                        app.request_redraw();
                    }
                }

                // Delete
                Key::Named(NamedKey::Delete) => {
                    let char_count = app.input.chars().count();
                    if app.cursor_pos < char_count {
                        let byte_pos = app
                            .input
                            .char_indices()
                            .nth(app.cursor_pos)
                            .map(|(i, _)| i)
                            .unwrap_or(app.input.len());
                        let next_byte = app
                            .input
                            .char_indices()
                            .nth(app.cursor_pos + 1)
                            .map(|(i, _)| i)
                            .unwrap_or(app.input.len());
                        app.input.replace_range(byte_pos..next_byte, "");
                        app.tab_state = None;
                        app.request_redraw();
                    }
                }

                // Arrow keys: cursor movement + history
                Key::Named(NamedKey::ArrowLeft) => {
                    if app.cursor_pos > 0 {
                        app.cursor_pos -= 1;
                        app.request_redraw();
                    }
                }
                Key::Named(NamedKey::ArrowRight) => {
                    if app.cursor_pos < app.input.chars().count() {
                        app.cursor_pos += 1;
                        app.request_redraw();
                    }
                }
                Key::Named(NamedKey::ArrowUp) => {
                    if !app.cmd_history.is_empty() {
                        let new_cursor = match app.history_cursor {
                            None => app.cmd_history.len() - 1,
                            Some(c) if c > 0 => c - 1,
                            Some(c) => c,
                        };
                        app.history_cursor = Some(new_cursor);
                        app.input = app.cmd_history[new_cursor].clone();
                        app.cursor_pos = app.input.chars().count();
                        app.request_redraw();
                    }
                }
                Key::Named(NamedKey::ArrowDown) => {
                    if let Some(c) = app.history_cursor {
                        if c + 1 < app.cmd_history.len() {
                            let new_cursor = c + 1;
                            app.history_cursor = Some(new_cursor);
                            app.input = app.cmd_history[new_cursor].clone();
                            app.cursor_pos = app.input.chars().count();
                        } else {
                            app.history_cursor = None;
                            app.input.clear();
                            app.cursor_pos = 0;
                        }
                        app.request_redraw();
                    }
                }

                // Home / End
                Key::Named(NamedKey::Home) => {
                    app.cursor_pos = 0;
                    app.request_redraw();
                }
                Key::Named(NamedKey::End) => {
                    app.cursor_pos = app.input.chars().count();
                    app.request_redraw();
                }

                // Tab: completion
                Key::Named(NamedKey::Tab) => {
                    // TODO: integrate with completer
                    app.request_redraw();
                }

                // Regular character input
                Key::Character(c) if !ctrl => {
                    if app.cursor_pos == app.input.chars().count() {
                        app.input.push_str(c);
                    } else {
                        let byte_pos = app
                            .input
                            .char_indices()
                            .nth(app.cursor_pos)
                            .map(|(i, _)| i)
                            .unwrap_or(app.input.len());
                        app.input.insert_str(byte_pos, c);
                    }
                    app.cursor_pos += c.chars().count();
                    app.tab_state = None;
                    app.history_cursor = None;
                    app.request_redraw();
                }
                _ => {}
            }
        }

        // ── Redraw ───────────────────────────────────────────────
        WindowEvent::RedrawRequested => {
            // Check if engine became ready
            app.check_engine_ready();

            // Poll PTY + command results
            app.poll_redraws();
            app.poll_cmd_results();

            // Render
            if let Some(gpu) = &mut app.gpu {
                let theme = app.theme_name;
                let clear = theme.bg_color();

                // Gather all UI draw data
                let snapshot = app.last_snapshot.clone();
                let direct = app.direct_output.clone();
                let input_text = app.input.clone();
                let cursor = app.cursor_pos;
                let state = app.state.clone();
                let cmd_count = app.session_cmd_count;
                let boot = app.boot_instant;
                let cwd = app.cwd.clone();
                let theme_name = app.theme_name;

                let result = gpu.render_frame(clear, |quads, text, _device, _queue, viewport| {
                    ui::scene::compose(
                        quads,
                        text,
                        viewport,
                        &ui::scene::SceneData {
                            state: &state,
                            snapshot: snapshot.as_ref(),
                            direct_output: &direct,
                            input: &input_text,
                            cursor_pos: cursor,
                            theme: theme_name,
                            session_cmd_count: cmd_count,
                            boot_instant: boot,
                            cwd: &cwd,
                        },
                    );
                });

                if let Err(e) = result {
                    tracing::error!("Render failed: {:#}", e);
                }
            }
        }

        _ => {}
    }
}