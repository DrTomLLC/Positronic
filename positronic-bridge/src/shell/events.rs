use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};

use super::app::PositronicApp;

pub fn handle_window_event(
    app: &mut PositronicApp,
    event_loop: &dyn ActiveEventLoop,
    event: WindowEvent,
) {
    match event {
        WindowEvent::CloseRequested => {
            tracing::info!("Window close requested");
            event_loop.exit();
        }

        WindowEvent::Destroyed => {
            tracing::info!("Window destroyed");
        }

        WindowEvent::SurfaceResized(new_size) => {
            if let Some(gpu) = &mut app.gpu {
                gpu.resize(new_size);
            }

            if new_size.width > 0 && new_size.height > 0 {
                let (cell_w, cell_h) = (8.0f32, 18.0f32);
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

        WindowEvent::ModifiersChanged(modifiers) => {
            app.modifiers = modifiers.state();
        }

        WindowEvent::CursorMoved { position, .. } => {
            app.last_mouse_x = position.x as f32;
            app.last_mouse_y = position.y as f32;
        }

        WindowEvent::MouseInput { state, button, .. } => {
            if state == ElementState::Pressed && button == MouseButton::Left {
                // Click Holodeck buttons if visible
                if app.holodeck_safe {
                    if let Some(doc) = &app.holodeck_doc {
                        if let Some(action) = crate::holodeck::renderer::click(
                            doc,
                            app.last_mouse_x,
                            app.last_mouse_y,
                        ) {
                            app.apply_holodeck_action(action);
                            app.request_redraw();
                        }
                    }
                }
            }
        }

        WindowEvent::KeyboardInput { event, .. } => {
            if event.state != ElementState::Pressed {
                return;
            }

            let mods = app.modifiers;
            let ctrl = mods.control_key();
            let shift = mods.shift_key();

            match event.logical_key.as_ref() {
                // Ctrl+Shift+C = copy snapshot
                Key::Character("c") if ctrl && shift => {
                    app.copy_visible_to_clipboard();
                }

                // Ctrl+C interrupt
                Key::Character("c") if ctrl => app.send_interrupt(),

                Key::Character("d") if ctrl => app.send_eof(),

                Key::Character("l") if ctrl => {
                    app.direct_output.clear();
                    app.last_snapshot = None;
                    app.request_redraw();
                }

                Key::Named(NamedKey::Escape) => app.send_escape(),

                Key::Named(NamedKey::Enter) => {
                    app.submit_command();
                    app.request_redraw();
                }

                Key::Named(NamedKey::Backspace) => app.input_backspace(),
                Key::Named(NamedKey::Delete) => app.input_delete(),

                Key::Named(NamedKey::ArrowLeft) => app.input_left(),
                Key::Named(NamedKey::ArrowRight) => app.input_right(),

                Key::Named(NamedKey::ArrowUp) => app.history_up(),
                Key::Named(NamedKey::ArrowDown) => app.history_down(),

                Key::Named(NamedKey::Home) => app.input_home(),
                Key::Named(NamedKey::End) => app.input_end(),

                Key::Named(NamedKey::Tab) => {
                    // TODO: integrate with completer
                    app.request_redraw();
                }

                Key::Character(c) if !ctrl => {
                    app.input_insert(c);
                    app.request_redraw();
                }
                _ => {}
            }
        }

        WindowEvent::RedrawRequested => {
            app.check_engine_ready();

            app.poll_redraws();
            app.poll_cmd_results();

            if let Some(gpu) = &mut app.gpu {
                let theme = app.theme_name;
                let clear = theme.bg_color();

                let snapshot = app.last_snapshot.clone();
                let direct = app.direct_output.clone();
                let input_text = app.input.clone();
                let cursor = app.cursor_pos;
                let state = app.state.clone();
                let cmd_count = app.session_cmd_count;
                let boot = app.boot_instant;
                let cwd = app.cwd.clone();

                // Holodeck
                let holodeck_safe = app.holodeck_safe;
                let mut holodeck_doc = app.holodeck_doc.clone();

                let result = gpu.render_frame(clear, |quads, text, _device, _queue, viewport| {
                    crate::ui::scene::compose(
                        quads,
                        text,
                        viewport,
                        &mut crate::ui::scene::SceneData {
                            state: &state,
                            snapshot: snapshot.as_ref(),
                            direct_output: &direct,
                            input: &input_text,
                            cursor_pos: cursor,
                            theme,
                            session_cmd_count: cmd_count,
                            boot_instant: boot,
                            cwd: &cwd,
                            holodeck_doc: holodeck_doc.as_mut(),
                            holodeck_safe,
                        },
                    );
                });

                if let Err(e) = result {
                    tracing::error!("Render failed: {:#}", e);
                }

                // write back doc (it gets laid out during draw)
                app.holodeck_doc = holodeck_doc;
            }
        }

        _ => {}
    }
}
