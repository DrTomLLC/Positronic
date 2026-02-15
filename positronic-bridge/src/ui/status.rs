//! Status bar rendering component.
//!
//! Shows: command count, uptime, CWD, theme name, version.

use glyphon::TextBounds;

use crate::gfx::{QuadInstance, QuadPipeline, TextEngine};
use crate::gfx::text::TextRegion;
use crate::helpers::{format_duration_short, short_path};
use crate::renderer::{ColoredSpan, Rgba};
use crate::shell::layout::Layout;
use super::scene::SceneData;

pub fn draw(
    quads: &mut QuadPipeline,
    text: &mut TextEngine,
    lay: &Layout,
    data: &SceneData<'_>,
) {
    let theme = data.theme;

    // Background
    quads.push(QuadInstance {
        x: lay.status_x,
        y: lay.status_y,
        w: lay.status_w,
        h: lay.status_h,
        color: theme.status_bg(),
    });

    // Top border
    quads.push(QuadInstance {
        x: lay.status_x,
        y: lay.status_y,
        w: lay.status_w,
        h: 1.0,
        color: Rgba::rgb(0.2, 0.22, 0.25),
    });

    // Status text
    let uptime_secs = data.boot_instant.elapsed().as_secs() as i64;
    let uptime_str = format_duration_short(uptime_secs);
    let short_cwd = short_path(data.cwd);

    let status_text = format!(
        " ⚡ {} cmd  │  ⏱ {}  │  📂 {}  │  🎨 {}  │  Positronic v0.3.0",
        data.session_cmd_count, uptime_str, short_cwd, data.theme.label(),
    );

    let bounds = TextBounds {
        left: lay.status_x as i32 + 8,
        top: lay.status_y as i32 + 3,
        right: (lay.status_x + lay.status_w) as i32 - 8,
        bottom: (lay.status_y + lay.status_h) as i32,
    };

    text.push_region(TextRegion {
        spans: vec![ColoredSpan::new(status_text, theme.status_fg())],
        bounds,
        left: lay.status_x + 8.0,
        top: lay.status_y + 4.0,
        scale: 0.85,
        default_color: theme.status_fg(),
    });
}