use iced::widget::{column, scrollable};
use iced::{Application, Command, Element, Length, Settings, Subscription, Theme, executor};
use positronic_core::PositronicEngine;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

use positronic_bridge::holodeck::TerminalBlock;
use positronic_bridge::input::InputEditor;

pub fn main() -> iced::Result {
    let settings = Settings {
        antialiasing: true,
        ..Settings::default()
    };
    PositronicApp::run(settings)
}

struct PositronicApp {
    engine: Option<Arc<PositronicEngine>>,
    output_blocks: Vec<TerminalBlock>,
    // We keep this wrapped to share with Subscription
    redraw_rx: Option<Arc<Mutex<mpsc::Receiver<()>>>>,
    input_editor: InputEditor,
    state: AppState,
}

#[derive(Debug, Clone, PartialEq)]
enum AppState {
    Booting,
    Active,
    Error(String),
}

#[derive(Clone, Debug)]
enum Message {
    Boot,
    EngineReady(Arc<PositronicEngine>, Arc<Mutex<mpsc::Receiver<()>>>),
    EngineFailed(String),
    Redraw,
    InputChanged(String),
    InputSent,
}

impl Application for PositronicApp {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let app = Self {
            engine: None,
            output_blocks: Vec::new(),
            redraw_rx: None,
            input_editor: InputEditor::new(),
            state: AppState::Booting,
        };

        // Send Boot message immediately to trigger async startup
        (app, Command::perform(async {}, |_| Message::Boot))
    }

    fn title(&self) -> String {
        match self.state {
            AppState::Booting => String::from("Positronic /// Booting..."),
            AppState::Active => String::from("Positronic /// Data Surface"),
            AppState::Error(_) => String::from("Positronic /// SYSTEM FAILURE"),
        }
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Boot => {
                Command::perform(
                    async {
                        let (tx, rx) = mpsc::channel(100);
                        // Start Engine
                        match PositronicEngine::start(80, 24, tx).await {
                            Ok(eng) => Ok((Arc::new(eng), Arc::new(Mutex::new(rx)))),
                            Err(e) => Err(e.to_string()),
                        }
                    },
                    |res| match res {
                        Ok((eng, rx)) => Message::EngineReady(eng, rx),
                        Err(e) => Message::EngineFailed(e),
                    },
                )
            }
            Message::EngineReady(engine, rx) => {
                self.engine = Some(engine);
                self.redraw_rx = Some(rx);
                self.state = AppState::Active;
                Command::none()
            }
            Message::EngineFailed(err) => {
                self.state = AppState::Error(err.clone());
                self.output_blocks
                    .push(TerminalBlock::ErrorOutput(format!("CRITICAL: {}", err)));
                Command::none()
            }
            Message::Redraw => {
                if let Some(engine) = &self.engine {
                    // Optimized: Only snapshot when told to
                    let raw_grid = engine.state.snapshot();
                    // Still inefficient string conversion, but strictly event-driven now
                    let display_text: String = raw_grid
                        .iter()
                        .map(|row| row.iter().map(|(c, _)| c).collect::<String>() + "\n")
                        .collect();

                    if let Some(TerminalBlock::StandardOutput(_)) = self.output_blocks.last() {
                        self.output_blocks.pop();
                    }
                    self.output_blocks
                        .push(TerminalBlock::StandardOutput(display_text));
                }
                // NOTE: We do NOT flush rx here anymore, usage of unfold handles it one by one
                Command::none()
            }
            Message::InputChanged(val) => {
                self.input_editor.value = val;
                Command::none()
            }
            Message::InputSent => {
                if let Some(engine) = &self.engine {
                    let engine = engine.clone();
                    let input = self.input_editor.value.clone();

                    self.output_blocks
                        .push(TerminalBlock::Command(input.clone()));
                    self.input_editor.value.clear();

                    return Command::perform(
                        async move {
                            // Ensure newline is sent
                            let _ = engine.send_input(&format!("{}\n", input)).await;
                        },
                        |_| Message::Redraw, // Optimistic local redraw? Or just wait for pipe?
                    );
                }
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let output_column = self
            .output_blocks
            .iter()
            .fold(column![].spacing(5), |col, block| col.push(block.view()));

        let term_area = scrollable(output_column).height(Length::Fill);

        let input_area = self
            .input_editor
            .view(Message::InputChanged, Message::InputSent);

        column![term_area, input_area]
            .padding(15)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        match &self.redraw_rx {
            Some(rx) => {
                // Event-Driven Subscription using unfold
                // State is the cloned Arc<Mutex<Receiver>>
                iced::subscription::unfold(
                    "redraw_subscription",
                    rx.clone(),
                    move |rx_arc| async move {
                        let mut guard = rx_arc.lock().await;
                        // Wait for next message
                        let _ = guard.recv().await;
                        // Yield Redraw and the state logic continues
                        (Message::Redraw, rx_arc.clone())
                    },
                )
            }
            None => Subscription::none(),
        }
    }
}
