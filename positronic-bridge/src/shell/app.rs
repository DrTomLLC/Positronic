//! Application state and winit event loop.
//!
//! Replaces the old iced app.rs + update.rs + messages.rs.
//! All business logic (engine boot, command dispatch, history, etc.) is preserved.

use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

use positronic_core::state_machine::Snapshot;
use positronic_core::PositronicEngine;
use tokio::sync::mpsc;

use crate::completer;
use crate::cwd::{track_cd_command, update_cwd_from_snapshot};
use crate::gfx::GpuState;
use crate::renderer::ThemeName;
use crate::ui;

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

    // ── Tokio runtime handle (for async engine ops) ──
    pub rt: tokio::runtime::Handle,
}

// ════════════════════════════════════════════════════════════════════
// Output Management
// ════════════════════════════════════════════════════════════════════

impl PositronicApp {
    pub fn push_direct(&mut self, new_text: &str) {
        self.direct_output.push_str(new_text);
        if !new_text.ends_with('\n') {
            self.direct_output.push('\n');
        }

        if self.direct_output.len() > MAX_DIRECT_BYTES {
            let mid = self.direct_output.len() / 2;
            if let Some(nl) = self.direct_output[mid..].find('\n') {
                let trim_at = mid + nl + 1;
                let kept = self.direct_output[trim_at..].to_string();
                self.direct_output = format!("··· (older output trimmed) ···\n{}", kept);
            }
        }
    }

    /// Poll for PTY redraw notifications (non-blocking).
    pub(crate) fn poll_redraws(&mut self) -> bool {
        let Some(rx) = &mut self.redraw_rx else {
            return false;
        };
        let mut got_any = false;
        while rx.try_recv().is_ok() {
            got_any = true;
        }
        if got_any {
            self.update_snapshot();
        }
        got_any
    }

    /// Refresh the snapshot from the engine.
    fn update_snapshot(&mut self) {
        if let Some(engine) = &self.engine {
            let snapshot = engine.state.snapshot();
            let new_hash = crate::helpers::hash_snapshot(&snapshot);
            if new_hash != self.last_screen_hash {
                self.last_screen_hash = new_hash;
                update_cwd_from_snapshot(&snapshot, &mut self.cwd);
                self.last_snapshot = Some(snapshot);

                if !self.direct_output.is_empty() && self.direct_output.starts_with("⚡") {
                    self.direct_output.clear();
                }
            }
        }
    }

