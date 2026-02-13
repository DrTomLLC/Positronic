use iced::futures::SinkExt;
use iced::widget::{column, container, row, text, text_editor, text_input};
use iced::{event, keyboard, Color, Element, Length, Settings, Subscription, Task, Theme};

use positronic_core::runner::ExecuteResult;
use positronic_core::PositronicEngine;

use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

/// Max direct output buffer (~256KB). Oldest half trimmed when exceeded.
const MAX_DIRECT_BYTES: usize = 256 * 1024;

pub fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .init();

    eprintln!("=== Positronic Starting ===");

    iced::application(boot, update, view)
        .title(title)
        .theme(theme)
        .subscription(subscription)
        .settings(Settings {
            antialiasing: true,
            ..Settings::default()
        })
        .run()
}

// ====================================================================
// State
// ====================================================================

#[derive(Debug, Clone, PartialEq)]
enum AppState {
    Booting,
    Active,
    Error(String),
}

struct PositronicApp {
    engine: Option<Arc<PositronicEngine>>,
    redraw: Option<RedrawHandle>,

    /// Accumulated direct output (echoed commands, ! command output).
    direct_output: String,
    /// Current live PTY screen. REPLACED on each Redraw.
    pty_snapshot: String,
    /// The text_editor content.
    output_content: text_editor::Content,

    /// Command history and cursor for Up/Down navigation.
    cmd_history: Vec<String>,
    history_cursor: Option<usize>,

    /// Status bar data
    session_cmd_count: usize,
    boot_instant: std::time::Instant,

    input: String,
    state: AppState,
    last_screen_hash: u64,
}

#[derive(Clone, Debug)]
struct RedrawHandle(Arc<Mutex<mpsc::Receiver<()>>>);

impl PartialEq for RedrawHandle {
    fn eq(&self, other: &Self) -> bool {
        Arc::as_ptr(&self.0) == Arc::as_ptr(&other.0)
    }
}
impl Eq for RedrawHandle {}
impl Hash for RedrawHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.0) as usize).hash(state);
    }
}

// ====================================================================
// Messages
// ====================================================================

#[derive(Clone, Debug)]
enum Message {
    EngineReady(Arc<PositronicEngine>, RedrawHandle),
    EngineFailed(String),
    Redraw,
    InputChanged(String),
    InputSent,
    CommandResult(ExecuteResult),
    CommandError(String),
    OutputAction(text_editor::Action),
    HistoryUp,
    HistoryDown,
    ClearScreen,
    WindowResized(u32, u32),
    Tick,
}

// ====================================================================
// Output management
// ====================================================================

fn push_direct(app: &mut PositronicApp, new_text: &str) {
    app.direct_output.push_str(new_text);
    if !new_text.ends_with('\n') {
        app.direct_output.push('\n');
    }

    if app.direct_output.len() > MAX_DIRECT_BYTES {
        let mid = app.direct_output.len() / 2;
        if let Some(nl) = app.direct_output[mid..].find('\n') {
            let trim_at = mid + nl + 1;
            let kept = app.direct_output[trim_at..].to_string();
            app.direct_output = format!("··· (older output trimmed) ···\n{}", kept);
        }
    }

    rebuild_display(app);
}

fn set_pty_snapshot(app: &mut PositronicApp, text: &str) {
    app.pty_snapshot = text.to_string();
    rebuild_display(app);
}

fn rebuild_display(app: &mut PositronicApp) {
    let mut full = String::with_capacity(app.direct_output.len() + app.pty_snapshot.len() + 2);
    full.push_str(&app.direct_output);

    if !app.pty_snapshot.is_empty() {
        if !full.is_empty() && !full.ends_with('\n') {
            full.push('\n');
        }
        full.push_str(&app.pty_snapshot);
    }

    app.output_content = text_editor::Content::with_text(&full);
    app.output_content
        .perform(text_editor::Action::Move(text_editor::Motion::DocumentEnd));
}

// ====================================================================
// Boot
// ====================================================================

fn boot() -> (PositronicApp, Task<Message>) {
    let app = PositronicApp {
        engine: None,
        redraw: None,
        direct_output: "⏳ Booting Positronic Engine...\n".to_string(),
        pty_snapshot: String::new(),
        output_content: text_editor::Content::with_text("⏳ Booting Positronic Engine...\n"),
        cmd_history: Vec::new(),
        history_cursor: None,
        session_cmd_count: 0,
        boot_instant: std::time::Instant::now(),
        input: String::new(),
        state: AppState::Booting,
        last_screen_hash: 0,
    };

    let task = Task::perform(
        async {
            let (tx, rx) = mpsc::channel(100);
            match PositronicEngine::start(80, 24, tx).await {
                Ok(e) => Ok((Arc::new(e), RedrawHandle(Arc::new(Mutex::new(rx))))),
                Err(e) => Err(format!("{:#}", e)),
            }
        },
        |r| match r {
            Ok((e, h)) => Message::EngineReady(e, h),
            Err(s) => Message::EngineFailed(s),
        },
    );

    (app, task)
}

