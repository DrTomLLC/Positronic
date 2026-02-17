use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowAttributes, WindowId};

use positronic_core::engine::ExecuteResult;
use positronic_core::state_machine::Snapshot;
use positronic_core::PositronicEngine;
use tokio::sync::mpsc;

use crate::cwd::{track_cd_command, update_cwd_from_snapshot};
use crate::gfx::GpuState;
use crate::renderer::{self, ThemeName};

use positronic_core::term::modes::ModeTracker;
use positronic_core::term::osc::OscParser;
use positronic_core::term::semantic::SemanticState;

use crate::holodeck::{detect, protocol::HolodeckDoc};
use crate::holodeck::protocol::Action;

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Booting,
    Active,
    Error(String),
}

pub const MAX_DIRECT_BYTES: usize = 256 * 1024;

pub struct PositronicApp {
    pub window: Option<Arc<dyn Window>>,
    pub gpu: Option<GpuState>,

    pub engine: Option<Arc<PositronicEngine>>,
    pub redraw_rx: Option<mpsc::Receiver<()>>,
    pub cmd_result_tx: mpsc::Sender<CmdResult>,
    pub cmd_result_rx: mpsc::Receiver<CmdResult>,

    pub state: AppState,
    pub direct_output: String,
    pub last_snapshot: Option<Snapshot>,
    pub last_screen_hash: u64,

    pub input: String,
    pub cursor_pos: usize,
    pub composing: bool,

    pub cmd_history: Vec<String>,
    pub history_cursor: Option<usize>,

    pub session_cmd_count: usize,
    pub boot_instant: Instant,
    pub cwd: String,
    pub theme_name: ThemeName,

    pub modifiers: ModifiersState,
    pub wants_exit: bool,
    pub rt: tokio::runtime::Handle,

    // --- new: semantic gate + holodeck ---
    pub mode_tracker: ModeTracker,
    pub osc_parser: OscParser,
    pub semantic: SemanticState,

    pub holodeck_doc: Option<HolodeckDoc>,
    pub holodeck_safe: bool,

    pub last_mouse_x: f32,
    pub last_mouse_y: f32,
}

pub enum CmdResult {
    Executed(ExecuteResult),
    Error(String),
}

use std::sync::{LazyLock, Mutex};

static ENGINE_READY: LazyLock<Mutex<Option<Arc<PositronicEngine>>>> =
    LazyLock::new(|| Mutex::new(None));
static ENGINE_ERROR: LazyLock<Mutex<Option<String>>> = LazyLock::new(|| Mutex::new(None));

impl PositronicApp {
    pub fn push_direct(&mut self, text: &str) {
        self.direct_output.push_str(text);
        self.direct_output.push('\n');

        if self.direct_output.len() > MAX_DIRECT_BYTES {
            let half = self.direct_output.len() / 2;
            let boundary = self.direct_output[half..]
                .find('\n')
                .map(|i| half + i + 1)
                .unwrap_or(half);
            self.direct_output = self.direct_output[boundary..].to_string();
        }
    }

    pub fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    pub fn poll_redraws(&mut self) -> bool {
        let mut changed = false;
        if let Some(rx) = &mut self.redraw_rx {
            while rx.try_recv().is_ok() {
                changed = true;
            }
        }

        if changed {
            if let Some(engine) = &self.engine {
                // Drain bytes for semantic + mode tracking
                let bytes = engine.drain_pty_output();
                if !bytes.is_empty() {
                    self.mode_tracker.feed(&bytes);
                    for ev in self.osc_parser.feed(&bytes) {
                        self.semantic.apply(&ev);
                    }
                }

                // Snapshot for display
                let snap = engine.state.snapshot();
                if let Some(cwd) = &self.semantic.cwd {
                    self.cwd = cwd.clone();
                } else {
                    update_cwd_from_snapshot(&snap, &mut self.cwd);
                }
                self.last_snapshot = Some(snap.clone());

                // Holodeck safe gate: only show overlay at prompt + safe modes
                self.holodeck_safe = self.semantic.in_prompt && self.mode_tracker.snapshot().intelli_safe();

                // Detect content from what user can see (snapshot → plain)
                let plain = renderer::snapshot_to_plain(&snap);
                let rich = detect::detect_rich(&plain);
                self.holodeck_doc = Some(HolodeckDoc::from_rich(&rich));
            }
        }

        changed
    }

    pub fn poll_cmd_results(&mut self) -> bool {
        let mut changed = false;
        while let Ok(result) = self.cmd_result_rx.try_recv() {
            changed = true;
            self.handle_cmd_result(result);
        }
        changed
    }

    fn handle_cmd_result(&mut self, result: CmdResult) {
        match result {
            CmdResult::Executed(exec_result) => self.handle_execute_result(exec_result),
            CmdResult::Error(e) => self.push_direct(&format!("❌ {}", e)),
        }
    }

