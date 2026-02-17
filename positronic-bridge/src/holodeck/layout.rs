use super::protocol::{HolodeckDoc, Rect};

/// Compute rects for doc nodes inside an overlay panel placed in the terminal area.
///
/// Strategy:
/// - Panel anchored top-right of terminal bounds
/// - Vertical stacking of nodes
pub fn layout_doc(doc: &mut HolodeckDoc, terminal_bounds: Rect) -> Rect {
    let margin = 10.0;
    let panel_w = (terminal_bounds.w * 0.46).clamp(360.0, 560.0);
    let panel_h = (terminal_bounds.h * 0.72).clamp(220.0, 520.0);

    let panel = Rect {
        x: terminal_bounds.x + terminal_bounds.w - panel_w - margin,
        y: terminal_bounds.y + margin,
        w: panel_w,
        h: panel_h,
    };

    // node 0 is always Panel
    let mut y = panel.y;
    let title_h = 34.0;
    let gap = 8.0;

    for (i, n) in doc.nodes.iter_mut().enumerate() {
        if i == 0 {
            n.rect = Rect { x: panel.x, y: panel.y, w: panel.w, h: title_h };
            y = panel.y + title_h + gap;
            continue;
        }

        let h = match &n.kind {
            super::protocol::NodeKind::Button { .. } => 28.0,
            super::protocol::NodeKind::Text { .. } => panel.h - (y - panel.y) - gap,
            super::protocol::NodeKind::Json { .. } => panel.h - (y - panel.y) - gap,
            super::protocol::NodeKind::Table { .. } => panel.h - (y - panel.y) - gap,
            super::protocol::NodeKind::Markdown { .. } => panel.h - (y - panel.y) - gap,
            super::protocol::NodeKind::Image { .. } => 140.0,
            super::protocol::NodeKind::Panel { .. } => title_h,
        }
            .max(24.0);

        n.rect = Rect { x: panel.x + 10.0, y, w: panel.w - 20.0, h };
        y += h + gap;

        if y > panel.y + panel.h {
            break;
        }
    }

    panel
}
