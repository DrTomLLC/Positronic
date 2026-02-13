use iced::futures::SinkExt;
use iced::widget::{column, row, text, text_editor, text_input};
use iced::{Element, Length, Settings, Subscription, Task, Theme};

use positronic_core::runner::ExecuteResult;
use positronic_core::PositronicEngine;

use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

/// Maximum direct output buffer (~256KB). Oldest half trimmed when exceeded.
const MAX_DIRECT_BYTES: usize = 256 * 1024;

pub fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .init();

    eprintln!("=== Positronic Starting ===");

    let settings = Settings {
        antialiasing: true,
        ..Settings::default()
    };

    iced::application(boot, update, view)
        .title(title)
        .theme(theme)
        .subscription(subscription)
        .settings(settings)
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

    /// Accumulated direct output from ! commands and echoed commands.
    /// This is NEVER overwritten by PTY snapshots.
    direct_output: String,

    /// The current live PTY snapshot text. REPLACED (not appended)
    /// on every Redraw.
    pty_snapshot: String,

    /// The text_editor content (rebuilt from direct_output + pty_snapshot).
    output_content: text_editor::Content,

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
}

// ====================================================================
// Output management
// ====================================================================

/// Append text to direct output and rebuild the display.
fn push_direct(app: &mut PositronicApp, text: &str) {
    app.direct_output.push_str(text);
    if !text.ends_with('\n') {
        app.direct_output.push('\n');
    }

    // Trim direct output if too large
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

/// Replace the PTY snapshot portion and rebuild the display.
fn set_pty_snapshot(app: &mut PositronicApp, snapshot: &str) {
    app.pty_snapshot = snapshot.to_string();
    rebuild_display(app);
}

/// Rebuild the text_editor content from direct_output + pty_snapshot,
/// and auto-scroll to the bottom.
fn rebuild_display(app: &mut PositronicApp) {
    let mut full = String::with_capacity(
        app.direct_output.len() + app.pty_snapshot.len() + 2,
    );

    full.push_str(&app.direct_output);

    if !app.pty_snapshot.is_empty() {
        if !full.is_empty() && !full.ends_with('\n') {
            full.push('\n');
        }
        full.push_str(&app.pty_snapshot);
    }

    app.output_content = text_editor::Content::with_text(&full);

    // Auto-scroll to bottom
    app.output_content
        .perform(text_editor::Action::Move(text_editor::Motion::DocumentEnd));
}

// ====================================================================
// Boot
// ====================================================================

fn boot() -> (PositronicApp, Task<Message>) {
    let initial = "⏳ Booting Positronic Engine...\n";

    let app = PositronicApp {
        engine: None,
        redraw: None,
        direct_output: initial.to_string(),
        pty_snapshot: String::new(),
        output_content: text_editor::Content::with_text(initial),
        input: String::new(),
        state: AppState::Booting,
        last_screen_hash: 0,
    };

    let task = Task::perform(
        async {
            let (tx, rx) = mpsc::channel(100);
            match PositronicEngine::start(80, 24, tx).await {
                Ok(engine) => Ok((Arc::new(engine), RedrawHandle(Arc::new(Mutex::new(rx))))),
                Err(e) => Err(format!("{:#}", e)),
            }
        },
        |res| match res {
            Ok((engine, redraw)) => Message::EngineReady(engine, redraw),
            Err(err) => Message::EngineFailed(err),
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

fn theme(_app: &PositronicApp) -> Theme {
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
            push_direct(app, "⚡ Positronic Engine Online.  Type !help for commands.\n");
            Task::none()
        }

        Message::EngineFailed(err) => {
            eprintln!("[UI] ❌ EngineFailed: {}", err);
            app.state = AppState::Error(err.clone());
            push_direct(app, &format!("❌ BOOT FAILED: {}\n", err));
            Task::none()
        }

        Message::Redraw => {
            if let Some(engine) = &app.engine {
                let snapshot = engine.state.snapshot();
                let new_hash = hash_snapshot(&snapshot);

                if new_hash != app.last_screen_hash {
                    app.last_screen_hash = new_hash;
                    let display = snapshot_to_string(&snapshot);

                    // REPLACE the PTY snapshot portion (not append)
                    set_pty_snapshot(app, &display);
                }
            }
            Task::none()
        }

        Message::InputChanged(val) => {
            app.input = val;
            Task::none()
        }

        Message::InputSent => {
            let trimmed = app.input.trim().to_string();
            if trimmed.is_empty() {
                return Task::none();
            }

            // Echo command into direct output
            push_direct(app, &format!("➜ {}", trimmed));
            app.input.clear();

            let Some(engine) = app.engine.clone() else {
                push_direct(app, "❌ Engine not ready.");
                return Task::none();
            };

            Task::perform(
                async move { engine.send_input(&format!("{}\n", trimmed)).await },
                |res| match res {
                    Ok(result) => Message::CommandResult(result),
                    Err(e) => Message::CommandError(format!("{:#}", e)),
                },
            )
        }

        Message::CommandResult(result) => {
            match result {
                ExecuteResult::SentToPty => {
                    // Redraw subscription will replace pty_snapshot
                }
                ExecuteResult::DirectOutput(lines) => {
                    push_direct(app, &lines.join("\n"));
                }
                ExecuteResult::ClearScreen => {
                    // Clear BOTH direct output and PTY snapshot.
                    // The runner already sent `cls` to the PTY,
                    // so the next snapshot will be clean.
                    app.direct_output.clear();
                    app.pty_snapshot.clear();
                    app.last_screen_hash = 0;
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
            // Read-only: allow selection/copy/cursor, block edits
            match &action {
                text_editor::Action::Edit(_) => {}
                _ => {
                    app.output_content.perform(action);
                }
            }
            Task::none()
        }
    }
}

// ====================================================================
// View
// ====================================================================

fn view(app: &PositronicApp) -> Element<'_, Message> {
    let mut layout = column![]
        .spacing(5)
        .padding(10)
        .width(Length::Fill)
        .height(Length::Fill);

    match &app.state {
        AppState::Booting => {
            layout = layout.push(
                text("⏳ Booting engine...")
                    .font(iced::Font::MONOSPACE)
                    .size(14),
            );
        }
        AppState::Error(e) => {
            layout = layout.push(
                text(format!("❌ {}", e))
                    .font(iced::Font::MONOSPACE)
                    .size(14),
            );
        }
        AppState::Active => {}
    }

    // Output: text_editor scrolls natively. NO scrollable wrapper.
    let output = text_editor(&app.output_content)
        .font(iced::Font::MONOSPACE)
        .size(14)
        .height(Length::Fill)
        .on_action(Message::OutputAction);

    layout = layout.push(output);

    // Input bar
    let input = text_input("Type a command… (!help for commands)", &app.input)
        .font(iced::Font::MONOSPACE)
        .size(14)
        .padding(10)
        .on_input(Message::InputChanged)
        .on_submit(Message::InputSent);

    layout = layout.push(row![input].width(Length::Fill));

    layout.into()
}

// ====================================================================
// Subscription
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
                    Some(()) => {
                        let _ = output.send(Message::Redraw).await;
                    }
                    None => break,
                }
            }
        },
    ))
}

fn subscription(app: &PositronicApp) -> Subscription<Message> {
    match &app.redraw {
        Some(handle) => Subscription::run_with(handle.clone(), redraw_worker),
        None => Subscription::none(),
    }
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
        while s.ends_with(' ') {
            s.pop();
        }
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
    fn rows(&self) -> usize {
        self.rows()
    }
    fn cols(&self) -> usize {
        self.cols()
    }
    fn rows_iter<'a>(
        &'a self,
    ) -> Box<dyn Iterator<Item = &'a [(char, positronic_core::state_machine::MyColor)]> + 'a> {
        Box::new(self.into_iter())
    }
}