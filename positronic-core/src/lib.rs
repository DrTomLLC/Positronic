use serde::{Deserialize, Serialize};

pub mod pty_manager;

/// A single "Block" of terminal output.
/// This is the atomic unit of the Positronic interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalBlock {
    pub id: usize,
    pub command: String,
    pub output: String,
    pub exit_code: Option<i32>,
    pub timestamp: i64,
}

/// The event stream from the PTY.
/// The Bridge listens to this to know when to redraw.
#[derive(Debug, Clone)]
pub enum PtyEvent {
    Output(Vec<u8>),       // Raw bytes from shell
    BlockFinished(TerminalBlock), // A command finished
    Bell,                  // Ding!
}

/// The command stream to the PTY.
/// The Bridge sends this to the Core.
#[derive(Debug, Clone)]
pub enum PtyCommand {
    Input(String),         // User typed something
    Resize(u16, u16),      // Window resized
    Execute(String),       // Run this command (creates a block)
}