fn title(app: &PositronicApp) -> String {
    match &app.state {
        AppState::Booting => "Positronic /// Booting...".into(),
        AppState::Active => "Positronic /// Data Surface".into(),
        AppState::Error(e) => format!("Positronic /// ERROR: {}", &e[..e.len().min(60)]),
    }
}

fn theme(_: &PositronicApp) -> Theme {
    Theme::Dark
}

// ====================================================================
// Update
// ====================================================================

fn update(app: &mut PositronicApp, message: Message) -> Task<Message> {
    match message {
        Message::EngineReady(engine, redraw) => {
            eprintln!("[UI] ✅ Engine ready!");
            app.engine = Some(engine);
            app.redraw = Some(redraw);
            app.state = AppState::Active;
            app.direct_output.clear();
            app.pty_snapshot.clear();
            push_direct(app, "⚡ Positronic v0.2.0 Online.  Type !help for commands.");
            Task::none()
        }

        Message::EngineFailed(err) => {
            app.state = AppState::Error(err.clone());
            push_direct(app, &format!("❌ BOOT FAILED: {}", err));
            Task::none()
        }

        Message::Redraw => {
            if let Some(engine) = &app.engine {
                let snapshot = engine.state.snapshot();
                let new_hash = hash_snapshot(&snapshot);

                if new_hash != app.last_screen_hash {
                    app.last_screen_hash = new_hash;
                    let text = snapshot_to_string(&snapshot);
                    set_pty_snapshot(app, &text);
                }
            }
            Task::none()
        }

        Message::InputChanged(val) => {
            app.input = val;
            app.history_cursor = None;
            Task::none()
        }

        Message::InputSent => {
            let trimmed = app.input.trim().to_string();
            if trimmed.is_empty() {
                return Task::none();
            }

            app.cmd_history.push(trimmed.clone());
            app.history_cursor = None;
            app.session_cmd_count += 1;

            push_direct(app, &format!("➜ {}", trimmed));
            app.input.clear();

            let Some(engine) = app.engine.clone() else {
                push_direct(app, "❌ Engine not ready.");
                return Task::none();
            };

            Task::perform(
                async move { engine.send_input(&format!("{}\n", trimmed)).await },
                |r| match r {
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
                    app.pty_snapshot.clear();
                    app.output_content = text_editor::Content::with_text("");
                }
            }
            Task::none()
        }

        Message::CommandError(err) => {
            push_direct(app, &format!("❌ {}", err));
            Task::none()
        }

        Message::OutputAction(action) => {
            if !matches!(action, text_editor::Action::Edit(_)) {
                app.output_content.perform(action);
            }
            Task::none()
        }

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
            Task::none()
        }

        Message::ClearScreen => {
            let Some(engine) = app.engine.clone() else {
                return Task::none();
            };
            app.direct_output.clear();
            app.pty_snapshot.clear();
            app.output_content = text_editor::Content::with_text("");
            Task::perform(
                async move { engine.send_input("cls\n").await },
                |r| match r {
                    Ok(_) => Message::Redraw,
                    Err(e) => Message::CommandError(format!("{:#}", e)),
                },
            )
        }

        Message::WindowResized(width, _height) => {
            let cols = ((width as f32 - 20.0) / 8.0).max(40.0) as u16;
            let rows = 24u16;
            if let Some(engine) = app.engine.clone() {
                Task::perform(
                    async move { let _ = engine.resize(cols, rows).await; },
                    |_| Message::Redraw,
                )
            } else {
                Task::none()
            }
        }

        Message::Tick => {
            // Status bar updates on tick — no-op, iced re-renders view()
            Task::none()
        }
    }
}

// ====================================================================
// View
// ====================================================================

