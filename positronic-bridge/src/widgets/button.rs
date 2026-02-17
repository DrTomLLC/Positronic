//! Simple clickable button widget.

use glyphon::TextBounds;

use crate::gfx::{QuadInstance, QuadPipeline, TextEngine};
use crate::gfx::text::TextRegion;
use crate::renderer::{ColoredSpan, Rgba};

use super::{Rect, WidgetAction, WidgetId};

#[derive(Debug, Clone)]
pub struct Button {
    pub id: WidgetId,
    pub rect: Rect,
    pub label: String,
    pub action: WidgetAction,
    pub hovered: bool,
    pub pressed: bool,
}

impl Button {
    pub fn new(rect: Rect, label: impl Into<String>, action: WidgetAction) -> Self {
        Self {
            id: WidgetId::new(),
            rect,
            label: label.into(),
            action,
            hovered: false,
            pressed: false,
        }
    }

    pub fn render(&self, quads: &mut QuadPipeline, text: &mut TextEngine) {
        let bg = if self.pressed {
            Rgba::rgb(0.25, 0.28, 0.35)
        } else if self.hovered {
            Rgba::rgb(0.18, 0.2, 0.26)
        } else {
            Rgba::rgb(0.13, 0.14, 0.18)
        };

        quads.push(QuadInstance {
            x: self.rect.x,
            y: self.rect.y,
            w: self.rect.w,
            h: self.rect.h,
            color: bg,
        });

        let bounds = TextBounds {
            left: (self.rect.x + 8.0) as i32,
            top: (self.rect.y + 6.0) as i32,
            right: (self.rect.x + self.rect.w - 8.0) as i32,
            bottom: (self.rect.y + self.rect.h - 6.0) as i32,
        };

        text.push_region(TextRegion {
            spans: vec![ColoredSpan::new(self.label.clone(), Rgba::rgb(0.9, 0.9, 0.9))],
            bounds,
            left: self.rect.x + 8.0,
            top: self.rect.y + 6.0,
            scale: 1.0,
            default_color: Rgba::rgb(0.9, 0.9, 0.9),
        });
    }

    pub fn hit(&self, x: f32, y: f32) -> bool {
        self.rect.contains(x, y)
    }
}
