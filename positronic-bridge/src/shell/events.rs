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
use winit::keyboard::{Key, ModifiersState, NamedKey};

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

        // ── Resize ───────────────────────────────────────────────
        WindowEvent::Resized(new_size) => {
            if let Some(gpu) = &mut app.gpu {
                gpu.resize(new_size);
            }

            // Resize PTY to match
            if new_size.width > 0 && new_size.height > 0 {
                if let Some(gpu) = &app.gpu {
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
            }

            app.request_redraw();
        }

        // ── Keyboard ─────────────────────────────────────────────
        WindowEvent::KeyboardInput { event, .. } => {
            if event.state != ElementState::Pressed {
                return;
            }

            // Get current modifiers from the event
            let mods = event.modifiers.state();
            let ctrl = mods.contains(ModifiersState::CONTROL);
            let shift = mods.contains(ModifiersState::SHIFT);

            // ── Ctrl+Shift combos FIRST ──
            if ctrl && shift {
                if let Key::Character(ref c) = event.logical_key {
                    if c.as_str() == "c" || c.as_str() == "C" {
                        app.handle_copy();
                        app.request_redraw();
                        return;
                    }
                }
            }

            // ── Ctrl combos (without Shift) ──
            if ctrl && !shift {
                match &event.logical_key {
                    Key::Character(c) if c.as_str() == "c" => {
                        app.send_interrupt();
                        app.request_redraw();
                        return;
                    }
                    Key::Character(c) if c.as_str() == "d" => {
                        app.send_eof();
                        app.request_redraw();
                        return;
                    }
                    Key::Character(c) if c.as_str() == "l" => {
                        app.direct_output.clear();
                        app.last_snapshot = None;
                        if let Some(engine) = &app.engine {
                            let engine = engine.clone();
                            app.rt.spawn(async move {
                                let _ = engine.send_interrupt().await;
                                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                                let _ = engine.send_raw("\r\n").await;
                                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                                if cfg!(windows) {
                                    let _ = engine.send_input("cls").await;
                                } else {
                                    let _ = engine.send_input("clear").await;
                                }
                            });
                        }
                        app.request_redraw();
                        return;
                    }
                    _ => {}
                }
            }

            // ── Named keys ──
            match &event.logical_key {
                Key::Named(NamedKey::Escape) => {
                    app.send_escape();
                    app.request_redraw();
                }
                Key::Named(NamedKey::ArrowUp) => {
                    app.history_up();
                    app.request_redraw();
                }
                Key::Named(NamedKey::ArrowDown) => {
                    app.history_down();
                    app.request_redraw();
                }
                Key::Named(NamedKey::Tab) => {
                    app.tab_complete();
                    app.request_redraw();
                }
                Key::Named(NamedKey::Enter) => {
                    app.submit_input();
                    app.request_redraw();
                }
                Key::Named(NamedKey::Backspace) => {
                    if app.cursor_pos > 0 && !app.input.is_empty() {
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
                        app.input = format!("{}{}", &app.input[..byte_pos], &app.input[next_byte..]);
                        app.cursor_pos -= 1;
                        app.tab_state = None;
                        app.history_cursor = None;
                        app.request_redraw();
                    }
                }
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
                        app.input = format!("{}{}", &app.input[..byte_pos], &app.input[next_byte..]);
                        app.tab_state = None;
                        app.request_redraw();
                    }
                }
                Key::Named(NamedKey::ArrowLeft) => {
                    if app.cursor_pos > 0 {
                        app.cursor_pos -= 1;
                        app.request_redraw();
                    }
                }
                Key::Named(NamedKey::ArrowRight) => {
                    let char_count = app.input.chars().count();
                    if app.cursor_pos < char_count {
                        app.cursor_pos += 1;
                        app.request_redraw();
                    }
                }
                Key::Named(NamedKey::Home) => {
                    app.cursor_pos = 0;
                    app.request_redraw();
                }
                Key::Named(NamedKey::End) => {
                    app.cursor_pos = app.input.chars().count();
                    app.request_redraw();
                }
                // ── Character input ──
                Key::Character(c) if !ctrl => {
                    let char_count = app.input.chars().count();
                    if app.cursor_pos >= char_count {
                        app.input.push_str(c.as_str());
                    } else {
                        let byte_pos = app
                            .input
                            .char_indices()
                            .nth(app.cursor_pos)
                            .map(|(i, _)| i)
                            .unwrap_or(app.input.len());
                        app.input.insert_str(byte_pos, c.as_str());
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

            // Poll PTY
            app.poll_redraws();

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

                let result = gpu.render_frame(clear, |quads, text, device, queue, viewport| {
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