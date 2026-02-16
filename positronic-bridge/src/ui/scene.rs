// positronic-bridge/src/ui/scene.rs
//! Scene compositor.
//!
//! Composes the terminal output, status bar, and input bar into
//! quad + text draw calls. Replaces the old iced view_ui.rs.

use std::time::Instant;

use crate::gfx::{QuadPipeline, TextEngine};
use crate::renderer::ThemeName;
use crate::shell::app::AppState;
use crate::shell::layout;
use positronic_core::state_machine::Snapshot;

/// All data needed to compose a frame. Passed by reference to avoid cloning.
pub struct SceneData<'a> {
    pub state: &'a AppState,
    pub snapshot: Option<&'a Snapshot>,
    pub direct_output: &'a str,
    pub input: &'a str,
    pub cursor_pos: usize,
    pub theme: ThemeName,
    pub session_cmd_count: usize,
    pub boot_instant: Instant,
    pub cwd: &'a str,
}

/// Compose the full UI into quad + text draw commands.
pub fn compose(
    quads: &mut QuadPipeline,
    text: &mut TextEngine,
    viewport: [u32; 2],
    data: &SceneData<'_>,
) {
    let lay = layout::compute(viewport);

    // Draw status bar background
    super::status::draw(quads, text, &lay, data);

    // Draw input bar background
    super::inputbar::draw(quads, text, &lay, data);

    // Draw terminal output
    super::terminal::draw(quads, text, &lay, data);
}