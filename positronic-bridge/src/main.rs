use iced::futures::SinkExt;
use iced::widget::{column, row, scrollable, text, text_input};
use iced::{Element, Length, Settings, Subscription, Task, Theme};

use positronic_bridge::holodeck::TerminalBlock;
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

    output_blocks: Vec<TerminalBlock>,
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
    CommandError(String),
}

fn boot() -> (PositronicApp, Task<Message>) {
    let app = PositronicApp {
        engine: None,
        redraw: None,
        output_blocks: Vec::new(),
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

            app.output_blocks.push(TerminalBlock::StandardOutput(
                "Positronic Engine Online. Type a command below.\n".to_string(),
            ));

            Task::none()
        }

        Message::EngineFailed(err) => {
            eprintln!("[UI] ❌ EngineFailed: {}", err);
            app.state = AppState::Error(err.clone());
            app.output_blocks
                .push(TerminalBlock::ErrorOutput(format!("BOOT FAILED: {}", err)));
            Task::none()
        }

        Message::Redraw => {
            if let Some(engine) = &app.engine {
                // Snapshot type is whatever your engine returns; current code in your repo
                // already uses `engine.state.snapshot()`.
                let snapshot = engine.state.snapshot();

                let new_hash = hash_snapshot_chars(&snapshot);

                if new_hash != app.last_screen_hash {
                    app.last_screen_hash = new_hash;

                    let display_text = snapshot_to_string(&snapshot);

                    if !display_text.trim().is_empty() {
                        if matches!(
                            app.output_blocks.last(),
                            Some(TerminalBlock::StandardOutput(_))
                        ) {
                            app.output_blocks.pop();
                        }

                        app.output_blocks
                            .push(TerminalBlock::StandardOutput(display_text));
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

            // Echo command immediately
            app.output_blocks
                .push(TerminalBlock::Command(trimmed.clone()));
            app.input.clear();

            let Some(engine) = app.engine.clone() else {
                app.output_blocks.push(TerminalBlock::ErrorOutput(
                    "Engine not ready. Check console for boot errors.".to_string(),
                ));
                return Task::none();
            };

            Task::perform(
                async move { engine.send_input(&format!("{}\n", trimmed)).await },
                |res| match res {
                    Ok(()) => Message::Redraw,
                    Err(e) => Message::CommandError(format!("Error: {:#}", e)),
                },
            )
        }

        Message::CommandError(err) => {
            app.output_blocks.push(TerminalBlock::ErrorOutput(err));
            Task::none()
        }
    }
}

fn view(app: &PositronicApp) -> Element<'_, Message> {
    let mut content = column![].spacing(5);

    match &app.state {
        AppState::Booting => {
            content = content.push(
                text("⏳ Booting engine... check console for progress.")
                    .font(iced::Font::MONOSPACE)
                    .size(14),
            );
        }
        AppState::Error(e) => {
            content = content.push(
                text(format!("❌ {}", e))
                    .font(iced::Font::MONOSPACE)
                    .size(14),
            );
        }
        AppState::Active => {}
    }

    let output_column = app
        .output_blocks
        .iter()
        .fold(content, |col, block| col.push(block.view()));

    let term_area = scrollable(output_column).height(Length::Fill);

    let input = text_input("Type a command…", &app.input)
        .font(iced::Font::MONOSPACE)
        .size(14)
        .padding(10)
        .on_input(Message::InputChanged)
        .on_submit(Message::InputSent);

    let input_area = row![input].width(Length::Fill);

    column![term_area, input_area]
        .padding(15)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
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
// These are intentionally “char-only” to avoid needing Hash on your color enum.
// If you want color sensitivity too, we can hash colors as well.

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

// ---- Adapter trait (so this main.rs doesn't care if Snapshot is your flat vec type) ----
// Your current Snapshot implements `rows()`, `cols()`, and `IntoIterator for &Snapshot`.
// We bridge that here without adding any new requirements to your core crate.

trait SnapshotLike {
    fn rows(&self) -> usize;
    fn cols(&self) -> usize;
    fn rows_iter<'a>(
        &'a self,
    ) -> Box<dyn Iterator<Item = &'a [(char, positronic_core::state_machine::MyColor)]> + 'a>;
}

// If your Snapshot type path differs, adjust this `impl` to match.
// From your pasted core file: `SnapshotCell = (char, MyColor)` and `IntoIterator for &Snapshot`.
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
