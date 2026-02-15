//! Input bar rendering component.
//!
//! Renders the command input field with cursor indicator.

use glyphon::TextBounds;

use crate::gfx::{QuadInstance, QuadPipeline, TextEngine};
use crate::gfx::text::TextRegion;
use crate::renderer::{ColoredSpan, Rgba};
use crate::shell::layout::Layout;
use super::scene::SceneData;

/// Approximate monospace character width at font size 14.
const CHAR_WIDTH: f32 = 8.4;

pub fn draw(
    quads: &mut QuadPipeline,
    text: &mut TextEngine,
    lay: &Layout,
    data: &SceneData<'_>,
) {
    let theme = data.theme;

    // Background
    quads.push(QuadInstance {
        x: lay.input_x,
        y: lay.input_y,
        w: lay.input_w,
        h: lay.input_h,
        color: theme.input_bg(),
    });

    // Top border
    quads.push(QuadInstance {
        x: lay.input_x,
        y: lay.input_y,
        w: lay.input_w,
        h: 1.0,
        color: Rgba::rgb(0.2, 0.22, 0.25),
    });

    // Prompt prefix
    let prompt = "❯ ";
    let prompt_width = prompt.chars().count() as f32 * CHAR_WIDTH;
    let text_left = lay.input_x + 10.0;
    let text_top = lay.input_y + 9.0;

    // Build display text
    let display = if data.input.is_empty() {
        vec![
            ColoredSpan::new(prompt, Rgba::rgb(0.3, 0.85, 0.3)),
            ColoredSpan::new(
                "Type a command… (!help for commands)",
                Rgba::rgb(0.4, 0.4, 0.45),
            ),
        ]
    } else {
        vec![
            ColoredSpan::new(prompt, Rgba::rgb(0.3, 0.85, 0.3)),
            ColoredSpan::new(data.input, theme.input_fg()),
        ]
    };

    let bounds = TextBounds {
        left: text_left as i32,
        top: text_top as i32,
        right: (lay.input_x + lay.input_w - 10.0) as i32,
        bottom: (lay.input_y + lay.input_h) as i32,
    };

    text.push_region(TextRegion {
        spans: display,
        bounds,
        left: text_left,
        top: text_top,
        scale: 1.0,
        default_color: theme.input_fg(),
    });

    // ── Cursor ──
    if !data.input.is_empty() || true {
        // Show cursor even on empty input
        let cursor_x = text_left + prompt_width + (data.cursor_pos as f32 * CHAR_WIDTH);
        let cursor_y = text_top;
        let cursor_h = 16.0;

        quads.push(QuadInstance {
            x: cursor_x,
            y: cursor_y,
            w: 2.0,
            h: cursor_h,
            color: theme.cursor_color(),
        });
    }
}