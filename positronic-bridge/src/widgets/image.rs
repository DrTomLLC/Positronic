//! Placeholder image widget.
//!
//! You already have `image`, `svg`, `resvg` deps — but your current GPU stack
//! does not yet include a textured-quad pipeline.
//!
//! This widget renders image metadata and reserves the rect. When you add a
//! texture pipeline, this file becomes the single place to upgrade.

use glyphon::TextBounds;

use crate::gfx::{QuadInstance, QuadPipeline, TextEngine};
use crate::gfx::text::TextRegion;
use crate::renderer::{ColoredSpan, Rgba};
use crate::holodeck::ImageMeta;

use super::{Rect, WidgetId};

#[derive(Debug, Clone)]
pub struct ImageWidget {
    pub id: WidgetId,
    pub rect: Rect,
    pub meta: ImageMeta,
}

impl ImageWidget {
    pub fn new(rect: Rect, meta: ImageMeta) -> Self {
        Self {
            id: WidgetId::new(),
            rect,
            meta,
        }
    }

    pub fn render(&self, quads: &mut QuadPipeline, text: &mut TextEngine) {
        quads.push(QuadInstance {
            x: self.rect.x,
            y: self.rect.y,
            w: self.rect.w,
            h: self.rect.h,
            color: Rgba::rgb(0.08, 0.085, 0.11),
        });

        let spans = vec![
            ColoredSpan::new("🖼 Image (texture pipeline pending)\n", Rgba::rgb(0.85, 0.85, 0.95)),
            ColoredSpan::new(
                format!(
                    "protocol={} w={:?} h={:?} bytes={}..{}\n",
                    self.meta.protocol,
                    self.meta.width,
                    self.meta.height,
                    self.meta.data_offset,
                    self.meta.data_offset + self.meta.data_len
                ),
                Rgba::rgb(0.75, 0.75, 0.8),
            ),
        ];

        let bounds = TextBounds {
            left: (self.rect.x + 10.0) as i32,
            top: (self.rect.y + 8.0) as i32,
            right: (self.rect.x + self.rect.w - 10.0) as i32,
            bottom: (self.rect.y + self.rect.h - 8.0) as i32,
        };

        text.push_region(TextRegion {
            spans,
            bounds,
            left: self.rect.x + 10.0,
            top: self.rect.y + 8.0,
            scale: 1.0,
            default_color: Rgba::rgb(0.85, 0.85, 0.85),
        });
    }
}
