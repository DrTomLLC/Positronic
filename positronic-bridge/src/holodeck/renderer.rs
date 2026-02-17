use glyphon::TextBounds;

use crate::gfx::{QuadInstance, QuadPipeline, TextEngine};
use crate::gfx::text::TextRegion;
use crate::renderer::{ColoredSpan, Rgba, ThemeName};
use super::layout::layout_doc;
use super::protocol::{Action, HolodeckDoc, NodeKind, Rect};

/// Draw Holodeck overlay into the terminal area (safe/automatic gate handled by caller).
pub fn draw_overlay(
    quads: &mut QuadPipeline,
    text: &mut TextEngine,
    theme: ThemeName,
    terminal_bounds: Rect,
    doc: &mut HolodeckDoc,
) -> Option<Rect> {
    if doc.nodes.is_empty() {
        return None;
    }

    let panel = layout_doc(doc, terminal_bounds);

    // Panel background
    quads.push(QuadInstance {
        x: panel.x,
        y: panel.y,
        w: panel.w,
        h: panel.h,
        color: Rgba::new(0.06, 0.07, 0.09, 0.92),
    });

    // Border
    quads.push(QuadInstance {
        x: panel.x,
        y: panel.y,
        w: panel.w,
        h: 1.0,
        color: Rgba::new(0.25, 0.28, 0.32, 1.0),
    });

    for n in &doc.nodes {
        match &n.kind {
            NodeKind::Panel { title } => {
                // title bar background
                quads.push(QuadInstance {
                    x: panel.x,
                    y: panel.y,
                    w: panel.w,
                    h: n.rect.h,
                    color: Rgba::new(0.08, 0.09, 0.11, 0.95),
                });

                push_text(text, n.rect, vec![ColoredSpan::new(title, theme.status_fg())]);
            }
            NodeKind::Button { label, .. } => {
                quads.push(QuadInstance {
                    x: n.rect.x,
                    y: n.rect.y,
                    w: n.rect.w,
                    h: n.rect.h,
                    color: Rgba::new(0.12, 0.13, 0.16, 0.95),
                });
                push_text(
                    text,
                    n.rect,
                    vec![ColoredSpan::new(format!("🖱  {}", label), Rgba::rgb(0.85, 0.85, 0.85))],
                );
            }
            NodeKind::Text { text: body } => {
                push_text(text, n.rect, vec![ColoredSpan::new(body, Rgba::rgb(0.85, 0.85, 0.85))]);
            }
            NodeKind::Json { title, pretty } => {
                push_text(
                    text,
                    n.rect,
                    vec![
                        ColoredSpan::new(format!("{}\n", title), Rgba::rgb(0.6, 0.7, 0.85)),
                        ColoredSpan::new(pretty, Rgba::rgb(0.88, 0.88, 0.88)),
                    ],
                );
            }
            NodeKind::Table { title, preview, .. } => {
                push_text(
                    text,
                    n.rect,
                    vec![
                        ColoredSpan::new(format!("{}\n", title), Rgba::rgb(0.6, 0.85, 0.7)),
                        ColoredSpan::new(preview, Rgba::rgb(0.88, 0.88, 0.88)),
                    ],
                );
            }
            NodeKind::Markdown { title, preview } => {
                push_text(
                    text,
                    n.rect,
                    vec![
                        ColoredSpan::new(format!("{}\n", title), Rgba::rgb(0.85, 0.75, 0.55)),
                        ColoredSpan::new(preview, Rgba::rgb(0.88, 0.88, 0.88)),
                    ],
                );
            }
            NodeKind::Image { title, meta } => {
                // Placeholder image frame (real GPU image quad later)
                quads.push(QuadInstance {
                    x: n.rect.x,
                    y: n.rect.y,
                    w: n.rect.w,
                    h: n.rect.h,
                    color: Rgba::new(0.10, 0.11, 0.14, 0.95),
                });
                let caption = format!(
                    "{}\n{:?} ({}x{})",
                    title,
                    meta.path,
                    meta.width.unwrap_or(0),
                    meta.height.unwrap_or(0)
                );
                push_text(text, n.rect, vec![ColoredSpan::new(caption, Rgba::rgb(0.85, 0.85, 0.85))]);
            }
        }
    }

    Some(panel)
}

/// Hit-test mouse click; returns Action if a button was clicked.
pub fn click(doc: &HolodeckDoc, px: f32, py: f32) -> Option<Action> {
    for n in &doc.nodes {
        if !n.rect.contains(px, py) {
            continue;
        }
        if let NodeKind::Button { action, .. } = &n.kind {
            return Some(action.clone());
        }
    }
    None
}

fn push_text(text: &mut TextEngine, r: Rect, spans: Vec<ColoredSpan>) {
    let bounds = TextBounds {
        left: r.x as i32,
        top: r.y as i32,
        right: (r.x + r.w) as i32,
        bottom: (r.y + r.h) as i32,
    };

    text.push_region(TextRegion {
        spans,
        bounds,
        left: r.x,
        top: r.y,
        scale: 1.0,
        default_color: Rgba::rgb(0.85, 0.85, 0.85),
    });
}
