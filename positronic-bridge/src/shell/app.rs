// positronic-bridge/src/shell/app.rs
//! Application state and winit event loop.
//!
//! Replaces the old iced app.rs + update.rs + messages.rs.
//! All business logic (engine boot, command dispatch, history, etc.) is preserved.
//!
//! KEY ARCHITECTURAL NOTES:
//!   - Engine is booted via `PositronicEngine::start(cols, rows, redraw_tx)`.
//!     The engine internally spawns the PTY reader pump that feeds bytes
//!     through the state machine and fires `redraw_tx` on new output.
//!   - Command results (`ExecuteResult`) are delivered back to the main thread
//!     via a dedicated `cmd_result_rx` channel, polled alongside PTY redraws.
//!   - Engine is communicated from the async boot task to the main thread via
//!     `LazyLock<Mutex<Option>>` statics (single-window app pattern).

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

use crate::completer;
use crate::cwd::{track_cd_command, update_cwd_from_snapshot};
use crate::gfx::GpuState;
use crate::renderer::ThemeName;

// ════════════════════════════════════════════════════════════════════
// Application State
// ════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Booting,
    Active,
    Error(String),
}

/// Max direct output buffer (~256KB). Oldest half trimmed when exceeded.
pub const MAX_DIRECT_BYTES: usize = 256 * 1024;

/// The Positronic application. Owns all state.
pub struct PositronicApp {
    // ── Window + GPU ──
    pub window: Option<Arc<dyn Window>>,
    pub gpu: Option<GpuState>,

    // ── Engine ──
    pub engine: Option<Arc<PositronicEngine>>,
    pub redraw_rx: Option<mpsc::Receiver<()>>,
    pub cmd_result_tx: mpsc::Sender<CmdResult>,
    pub cmd_result_rx: mpsc::Receiver<CmdResult>,

    // ── Terminal State ──
    pub state: AppState,
    pub direct_output: String,
    pub last_snapshot: Option<Snapshot>,
    pub last_screen_hash: u64,

    // ── Input ──
    pub input: String,
    pub cursor_pos: usize,
    pub composing: bool,

    // ── History ──
    pub cmd_history: Vec<String>,
    pub history_cursor: Option<usize>,

    // ── Tab Completion ──
    pub tab_state: Option<completer::CompletionState>,

    // ── Status ──
    pub session_cmd_count: usize,
    pub boot_instant: Instant,
    pub cwd: String,
    pub theme_name: ThemeName,

    // ── Keyboard modifiers (tracked via WindowEvent::ModifiersChanged) ──
    pub modifiers: ModifiersState,

    // ── Shutdown flag (set by !exit, checked by event loop) ──
    pub wants_exit: bool,

    // ── Tokio runtime handle (for async engine ops) ──
    pub rt: tokio::runtime::Handle,
}

/// Result delivered from async command execution back to the main thread.
pub enum CmdResult {
    Executed(ExecuteResult),
    Error(String),
}

// ════════════════════════════════════════════════════════════════════
// Engine statics (single-window pattern)
// ════════════════════════════════════════════════════════════════════

use std::sync::{LazyLock, Mutex};

static ENGINE_READY: LazyLock<Mutex<Option<Arc<PositronicEngine>>>> =
    LazyLock::new(|| Mutex::new(None));
static ENGINE_ERROR: LazyLock<Mutex<Option<String>>> = LazyLock::new(|| Mutex::new(None));

// ════════════════════════════════════════════════════════════════════
// Core Application Methods
// ════════════════════════════════════════════════════════════════════

impl PositronicApp {
    /// Push text to the direct output buffer, trimming if needed.
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

