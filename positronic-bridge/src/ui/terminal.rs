//! Terminal output rendering component.

use glyphon::TextBounds;

use crate::gfx::{QuadPipeline, TextEngine};
use crate::gfx::text::TextRegion;
use crate::renderer::{self, ColoredSpan, Rgba};
use crate::shell::app::AppState;
use crate::shell::layout::{self, Layout};
use super::scene::SceneData;

use crate::holodeck::protocol::Rect as HRect;

pub fn draw(
    quads: &mut QuadPipeline,
    text: &mut TextEngine,
    lay: &Layout,
    data: &mut SceneData<'_>,
) {
    let padding = layout::TERMINAL_PADDING;

    let spans: Vec<ColoredSpan> = if let Some(snapshot) = data.snapshot {
        renderer::snapshot_to_spans(snapshot, data.theme)
    } else if !data.direct_output.is_empty() {
        renderer::direct_to_spans(data.direct_output)
    } else {
        match data.state {
            AppState::Booting => vec![ColoredSpan::new("⏳ Booting engine...\n", Rgba::rgb(0.7, 0.7, 0.7))],
            AppState::Error(e) => vec![ColoredSpan::new(format!("❌ {}\n", e), Rgba::rgb(1.0, 0.4, 0.4))],
            AppState::Active => vec![ColoredSpan::new("", Rgba::rgb(0.5, 0.5, 0.5))],
        }
    };

    if !spans.is_empty() {
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

    // Holodeck overlay (safe-gated)
    if data.holodeck_safe {
        if let Some(doc) = data.holodeck_doc.as_deref_mut() {
            let term_bounds = HRect {
                x: lay.terminal_x + padding,
                y: lay.terminal_y + padding,
                w: lay.terminal_w - (padding * 2.0),
                h: lay.terminal_h - (padding * 2.0),
            };
            let _ = crate::holodeck::renderer::draw_overlay(
                quads,
                text,
                data.theme,
                term_bounds,
                doc,
            );
        }
    }
}
