//! Table widget for Holodeck.
//!
//! Renders a DataFrame-like view using text (fast, portable, no textures required).
//! Supports scrolling + row selection via pointer events.

use glyphon::TextBounds;

use crate::gfx::{QuadInstance, QuadPipeline, TextEngine};
use crate::gfx::text::TextRegion;
use crate::renderer::{ColoredSpan, Rgba};
use crate::holodeck::DataFrame;

use super::{PointerEvent, PointerKind, Rect, WidgetAction, WidgetId};

#[derive(Debug, Clone)]
pub struct TableWidget {
    pub id: WidgetId,
    pub rect: Rect,
    pub df: DataFrame,
    pub scroll_row: usize,
    pub selected_row: Option<usize>,
    pub hovered: bool,
}

impl TableWidget {
    pub fn new(rect: Rect, df: DataFrame) -> Self {
        Self {
            id: WidgetId::new(),
            rect,
            df,
            scroll_row: 0,
            selected_row: None,
            hovered: false,
        }
    }

    pub fn render(&self, quads: &mut QuadPipeline, text: &mut TextEngine) {
        // Background
        quads.push(QuadInstance {
            x: self.rect.x,
            y: self.rect.y,
            w: self.rect.w,
            h: self.rect.h,
            color: Rgba::rgb(0.08, 0.085, 0.11),
        });

        // Header background strip
        quads.push(QuadInstance {
            x: self.rect.x,
            y: self.rect.y,
            w: self.rect.w,
            h: 28.0,
            color: Rgba::rgb(0.11, 0.12, 0.16),
        });

        let visible_rows = ((self.rect.h - 34.0) / 18.0).max(1.0) as usize;
        let end = (self.scroll_row + visible_rows).min(self.df.rows.len());

        let mut spans: Vec<ColoredSpan> = Vec::new();

        // Header line
        spans.push(ColoredSpan::new(
            format!("{}\n", self.df.headers.join(" │ ")),
            Rgba::rgb(0.85, 0.85, 0.95),
        ));

        // Separator
        spans.push(ColoredSpan::new(
            "────────────────────────────────────────\n".to_string(),
            Rgba::rgb(0.25, 0.28, 0.35),
        ));

        for row_idx in self.scroll_row..end {
            let row = &self.df.rows[row_idx];
            let line = row
                .iter()
                .map(|c| format!("{:?}", c).trim_matches('"').to_string())
                .collect::<Vec<_>>()
                .join(" │ ");

            let color = if self.selected_row == Some(row_idx) {
                Rgba::rgb(0.35, 0.85, 0.55)
            } else {
                Rgba::rgb(0.85, 0.85, 0.85)
            };

            spans.push(ColoredSpan::new(format!("{}\n", line), color));
        }

        let bounds = TextBounds {
            left: (self.rect.x + 10.0) as i32,
            top: (self.rect.y + 6.0) as i32,
            right: (self.rect.x + self.rect.w - 10.0) as i32,
            bottom: (self.rect.y + self.rect.h - 6.0) as i32,
        };

        text.push_region(TextRegion {
            spans,
            bounds,
            left: self.rect.x + 10.0,
            top: self.rect.y + 6.0,
            scale: 1.0,
            default_color: Rgba::rgb(0.85, 0.85, 0.85),
        });
    }

    pub fn on_pointer(&mut self, ev: PointerEvent) -> WidgetAction {
        if !self.rect.contains(ev.x, ev.y) {
            self.hovered = false;
            return WidgetAction::None;
        }
        self.hovered = true;

        match ev.kind {
            PointerKind::Wheel { delta_y } => {
                if delta_y < 0.0 {
                    self.scroll_row = (self.scroll_row + 1).min(self.df.rows.len().saturating_sub(1));
                } else {
                    self.scroll_row = self.scroll_row.saturating_sub(1);
                }
                WidgetAction::None
            }
            PointerKind::Down => {
                // Convert click y into row index
                let local_y = ev.y - (self.rect.y + 28.0 + 18.0); // header + separator
                if local_y >= 0.0 {
                    let clicked_row = self.scroll_row + (local_y / 18.0) as usize;
                    if clicked_row < self.df.rows.len() {
                        self.selected_row = Some(clicked_row);
                        eprintln!("[TABLE] selected_row={}", clicked_row);

                        // Provide a convenient "copy row" behavior.
                        let row = &self.df.rows[clicked_row];
                        let text = row
                            .iter()
                            .map(|c| format!("{:?}", c).trim_matches('"').to_string())
                            .collect::<Vec<_>>()
                            .join("\t");
                        return WidgetAction::CopyText(text);
                    }
                }
                WidgetAction::None
            }
            _ => WidgetAction::None,
        }
    }
}