    /// Request a window redraw.
    pub fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// Poll the PTY redraw channel. Returns true if new output was available.
    pub fn poll_redraws(&mut self) -> bool {
        let mut changed = false;
        if let Some(rx) = &mut self.redraw_rx {
            while rx.try_recv().is_ok() {
                changed = true;
            }
        }
        if changed {
            // Grab a fresh snapshot — StateMachine::snapshot() is synchronous
            if let Some(engine) = &self.engine {
                let snap = engine.state.snapshot();
                update_cwd_from_snapshot(&snap, &mut self.cwd);
                self.last_snapshot = Some(snap);
            }
        }
        changed
    }

    /// Poll the command result channel. Returns true if results were processed.
    pub fn poll_cmd_results(&mut self) -> bool {
        let mut changed = false;
        while let Ok(result) = self.cmd_result_rx.try_recv() {
            changed = true;
            self.handle_cmd_result(result);
        }
        changed
    }

    /// Handle a single command result.
    fn handle_cmd_result(&mut self, result: CmdResult) {
        match result {
            CmdResult::Executed(exec_result) => {
                self.handle_execute_result(exec_result);
            }
            CmdResult::Error(e) => {
                self.push_direct(&format!("❌ {}", e));
            }
        }
    }

    /// Process an ExecuteResult from the engine.
    fn handle_execute_result(&mut self, result: ExecuteResult) {
        match result {
            ExecuteResult::SentToPty => {
                // Output arrives via snapshots — nothing to do
            }
            ExecuteResult::DirectOutput(lines) => {
                self.push_direct(&lines.join("\n"));
            }
            ExecuteResult::ClearScreen => {
                self.direct_output.clear();
                self.last_snapshot = None;
            }
            ExecuteResult::Exit => {
                self.wants_exit = true;
            }
        }
    }

    /// Submit a command for execution.
    pub fn submit_command(&mut self) {
        let cmd = self.input.trim().to_string();
        if cmd.is_empty() {
            return;
        }

        // Record in history
        self.cmd_history.push(cmd.clone());
        self.history_cursor = None;
        self.session_cmd_count += 1;

        // Clear input
        self.input.clear();
        self.cursor_pos = 0;
        self.tab_state = None;

        // Handle local-only commands
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

        // Track cd commands for CWD display
        track_cd_command(&cmd, &mut self.cwd);

        // Send to engine
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

    /// Send an interrupt (Ctrl+C) to the PTY.
    pub fn send_interrupt(&self) {
        if let Some(engine) = &self.engine {
            let engine = engine.clone();
            self.rt.spawn(async move {
                let _ = engine.send_interrupt().await;
            });
        }
    }

    /// Send Escape to PTY.
    pub fn send_escape(&self) {
        if let Some(engine) = &self.engine {
            let engine = engine.clone();
            self.rt.spawn(async move {
                let _ = engine.send_escape().await;
            });
        }
    }

    /// Send Ctrl+D EOF to PTY.
    pub fn send_eof(&self) {
        if let Some(engine) = &self.engine {
            let engine = engine.clone();
            self.rt.spawn(async move {
                let _ = engine.send_eof().await;
            });
        }
    }
}

// ════════════════════════════════════════════════════════════════════
// ApplicationHandler (winit 0.31)
// ════════════════════════════════════════════════════════════════════

impl ApplicationHandler for PositronicApp {
    /// winit 0.31: can_create_surfaces replaces resumed() for window/surface creation
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.window.is_some() {
            return; // Already created
        }

        // winit 0.31: with_inner_size → with_surface_size
        let attrs = WindowAttributes::default()
            .with_title("Positronic /// Data Surface")
            .with_surface_size(PhysicalSize::new(1280u32, 800u32));