fn view(app: &PositronicApp) -> Element<'_, Message> {
    let mut layout = column![]
        .spacing(0)
        .padding(0)
        .width(Length::Fill)
        .height(Length::Fill);

    // ── Boot / Error messages ──
    match &app.state {
        AppState::Booting => {
            layout = layout.push(
                container(
                    text("⏳ Booting engine...")
                        .font(iced::Font::MONOSPACE)
                        .size(14)
                ).padding(10),
            );
        }
        AppState::Error(e) => {
            layout = layout.push(
                container(
                    text(format!("❌ {}", e))
                        .font(iced::Font::MONOSPACE)
                        .size(14)
                        .color(Color::from_rgb(1.0, 0.4, 0.4))
                ).padding(10),
            );
        }
        AppState::Active => {}
    }

    // ── Output area ──
    let output = text_editor(&app.output_content)
        .font(iced::Font::MONOSPACE)
        .size(14)
        .height(Length::Fill)
        .on_action(Message::OutputAction);

    layout = layout.push(
        container(output).padding([5, 10])
    );

    // ── Status bar ──
    let uptime_secs = app.boot_instant.elapsed().as_secs() as i64;
    let uptime_str = format_duration_short(uptime_secs);

    let status_text = format!(
        " ⚡ {} cmd(s)  │  ⏱ {}  │  Positronic v0.2.0",
        app.session_cmd_count,
        uptime_str,
    );

    let status_bar = container(
        text(status_text)
            .font(iced::Font::MONOSPACE)
            .size(12)
            .color(Color::from_rgb(0.5, 0.55, 0.6))
    )
        .width(Length::Fill)
        .padding([3.0, 12.0])
        .style(status_bar_style);

    layout = layout.push(status_bar);

    // ── Input bar ──
    let input = text_input("Type a command… (!help for commands)", &app.input)
        .font(iced::Font::MONOSPACE)
        .size(14)
        .padding(10)
        .on_input(Message::InputChanged)
        .on_submit(Message::InputSent);

    layout = layout.push(
        container(row![input].width(Length::Fill)).padding([10, 10])
    );

    layout.into()
}

/// Status bar styling — subtle dark background with top border.
fn status_bar_style(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(iced::Background::Color(Color::from_rgba(0.08, 0.09, 0.1, 1.0))),
        border: iced::Border {
            color: Color::from_rgb(0.2, 0.22, 0.25),
            width: 1.0,
            radius: 0.0.into(),
        },
        ..iced::widget::container::Style::default()
    }
}

// ====================================================================
// Subscriptions
// ====================================================================

fn redraw_worker(
    handle: &RedrawHandle,
) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = Message> + Send>> {
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
                    Some(()) => { let _ = output.send(Message::Redraw).await; }
                    None => break,
                }
            }
        },
    ))
}

fn subscription(app: &PositronicApp) -> Subscription<Message> {
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
                if modifiers.control() {
                    if let keyboard::Key::Character(ref c) = key {
                        if c.as_str() == "l" {
                            return Some(Message::ClearScreen);
                        }
                    }
                    return None;
                }
                match key {
                    keyboard::Key::Named(keyboard::key::Named::ArrowUp) => Some(Message::HistoryUp),
                    keyboard::Key::Named(keyboard::key::Named::ArrowDown) => Some(Message::HistoryDown),
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

// ====================================================================
// Snapshot helpers
// ====================================================================

fn hash_snapshot(snapshot: &impl SnapshotLike) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut h = DefaultHasher::new();
    snapshot.rows().hash(&mut h);
    snapshot.cols().hash(&mut h);
    for row in snapshot.rows_iter() {
        for (c, _) in row {
            c.hash(&mut h);
        }
    }
    h.finish()
}

fn snapshot_to_string(snapshot: &impl SnapshotLike) -> String {
    let mut lines: Vec<String> = Vec::with_capacity(snapshot.rows());
    for row in snapshot.rows_iter() {
        let mut s: String = row.iter().map(|(c, _)| *c).collect();
        while s.ends_with(' ') { s.pop(); }
        lines.push(s);
    }
    while matches!(lines.last(), Some(l) if l.trim().is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

trait SnapshotLike {
    fn rows(&self) -> usize;
    fn cols(&self) -> usize;
    fn rows_iter<'a>(
        &'a self,
    ) -> Box<dyn Iterator<Item = &'a [(char, positronic_core::state_machine::MyColor)]> + 'a>;
}

impl SnapshotLike for positronic_core::state_machine::Snapshot {
    fn rows(&self) -> usize { self.rows() }
    fn cols(&self) -> usize { self.cols() }
    fn rows_iter<'a>(
        &'a self,
    ) -> Box<dyn Iterator<Item = &'a [(char, positronic_core::state_machine::MyColor)]> + 'a> {
        Box::new(self.into_iter())
    }
}

// ====================================================================
// Formatting
// ====================================================================

fn format_duration_short(secs: i64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        format!("{}h {}m", h, m)
    }
}