//! Keyboard dispatch and subscription management.
//!
//! CRITICAL BUGFIX: Ctrl+C now sends `SendInterrupt` (0x03 to PTY).
//! Ctrl+Shift+C is used for clipboard copy.
//! Escape sends `SendEscape` (0x1b) for exiting vi-style pagers.
//! Ctrl+D sends `SendEof` (0x04) for end-of-input.

use crate::app::{PositronicApp, RedrawHandle};
use crate::messages::Message;

use iced::{event, keyboard, Subscription};

// ────────────────────────────────────────────────────────────────
// Redraw worker
// ────────────────────────────────────────────────────────────────

/// Async stream that pumps PTY redraw notifications into iced messages.
pub fn redraw_worker(
    handle: &RedrawHandle,
) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = Message> + Send>> {
    use iced::futures::SinkExt;
    use iced::futures::channel::mpsc;

    let handle = handle.clone();
    Box::pin(iced::stream::channel(
        16,
        async move |mut output: mpsc::Sender<Message>| {
            loop {
                let next = {
                    let mut guard = handle.0.lock().await;
                    guard.recv().await
                };
                match next {
                    Some(()) => {
                        let _ = output.send(Message::Redraw).await;
                    }
                    None => {
                        eprintln!("[DEBUG] Redraw worker channel closed");
                        break;
                    }
                }
            }
        },
    ))
}

// ────────────────────────────────────────────────────────────────
// Subscription
// ────────────────────────────────────────────────────────────────

pub fn subscription(app: &PositronicApp) -> Subscription<Message> {
    let mut subs: Vec<Subscription<Message>> = Vec::new();

    // PTY redraw pump
    if let Some(handle) = &app.redraw {
        subs.push(Subscription::run_with(handle.clone(), redraw_worker));
    }

    // Keyboard + window events
    subs.push(event::listen_with(|evt, _status, _id| {
        match evt {
            iced::Event::Keyboard(keyboard::Event::KeyPressed {
                                      key,
                                      modifiers,
                                      ..
                                  }) => {
                // ── Ctrl+Shift combos FIRST (check shift before plain ctrl) ──
                if modifiers.control() && modifiers.shift() {
                    match &key {
                        keyboard::Key::Character(c)
                        if c.as_str() == "c" || c.as_str() == "C" =>
                            {
                                return Some(Message::CopyToClipboard);
                            }
                        _ => {}
                    }
                }

                // ── Ctrl combos (without Shift) ──
                if modifiers.control() && !modifiers.shift() {
                    return match &key {
                        keyboard::Key::Character(c) if c.as_str() == "c" => {
                            Some(Message::SendInterrupt) // BUGFIX: was CopyToClipboard
                        }
                        keyboard::Key::Character(c) if c.as_str() == "d" => {
                            Some(Message::SendEof)
                        }
                        keyboard::Key::Character(c) if c.as_str() == "l" => {
                            Some(Message::ClearScreen)
                        }
                        _ => None,
                    };
                }

                // ── Bare keys ──
                match key {
                    keyboard::Key::Named(keyboard::key::Named::Escape) => {
                        Some(Message::SendEscape)
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                        Some(Message::HistoryUp)
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                        Some(Message::HistoryDown)
                    }
                    keyboard::Key::Named(keyboard::key::Named::Tab) => {
                        Some(Message::TabComplete)
                    }
                    _ => None,
                }
            }
            iced::Event::Window(iced::window::Event::Resized(size)) => {
                Some(Message::WindowResized(size.width as u32, size.height as u32))
            }
            _ => None,
        }
    }));

    // Status bar tick — update uptime every 5 seconds
    subs.push(iced::time::every(std::time::Duration::from_secs(5)).map(|_| Message::Tick));

    Subscription::batch(subs)
}