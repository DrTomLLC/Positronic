//! Message enum and output helpers.
//!
//! New message variants added for the pager-trap bugfix:
//! - `SendInterrupt` — Ctrl+C sends 0x03 to PTY
//! - `SendEscape` — Escape key sends 0x1b to PTY
//! - `SendEof` — Ctrl+D sends 0x04 to PTY

use crate::app::{PositronicApp, RedrawHandle, MAX_DIRECT_BYTES};
use crate::renderer::ThemeName;

use positronic_core::engine::ExecuteResult;
use positronic_core::PositronicEngine;

use std::sync::Arc;

// ────────────────────────────────────────────────────────────────
// Message enum
// ────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum Message {
    EngineReady(Arc<PositronicEngine>, RedrawHandle),
    EngineFailed(String),
    Redraw,
    InputChanged(String),
    InputSent,
    CommandResult(ExecuteResult),
    CommandError(String),
    HistoryUp,
    HistoryDown,
    ClearScreen,
    CopyToClipboard,
    TabComplete,
    WindowResized(u32, u32),
    Tick,
    ThemeChanged(ThemeName),

    // ── Pager-trap bugfix: new control signal messages ──
    SendInterrupt,
    SendEscape,
    SendEof,
}

// ────────────────────────────────────────────────────────────────
// Output management
// ────────────────────────────────────────────────────────────────

/// Append text to the direct output buffer, with automatic trimming.
pub fn push_direct(app: &mut PositronicApp, new_text: &str) {
    app.direct_output.push_str(new_text);
    if !new_text.ends_with('\n') {
        app.direct_output.push('\n');
    }

    if app.direct_output.len() > MAX_DIRECT_BYTES {
        let mid = app.direct_output.len() / 2;
        if let Some(nl) = app.direct_output[mid..].find('\n') {
            let trim_at = mid + nl + 1;
            let kept = app.direct_output[trim_at..].to_string();
            app.direct_output = format!("··· (older output trimmed) ···\n{}", kept);
        }
    }
}