    fn handle_execute_result(&mut self, result: ExecuteResult) {
        match result {
            ExecuteResult::SentToPty => {}
            ExecuteResult::DirectOutput(lines) => self.push_direct(&lines.join("\n")),
            ExecuteResult::ClearScreen => {
                self.direct_output.clear();
                self.last_snapshot = None;
            }
            ExecuteResult::Exit => self.wants_exit = true,
        }
    }

    // ----- input editing helpers (keeps events.rs clean) -----

    pub fn input_insert(&mut self, c: &str) {
        if self.cursor_pos == self.input.chars().count() {
            self.input.push_str(c);
        } else {
            let byte_pos = self
                .input
                .char_indices()
                .nth(self.cursor_pos)
                .map(|(i, _)| i)
                .unwrap_or(self.input.len());
            self.input.insert_str(byte_pos, c);
        }
        self.cursor_pos += c.chars().count();
        self.history_cursor = None;
    }

    pub fn input_backspace(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let byte_pos = self
            .input
            .char_indices()
            .nth(self.cursor_pos - 1)
            .map(|(i, _)| i)
            .unwrap_or(0);
        let next_byte = self
            .input
            .char_indices()
            .nth(self.cursor_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.input.len());
        self.input.replace_range(byte_pos..next_byte, "");
        self.cursor_pos -= 1;
    }

    pub fn input_delete(&mut self) {
        let char_count = self.input.chars().count();
        if self.cursor_pos >= char_count {
            return;
        }
        let byte_pos = self
            .input
            .char_indices()
            .nth(self.cursor_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.input.len());
        let next_byte = self
            .input
            .char_indices()
            .nth(self.cursor_pos + 1)
            .map(|(i, _)| i)
            .unwrap_or(self.input.len());
        self.input.replace_range(byte_pos..next_byte, "");
    }

    pub fn input_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }
    pub fn input_right(&mut self) {
        if self.cursor_pos < self.input.chars().count() {
            self.cursor_pos += 1;
        }
    }
    pub fn input_home(&mut self) {
        self.cursor_pos = 0;
    }
    pub fn input_end(&mut self) {
        self.cursor_pos = self.input.chars().count();
    }

    pub fn history_up(&mut self) {
        if self.cmd_history.is_empty() {
            return;
        }
        let new_cursor = match self.history_cursor {
            None => self.cmd_history.len() - 1,
            Some(c) if c > 0 => c - 1,
            Some(c) => c,
        };
        self.history_cursor = Some(new_cursor);
        self.input = self.cmd_history[new_cursor].clone();
        self.cursor_pos = self.input.chars().count();
    }

    pub fn history_down(&mut self) {
        if let Some(c) = self.history_cursor {
            if c + 1 < self.cmd_history.len() {
                let new_cursor = c + 1;
                self.history_cursor = Some(new_cursor);
                self.input = self.cmd_history[new_cursor].clone();
                self.cursor_pos = self.input.chars().count();
            } else {
                self.history_cursor = None;
                self.input.clear();
                self.cursor_pos = 0;
            }
        }
    }

    // ----- command submit (still uses your Runner path) -----

    pub fn submit_command(&mut self) {
        let cmd = self.input.trim().to_string();
        if cmd.is_empty() {
            return;
        }

        self.cmd_history.push(cmd.clone());
        self.history_cursor = None;
        self.session_cmd_count += 1;

        self.input.clear();
        self.cursor_pos = 0;

        match cmd.as_str() {
            "!pwd" => {
                self.push_direct(&format!("📂 {}", self.cwd));
                return;
            }
            "!exit" | "!quit" => {
                self.wants_exit = true;
                return;
            }
            _ => {}
        }

        track_cd_command(&cmd, &mut self.cwd);

        if let Some(engine) = &self.engine {
            let engine = engine.clone();
            let tx = self.cmd_result_tx.clone();
            self.rt.spawn(async move {
                match engine.send_input(&cmd).await {
                    Ok(result) => {
                        let _ = tx.send(CmdResult::Executed(result)).await;
                    }
                    Err(e) => {
                        let _ = tx.send(CmdResult::Error(format!("{:#}", e))).await;
                    }
                }
            });
        } else {
            self.push_direct("⚠️  Engine not ready yet");
        }
    }

    pub fn send_interrupt(&self) {
        if let Some(engine) = &self.engine {
            let engine = engine.clone();
            self.rt.spawn(async move {
                let _ = engine.send_interrupt().await;
            });
        }
    }

    pub fn send_escape(&self) {
        if let Some(engine) = &self.engine {
            let engine = engine.clone();
            self.rt.spawn(async move {
                let _ = engine.send_escape().await;
            });
        }
    }

    pub fn send_eof(&self) {
        if let Some(engine) = &self.engine {
            let engine = engine.clone();
            self.rt.spawn(async move {
                let _ = engine.send_eof().await;
            });
        }
    }

    // --- Holodeck actions ---

    pub fn apply_holodeck_action(&mut self, action: Action) {
        match action {
            Action::CopyText(s) => {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(s);
                }
                self.push_direct("⚡ Holodeck: Copied to clipboard");
            }
            Action::RunCommand(cmd) => {
                self.input = cmd;
                self.cursor_pos = self.input.chars().count();
            }
            Action::None => {}
        }
    }

    pub fn copy_visible_to_clipboard(&mut self) {
        if let Some(snap) = &self.last_snapshot {
            let plain = renderer::snapshot_to_plain(snap);
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                let _ = clipboard.set_text(plain);
            }
            self.push_direct("⚡ Copied visible terminal to clipboard");
        }
    }
}