        match event_loop.create_window(attrs) {
            Ok(window) => {
                // winit 0.31: create_window returns Box<dyn Window>
                // Convert to Arc<dyn Window> for shared ownership
                let window: Arc<dyn Window> = Arc::from(window);
                match GpuState::new(window.clone()) {
                    Ok(gpu) => {
                        self.gpu = Some(gpu);
                        self.window = Some(window);
                        tracing::info!("Window + GPU initialized");

                        // Boot the engine
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
        // Delegate to the events module
        super::events::handle_window_event(self, event_loop, event);
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        // Poll for PTY redraws
        let pty_changed = self.poll_redraws();

        // Poll for command results (DirectOutput, ClearScreen, Exit, etc.)
        let cmd_changed = self.poll_cmd_results();

        if pty_changed || cmd_changed {
            self.request_redraw();
        }

        // Honor the exit flag
        if self.wants_exit {
            event_loop.exit();
        }
    }
}

// ════════════════════════════════════════════════════════════════════
// Engine Boot
// ════════════════════════════════════════════════════════════════════

impl PositronicApp {
    /// Boot the Positronic engine in the background.
    ///
    /// Creates a PTY redraw channel and passes it to `PositronicEngine::start()`.
    /// The engine internally spawns the PTY reader pump that feeds bytes through
    /// the state machine. When new output arrives, the engine fires `redraw_tx`
    /// which we poll in `about_to_wait()`.
    fn boot_engine(&mut self) {
        self.push_direct("⏳ Booting Positronic Engine...");

        let rt = self.rt.clone();
        let (redraw_tx, redraw_rx) = mpsc::channel(64);
        self.redraw_rx = Some(redraw_rx);

        // Clone what we need for the async block
        let window = self.window.clone();

        rt.spawn(async move {
            // Pass redraw_tx directly to the engine — it owns the PTY reader pump.
            // No wait_for_output() loop needed — the engine handles everything internally.
            match PositronicEngine::start(120, 30, redraw_tx).await {
                Ok(engine) => {
                    let engine = Arc::new(engine);

                    // Store engine for main-thread pickup
                    ENGINE_READY.lock().unwrap().replace(engine);

                    // Wake the event loop to pick up the engine
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

    /// Check if the engine is ready (called from the event loop on RedrawRequested).
    pub fn check_engine_ready(&mut self) {
        if self.engine.is_some() {
            return; // Already have it
        }

        // Check for engine error first
        if let Some(err) = ENGINE_ERROR.lock().unwrap().take() {
            self.state = AppState::Error(err.clone());
            self.push_direct(&format!("❌ Engine boot failed: {}", err));
            return;
        }

        // Check for engine ready
        if let Some(engine) = ENGINE_READY.lock().unwrap().take() {
            self.engine = Some(engine);
            self.state = AppState::Active;
            self.push_direct("✅ Engine ready");
            self.push_direct("Type a command, or !help for built-in commands.");

            // Grab initial snapshot — StateMachine::snapshot() is synchronous
            if let Some(engine) = &self.engine {
                let snap = engine.state.snapshot();
                update_cwd_from_snapshot(&snap, &mut self.cwd);
                self.last_snapshot = Some(snap);
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════
// Entry Point
// ════════════════════════════════════════════════════════════════════

/// Run the Positronic terminal application.
pub fn run() -> anyhow::Result<()> {
    // Tracing is initialized once in the binary entry point (src/main.rs).
    // Do not install a global subscriber from library code.

    tracing::info!("Positronic v0.3.0 starting...");

    // Build tokio runtime
    let rt = tokio::runtime::Runtime::new()?;
    let rt_handle = rt.handle().clone();

    // Create event loop
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);

    // Create command result channel
    let (cmd_result_tx, cmd_result_rx) = mpsc::channel(64);

    // Get initial CWD
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Build application state
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
        tab_state: None,
        session_cmd_count: 0,
        boot_instant: Instant::now(),
        cwd,
        theme_name: ThemeName::Default,
        modifiers: ModifiersState::empty(),
        wants_exit: false,
        rt: rt_handle,
    };

    // winit 0.31: run_app requires A: 'static.
    // Box::leak satisfies the lifetime requirement. This is fine because
    // run_app runs until the process exits — it never returns normally.
    let app = Box::leak(Box::new(app));
    event_loop.run_app(app)?;

    Ok(())
}