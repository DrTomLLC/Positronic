//! Terminal output rendering component.
//!
//! Renders the PTY snapshot or direct output text into the terminal area.

use glyphon::TextBounds;

use crate::gfx::{QuadPipeline, TextEngine};
use crate::gfx::text::TextRegion;
use crate::renderer::{self, ColoredSpan, Rgba};
use crate::shell::app::AppState;
use crate::shell::layout::{self, Layout};
use super::scene::SceneData;

pub fn draw(
    quads: &mut QuadPipeline,
    text: &mut TextEngine,
    lay: &Layout,
    data: &SceneData<'_>,
) {
    let padding = layout::TERMINAL_PADDING;

    // ── Gather spans ──
    let spans: Vec<ColoredSpan> = if let Some(snapshot) = data.snapshot {
        renderer::snapshot_to_spans(snapshot, data.theme)
    } else if !data.direct_output.is_empty() {
        renderer::direct_to_spans(data.direct_output)
    } else {
        // Boot/error state
        match data.state {
            AppState::Booting => {
                vec![ColoredSpan::new(
                    "⏳ Booting engine...\n",
                    Rgba::rgb(0.7, 0.7, 0.7),
                )]
            }
            AppState::Error(e) => {
                vec![ColoredSpan::new(
                    format!("❌ {}\n", e),
                    Rgba::rgb(1.0, 0.4, 0.4),
                )]
            }
            AppState::Active => {
                vec![ColoredSpan::new("", Rgba::rgb(0.5, 0.5, 0.5))]
            }
        }
    };

    if spans.is_empty() {
        return;
    }

    // ── Push text region ──
    let bounds = TextBounds {
        left: (lay.terminal_x + padding) as i32,
        top: (lay.terminal_y + padding) as i32,
        right: (lay.terminal_x + lay.terminal_w - padding) as i32,
        bottom: (lay.terminal_y + lay.terminal_h) as i32,
    };

    text.push_region(TextRegion {
        spans,
        bounds,
        left: lay.terminal_x + padding,
        top: lay.terminal_y + padding,
        scale: 1.0,
        default_color: Rgba::rgb(0.85, 0.85, 0.85),
    });
}