    /// Request a window redraw.
    pub(crate) fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

// ════════════════════════════════════════════════════════════════════
// Command Execution
// ════════════════════════════════════════════════════════════════════

impl PositronicApp {
    /// Submit the current input as a command.
    pub fn submit_input(&mut self) {
        let trimmed = self.input.trim().to_string();
        if trimmed.is_empty() {
            return;
        }

        self.cmd_history.push(trimmed.clone());
        self.history_cursor = None;
        self.tab_state = None;
        self.session_cmd_count += 1;
        self.input.clear();
        self.cursor_pos = 0;

        track_cd_command(&trimmed, &mut self.cwd);

        // Local theme handling
        if trimmed.starts_with("!theme") {
            self.handle_theme_command(&trimmed);
            return;
        }

        // Local exit handling
        if trimmed == "!exit" || trimmed == "!quit" {
            // Will be handled by the event loop
            if let Some(window) = &self.window {
                // Request close — the event loop will handle it
                // For now just push a message
                self.push_direct("  👋 Goodbye!");
            }
            return;
        }

        // Local clear
        if trimmed == "!clear" || trimmed == "clear" || trimmed == "cls" {
            self.direct_output.clear();
            self.last_snapshot = None;
            if let Some(engine) = &self.engine {
                let engine = engine.clone();
                self.rt.spawn(async move {
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
            return;
        }

        // Local clipboard copy
        if trimmed == "!copy" {
            self.handle_copy();
            return;
        }

        // Execute via engine
        if let Some(engine) = &self.engine {
            self.push_direct(&format!("➜ {}", trimmed));
            let engine = engine.clone();
            let cmd = trimmed.clone();
            self.rt.spawn(async move {
                match engine.execute(&cmd).await {
                    Ok(_result) => {}
                    Err(e) => {
                        tracing::error!("Command failed: {:#}", e);
                    }
                }
            });
        } else {
            self.push_direct("❌ Engine not ready.");
        }
    }

    fn handle_theme_command(&mut self, cmd: &str) {
        let args: Vec<&str> = cmd.split_whitespace().collect();
        if args.len() < 2 {
            let names: Vec<&str> = ThemeName::all().iter().map(|t| t.label()).collect();
            self.push_direct(&format!("🎨 Current theme: {}", self.theme_name.label()));
            self.push_direct(&format!("   Available: {}", names.join(", ")));
        } else if let Some(new_theme) = ThemeName::from_str(args[1]) {
            self.theme_name = new_theme;
            self.push_direct(&format!("🎨 Theme switched to: {}", new_theme.label()));
            if let Some(engine) = &self.engine {
                let _ = engine.runner.vault().set_config("theme", new_theme.label());
            }
        } else {
            self.push_direct(&format!("❌ Unknown theme: {}", args[1]));
        }
    }

    pub(crate) fn handle_copy(&mut self) {
        let mut text = String::new();
        if let Some(ref snapshot) = self.last_snapshot {
            text = crate::renderer::snapshot_to_plain(snapshot);
        } else if !self.direct_output.is_empty() {
            text = self.direct_output.clone();
        }

        match arboard::Clipboard::new() {
            Ok(mut clip) => {
                let _ = clip.set_text(text);
                self.push_direct("  📋 Copied to clipboard.");
            }
            Err(_) => {
                self.push_direct("  ⚠ Clipboard unavailable.");
            }
        }
    }

    /// Navigate history up.
    pub fn history_up(&mut self) {
        if self.cmd_history.is_empty() {
            return;
        }
        let idx = match self.history_cursor {
            Some(i) if i > 0 => i - 1,
            Some(i) => i,
            None => self.cmd_history.len() - 1,
        };
        self.history_cursor = Some(idx);
        self.input = self.cmd_history[idx].clone();
        self.cursor_pos = self.input.len();
        self.tab_state = None;
    }

    /// Navigate history down.
    pub fn history_down(&mut self) {
        let Some(cursor) = self.history_cursor else {
            return;
        };
        if cursor + 1 < self.cmd_history.len() {
            let idx = cursor + 1;
            self.history_cursor = Some(idx);
            self.input = self.cmd_history[idx].clone();
            self.cursor_pos = self.input.len();
        } else {
            self.history_cursor = None;
            self.input.clear();
            self.cursor_pos = 0;
        }
        self.tab_state = None;
    }

    /// Tab completion.
    pub fn tab_complete(&mut self) {
        if self.input.trim().is_empty() {
            return;
        }

        if let Some(ref mut state) = self.tab_state {
            let next = state.next().to_string();
            self.input = next;
            self.cursor_pos = self.input.len();
        } else {
            let aliases = crate::helpers::get_alias_names_from(self.engine.as_deref());
            if let Some(state) = completer::complete(&self.input, &aliases, &self.cwd) {
                let first = state.current().to_string();
                let count = state.len();
                self.tab_state = Some(state);
                self.input = first;
                self.cursor_pos = self.input.len();

                if count > 1 {
                    let all: Vec<String> = self
                        .tab_state
                        .as_ref()
                        .unwrap()
                        .completions
                        .iter()
                        .map(|c| {
                            c.rsplit_once(' ')
                                .map(|(_, r)| r)
                                .or_else(|| c.strip_prefix('!'))
                                .unwrap_or(c)
                                .to_string()
                        })
                        .collect();
                    self.push_direct(&format!("  💡 {} matches: {}", count, all.join("  ")));
                }
            }
        }
    }

    /// Send Ctrl+C interrupt to PTY.
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
// ApplicationHandler (winit 0.30)
// ════════════════════════════════════════════════════════════════════

impl ApplicationHandler for PositronicApp {
    fn resumed(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.window.is_some() {
            return; // Already created
        }

        let attrs = WindowAttributes::default()
            .with_title("Positronic /// Data Surface")
            .with_min_surface_size(PhysicalSize::new(1280u32, 800u32));

        match event_loop.create_window(attrs) {
            Ok(window) => {
                let window = Arc::new(window);
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

    fn about_to_wait(&mut self, _event_loop: &dyn ActiveEventLoop) {
        // Poll for PTY redraws
        if self.poll_redraws() {
            self.request_redraw();
        }
    }
fn can_create_surfaces(&mut self, _: &(dyn ActiveEventLoop + 'static)) { todo!() }
}

impl<T> PositronicApp {
    /// Boot the Positronic engine in the background.
    fn boot_engine(&mut self) {
        self.push_direct("⚡ Positronic v0.3.0 Online.  Type !help for commands.");

        let rt = self.rt.clone();
        let (redraw_tx, redraw_rx) = mpsc::channel(64);
        self.redraw_rx = Some(redraw_rx);

        // Clone what we need for the async block
        let window = self.window.clone();

        rt.spawn(async move {
            match PositronicEngine::boot(120, 30).await {
                Ok(engine) => {
                    let engine: std::sync::Arc<T> = Arc::new(engine);

                    // Start the PTY reader pump
                    let eng2 = engine.clone();
                    let tx = redraw_tx.clone();
                    tokio::spawn(async move {
                        loop {
                            match eng2.wait_for_output().await {
                                Ok(true) => {
                                    let _ = tx.send(()).await;
                                }
                                Ok(false) => break,
                                Err(_) => break,
                            }
                        }
                    });

                    // We need to communicate the engine back to the main thread.
                    // For now, we'll use a simple approach — store it via a static channel.
                    // (In production, use a proper cross-thread channel.)
                    if let Some(w) = window {
                        // Store engine for pickup
                        ENGINE_READY
                            .lock()
                            .unwrap()
                            .replace(engine);
                        w.request_redraw();
                    }
                }
                Err(e) => {
                    tracing::error!("Engine boot failed: {:#}", e);
                    ENGINE_ERROR
                        .lock()
                        .unwrap()
                        .replace(format!("{:#}", e));
                }
            }
        });
    }

    /// Check if the engine is ready (called from the event loop).
    pub fn check_engine_ready(&mut self) {
        if self.engine.is_some() {
            return;
        }

        if let Some(engine) = ENGINE_READY.lock().unwrap().take() {
            // Hydrate history from vault
            match engine.runner.vault().recent_unique(100) {
                Ok(history) => {
                    self.cmd_history = history.into_iter().rev().collect();
                    tracing::info!("Hydrated {} commands from Vault", self.cmd_history.len());
                }
                Err(e) => tracing::warn!("Failed to load history: {}", e),
            }

            // Load saved theme
            if let Ok(Some(saved)) = engine.runner.vault().get_config("theme") {
                if let Some(t) = ThemeName::from_str(&saved) {
                    self.theme_name = t;
                }
            }

            self.engine = Some(engine);
            self.state = AppState::Active;
            self.direct_output.clear();
            self.last_snapshot = None;
            self.push_direct("⚡ Positronic v0.3.0 Online.  Type !help for commands.");
        }

        if let Some(err) = ENGINE_ERROR.lock().unwrap().take() {
            self.state = AppState::Error(err.clone());
            self.push_direct(&format!("❌ BOOT FAILED: {}", err));
        }
    }
}

// Cross-thread engine delivery (simple approach for single-window app)
use std::sync::Mutex;
use crossterm::ExecutableCommand;

static ENGINE_READY: std::sync::LazyLock<Mutex<Option<Arc<PositronicEngine>>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));
static ENGINE_ERROR: std::sync::LazyLock<Mutex<Option<String>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

// ════════════════════════════════════════════════════════════════════
// Entry Point
// ════════════════════════════════════════════════════════════════════

/// Create and run the Positronic application. This is the main entry point.
pub fn run() -> anyhow::Result<()> {
    // Build the tokio runtime for async engine operations
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let handle = rt.handle().clone();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = PositronicApp {
        window: None,
        gpu: None,
        engine: None,
        redraw_rx: None,
        state: AppState::Booting,
        direct_output: "⏳ Booting Positronic Engine...\n".to_string(),
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
        cwd: std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string()),
        theme_name: ThemeName::Default,
        rt: handle,
    };

    // Keep the runtime alive for the duration of the event loop
    let _guard = rt.enter();

    event_loop.run_app(&mut app)?;

    Ok(())
}