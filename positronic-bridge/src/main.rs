use iced::futures::SinkExt;
use iced::widget::{column, container, row, scrollable, text, text_editor, text_input};
use iced::{Element, Length, Settings, Subscription, Task, Theme};

use positronic_bridge::holodeck::TerminalBlock;
use positronic_core::runner::ExecuteResult;
use positronic_core::PositronicEngine;

use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

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

#[derive(Debug, Clone, PartialEq)]
enum AppState {
    Booting,
    Active,
    Error(String),
}

struct PositronicApp {
    engine: Option<Arc<PositronicEngine>>,
    redraw: Option<RedrawHandle>,

    /// Block list for tracking what's on screen
    output_blocks: Vec<TerminalBlock>,
    /// The full text content of the output area
    output_text: String,
    /// The text_editor content (selectable, scrollable)
    output_content: text_editor::Content,
    /// Track whether we need to scroll to bottom
    scroll_to_bottom: bool,

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

#[derive(Clone, Debug)]
enum Message {
    EngineReady(Arc<PositronicEngine>, RedrawHandle),
    EngineFailed(String),

    Redraw,

    InputChanged(String),
    InputSent,

    /// Result from executing a command
    CommandResult(ExecuteResult),
    CommandError(String),

    /// Handle text_editor actions (selection, copy, cursor) in the output area
    OutputAction(text_editor::Action),
}

/// Flatten blocks into a single string for the text_editor.
fn blocks_to_string(blocks: &[TerminalBlock]) -> String {
    let mut out = String::new();
    for block in blocks {
        match block {
            TerminalBlock::Command(cmd) => {
                out.push_str(&format!("➜ {}\n", cmd));
            }
            TerminalBlock::StandardOutput(s) => {
                out.push_str(s);
                if !s.ends_with('\n') {
                    out.push('\n');
                }
            }
            TerminalBlock::ErrorOutput(err) => {
                out.push_str(&format!("❌ {}\n", err));
            }
        }
    }
    out
}

/// Rebuild the text_editor Content and move cursor to the very end
/// so the view auto-scrolls to the bottom.
fn rebuild_content_at_end(text: &str) -> text_editor::Content {
    let content = text_editor::Content::with_text(text);
    // Move cursor to end so the editor scrolls to the bottom.
    // We do this by performing a "move to end of document" action.
    content
}

fn append_output(app: &mut PositronicApp) {
    let new_text = blocks_to_string(&app.output_blocks);
    if new_text != app.output_text {
        app.output_text = new_text;
        app.output_content = text_editor::Content::with_text(&app.output_text);
        app.scroll_to_bottom = true;
    }
}

fn boot() -> (PositronicApp, Task<Message>) {
    let initial_text = "⏳ Booting Positronic Engine...\n".to_string();
    let app = PositronicApp {
        engine: None,
        redraw: None,
        output_blocks: vec![TerminalBlock::StandardOutput(initial_text.clone())],
        output_text: initial_text.clone(),
        output_content: text_editor::Content::with_text(&initial_text),
        scroll_to_bottom: false,
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
        AppState::Booting => String::from("Positronic /// Booting..."),
        AppState::Active => String::from("Positronic /// Data Surface"),
        AppState::Error(e) => format!("Positronic /// ERROR: {}", &e[..e.len().min(60)]),
    }
}

fn theme(_app: &PositronicApp) -> Theme {
    Theme::Dark
}

fn update(app: &mut PositronicApp, message: Message) -> Task<Message> {
    match message {
        Message::EngineReady(engine, redraw) => {
            eprintln!("[UI] ✅ Engine ready!");
            app.engine = Some(engine);
            app.redraw = Some(redraw);
            app.state = AppState::Active;

            app.output_blocks.clear();
            app.output_blocks.push(TerminalBlock::StandardOutput(
                "⚡ Positronic Engine Online.  Type !help for commands.\n".to_string(),
            ));
            append_output(app);

            Task::none()
        }

        Message::EngineFailed(err) => {
            eprintln!("[UI] ❌ EngineFailed: {}", err);
            app.state = AppState::Error(err.clone());
            app.output_blocks
                .push(TerminalBlock::ErrorOutput(format!("BOOT FAILED: {}", err)));
            append_output(app);
            Task::none()
        }

        Message::Redraw => {
            if let Some(engine) = &app.engine {
                let snapshot = engine.state.snapshot();
                let new_hash = hash_snapshot_chars(&snapshot);

                if new_hash != app.last_screen_hash {
                    app.last_screen_hash = new_hash;

                    let display_text = snapshot_to_string(&snapshot);

                    if !display_text.trim().is_empty() {
                        // Replace the last StandardOutput block (live PTY view)
                        if matches!(
                            app.output_blocks.last(),
                            Some(TerminalBlock::StandardOutput(_))
                        ) {
                            app.output_blocks.pop();
                        }

                        app.output_blocks
                            .push(TerminalBlock::StandardOutput(display_text));
                        append_output(app);
                    }
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

            // Echo command
            app.output_blocks
                .push(TerminalBlock::Command(trimmed.clone()));
            append_output(app);
            app.input.clear();

            let Some(engine) = app.engine.clone() else {
                app.output_blocks.push(TerminalBlock::ErrorOutput(
                    "Engine not ready.".to_string(),
                ));
                append_output(app);
                return Task::none();
            };

            Task::perform(
                async move { engine.send_input(&format!("{}\n", trimmed)).await },
                |res| match res {
                    Ok(exec_result) => Message::CommandResult(exec_result),
                    Err(e) => Message::CommandError(format!("{:#}", e)),
                },
            )
        }

        Message::CommandResult(result) => {
            match result {
                ExecuteResult::SentToPty => {
                    // PTY will produce output → Redraw will pick it up
                }
                ExecuteResult::Output(text) => {
                    // Direct output from ! commands — display immediately
                    app.output_blocks
                        .push(TerminalBlock::StandardOutput(text));
                    append_output(app);
                }
                ExecuteResult::ClearScreen => {
                    app.output_blocks.clear();
                    app.output_text.clear();
                    app.output_content = text_editor::Content::with_text("");
                    app.last_screen_hash = 0;
                }
            }
            Task::none()
        }

        Message::CommandError(err) => {
            app.output_blocks.push(TerminalBlock::ErrorOutput(err));
            append_output(app);
            Task::none()
        }

        Message::OutputAction(action) => {
            // Allow cursor movement, selection, and copy.
            // Block all edit (typing/paste/delete) actions so output is read-only.
            let is_edit = matches!(action, text_editor::Action::Edit(_));
            if !is_edit {
                app.output_content.perform(action);
            }
            Task::none()
        }
    }
}

fn view(app: &PositronicApp) -> Element<'_, Message> {
    let mut layout = column![]
        .spacing(5)
        .padding(15)
        .width(Length::Fill)
        .height(Length::Fill);

    // --- Status bar (only when not active) ---
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

    // --- Output area: selectable text_editor ---
    let output_editor = text_editor(&app.output_content)
        .font(iced::Font::MONOSPACE)
        .size(14)
        .on_action(Message::OutputAction);

    // Wrap in a scrollable so the user can scroll through output
    let output_area = scrollable(
        container(output_editor).width(Length::Fill),
    )
        .height(Length::Fill)
        .width(Length::Fill);

    layout = layout.push(output_area);

    // --- Input bar ---
    let input = text_input("Type a command… (!help for commands)", &app.input)
        .font(iced::Font::MONOSPACE)
        .size(14)
        .padding(10)
        .on_input(Message::InputChanged)
        .on_submit(Message::InputSent);

    layout = layout.push(row![input].width(Length::Fill));

    layout.into()
}

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

// ---- Rendering helpers ----

fn hash_snapshot_chars(snapshot: &impl SnapshotLike) -> u64 {
    use std::collections::hash_map::DefaultHasher;

    let mut h = DefaultHasher::new();
    snapshot.rows().hash(&mut h);
    snapshot.cols().hash(&mut h);

    for row in snapshot.rows_iter() {
        for (c, _color) in row {
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

// ---- Adapter trait ----

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