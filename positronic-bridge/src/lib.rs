//! Positronic Bridge library target.
//!
//! The GPU frontend for the Positronic terminal. Built on winit + wgpu + glyphon.
//! Zero iced dependencies. Zero Elm architecture. Direct GPU rendering.
//!
//! Module layout:
//!   gfx/     — wgpu rendering pipeline (device, quad, text, shaders)
//!   shell/   — winit application lifecycle, event dispatch, layout
//!   ui/      — composable UI components (terminal, status bar, input bar)
//!
//!   block    — TerminalBlock model (UI-side)
//!   biolink  — Biometric link surface (Pillar XII)
//!   hardware — Hardware panel (IoT device status)
//!   holodeck — Rich media content detection & parsing
//!   input    — Intelli-Input editor (pure Rust, no UI deps)
//!   completer — Tab completion engine
//!   cwd      — Working directory tracker
//!   detection — Terminal mode detection (pager, continuation, etc.)
//!   helpers  — Shared utility functions
//!   renderer — Color conversion & snapshot-to-text (no UI deps)
//!   platform — Platform-specific hooks

// ── The New Architecture ─────────────────────────────────────────
pub mod gfx;
pub mod shell;
pub mod ui;

// ── Pure-Rust Pillar Modules (zero UI dependencies) ──────────────
pub mod biolink;
pub mod block;
pub mod hardware;
pub mod holodeck;
pub mod input;

// ── Shared Logic ─────────────────────────────────────────────────
pub mod completer;
pub mod cwd;
pub mod detection;
pub mod helpers;
pub mod renderer;
pub mod util;

mod platform;