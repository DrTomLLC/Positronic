//! Positronic Bridge library target.
//!
//! Exposes internal modules for integration tests. The binary entry point
//! is in `main.rs`; this file exists solely so `tests/*.rs` can import
//! the bridge's logic.

// ── Original pillar modules ──────────────────────────────────────
pub mod biolink;
pub mod block;
pub mod hardware;
pub mod holodeck;
pub mod input;

// ── Decomposed pager-fix modules ─────────────────────────────────
pub mod app;
pub mod cwd;
pub mod detection;
pub mod helpers;
pub mod keyboard;
pub mod messages;
pub mod update;
pub mod view_ui;

// ── Unchanged modules (copy from existing project) ───────────────
pub mod completer;
pub mod renderer;
mod platform;
mod gfx;
mod ui;
mod shell;
mod util;