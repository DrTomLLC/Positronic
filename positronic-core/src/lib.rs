pub mod airlock;
pub mod builtins;
pub mod engine;
pub mod pty_manager;
pub mod runner;
pub mod runtime;
pub mod state_machine;
pub mod term;
pub mod vault;
pub mod watcher;

// Re-export the main struct so users can just use `positronic_core::PositronicEngine`
pub use engine::PositronicEngine;

// Re-export the simpler types for the UI
pub use state_machine::MyColor;

use serde::{Deserialize, Serialize};

/// A single "Block" of terminal output. (Legacy type; still OK to keep.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalBlock {
    pub id: usize,
    pub command: String,
    pub output: String,
    pub exit_code: Option<i32>,
    pub timestamp: i64,
}

/// The event stream from the PTY. The Bridge listens to this to know when to redraw.
#[derive(Debug, Clone)]
pub enum PtyEvent {
    Output(Vec<u8>),              // Raw bytes from shell
    BlockFinished(TerminalBlock), // A command finished
    Bell,                         // Ding!
}

/// The command stream to the PTY. The Bridge sends this to the Core.
#[derive(Debug, Clone)]
pub enum PtyCommand {
    Input(String),         // User typed something
    Resize(u16, u16),      // Window resized
    Execute(String),       // Run this command (creates a block)
}