impl ApplicationHandler for PositronicApp {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title("Positronic /// Data Surface")
            .with_surface_size(PhysicalSize::new(1280u32, 800u32));

        match event_loop.create_window(attrs) {
            Ok(window) => {
                let window: Arc<dyn Window> = Arc::from(window);
                match GpuState::new(window.clone()) {
                    Ok(gpu) => {
                        self.gpu = Some(gpu);
                        self.window = Some(window);
                        tracing::info!("Window + GPU initialized");
                        self.boot_engine();
                    }
                    Err(e) => {
                        tracing::error!("GPU init failed: {:#}", e);
                        self.state = AppState::Error(format!("GPU init failed: {:#}", e));
                        self.window = Some(window);
                    }
                }
            }
            Err(e) => {
                tracing::error!("Window creation failed: {:#}", e);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        super::events::handle_window_event(self, event_loop, event);
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        let pty_changed = self.poll_redraws();
        let cmd_changed = self.poll_cmd_results();

        if pty_changed || cmd_changed {
            self.request_redraw();
        }

        if self.wants_exit {
            event_loop.exit();
        }
    }
}

impl PositronicApp {
    fn boot_engine(&mut self) {
        self.push_direct("⏳ Booting Positronic Engine...");

        let rt = self.rt.clone();
        let (redraw_tx, redraw_rx) = mpsc::channel(64);
        self.redraw_rx = Some(redraw_rx);

        let window = self.window.clone();

        rt.spawn(async move {
            match PositronicEngine::start(120, 30, redraw_tx).await {
                Ok(engine) => {
                    let engine = Arc::new(engine);
                    ENGINE_READY.lock().unwrap().replace(engine);
                    if let Some(w) = window {
                        w.request_redraw();
                    }
                }
                Err(e) => {
                    tracing::error!("Engine boot failed: {:#}", e);
                    ENGINE_ERROR.lock().unwrap().replace(format!("{:#}", e));
                    if let Some(w) = window {
                        w.request_redraw();
                    }
                }
            }
        });
    }

    pub fn check_engine_ready(&mut self) {
        if self.engine.is_some() {
            return;
        }

        if let Some(err) = ENGINE_ERROR.lock().unwrap().take() {
            self.state = AppState::Error(err.clone());
            self.push_direct(&format!("❌ Engine boot failed: {}", err));
            return;
        }

        if let Some(engine) = ENGINE_READY.lock().unwrap().take() {
            self.engine = Some(engine);
            self.state = AppState::Active;
            self.push_direct("✅ Engine ready");
            self.push_direct("Type a command, or !help for built-in commands.");

            if let Some(engine) = &self.engine {
                let snap = engine.state.snapshot();
                update_cwd_from_snapshot(&snap, &mut self.cwd);
                self.last_snapshot = Some(snap);
            }
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    tracing::info!("Positronic v0.3.0 starting...");

    let rt = tokio::runtime::Runtime::new()?;
    let rt_handle = rt.handle().clone();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let (cmd_result_tx, cmd_result_rx) = mpsc::channel(64);

    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let app = PositronicApp {
        window: None,
        gpu: None,
        engine: None,
        redraw_rx: None,
        cmd_result_tx,
        cmd_result_rx,
        state: AppState::Booting,
        direct_output: String::new(),
        last_snapshot: None,
        last_screen_hash: 0,
        input: String::new(),
        cursor_pos: 0,
        composing: false,
        cmd_history: Vec::new(),
        history_cursor: None,
        session_cmd_count: 0,
        boot_instant: Instant::now(),
        cwd,
        theme_name: ThemeName::Default,
        modifiers: ModifiersState::empty(),
        wants_exit: false,
        rt: rt_handle,

        mode_tracker: ModeTracker::new(),
        osc_parser: OscParser::new(),
        semantic: SemanticState::new(),
        holodeck_doc: None,
        holodeck_safe: false,

        last_mouse_x: 0.0,
        last_mouse_y: 0.0,
    };

    let app = Box::leak(Box::new(app));
    event_loop.run_app(app)?;
    Ok(())
}
