//! Scene compositor.

use std::time::Instant;

use crate::gfx::{QuadPipeline, TextEngine};
use crate::renderer::ThemeName;
use crate::shell::app::AppState;
use crate::shell::layout;
use positronic_core::state_machine::Snapshot;

use crate::holodeck::protocol::HolodeckDoc;

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

    // Holodeck overlay (only shown when caller decides it is safe)
    pub holodeck_doc: Option<&'a mut HolodeckDoc>,
    pub holodeck_safe: bool,
}

pub fn compose(
    quads: &mut QuadPipeline,
    text: &mut TextEngine,
    viewport: [u32; 2],
    data: &mut SceneData<'_>,
) {
    let lay = layout::compute(viewport);

    super::status::draw(quads, text, &lay, data);
    super::inputbar::draw(quads, text, &lay, data);
    super::terminal::draw(quads, text, &lay, data);
}
