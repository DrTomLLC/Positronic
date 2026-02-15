//! Application state and boot logic.

use crate::messages::Message;
use crate::renderer::ThemeName;

use positronic_core::state_machine::Snapshot;
use positronic_core::PositronicEngine;

use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

use iced::Task;

// ────────────────────────────────────────────────────────────────
// App State
// ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Booting,
    Active,
    Error(String),
}

// ────────────────────────────────────────────────────────────────
// Redraw Handle (wraps the mpsc receiver for PTY redraw events)
// ────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct RedrawHandle(pub Arc<Mutex<mpsc::Receiver<()>>>);

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

// ────────────────────────────────────────────────────────────────
// PositronicApp
// ────────────────────────────────────────────────────────────────

/// Max direct output buffer (~256KB). Oldest half trimmed when exceeded.
pub const MAX_DIRECT_BYTES: usize = 256 * 1024;

pub struct PositronicApp {
    pub engine: Option<Arc<PositronicEngine>>,
    pub redraw: Option<RedrawHandle>,

    /// Accumulated direct output (echoed commands, ! command output).
    pub direct_output: String,

    /// The latest PTY snapshot (with color data).
    pub last_snapshot: Option<Snapshot>,

    /// Command history and cursor for Up/Down navigation.
    pub cmd_history: Vec<String>,
    pub history_cursor: Option<usize>,

    /// Status bar data
    pub session_cmd_count: usize,
    pub boot_instant: std::time::Instant,

    /// Current working directory (best effort tracking)
    pub cwd: String,

    /// Active color theme
    pub theme_name: ThemeName,

    /// Tab completion state (active while cycling through completions)
    pub tab_state: Option<crate::completer::CompletionState>,

    pub input: String,
    pub state: AppState,
    pub last_screen_hash: u64,
}

// ────────────────────────────────────────────────────────────────
// Boot
// ────────────────────────────────────────────────────────────────

pub fn boot() -> (PositronicApp, Task<Message>) {
    let app = PositronicApp {
        engine: None,
        redraw: None,
        direct_output: "⏳ Booting Positronic Engine...\n".to_string(),
        last_snapshot: None,
        cmd_history: Vec::new(),
        history_cursor: None,
        session_cmd_count: 0,
        boot_instant: std::time::Instant::now(),
        cwd: std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string()),
        theme_name: ThemeName::Default,
        tab_state: None,
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