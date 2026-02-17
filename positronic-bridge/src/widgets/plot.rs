//! Minimal plot widget.
//!
//! No texture pipeline required: renders a sparkline-style chart as text.
//! This is intentionally lightweight and cross-platform.

use glyphon::TextBounds;

use crate::gfx::{QuadInstance, QuadPipeline, TextEngine};
use crate::gfx::text::TextRegion;
use crate::renderer::{ColoredSpan, Rgba};

use super::{Rect, WidgetId};

#[derive(Debug, Clone)]
pub struct PlotWidget {
    pub id: WidgetId,
    pub rect: Rect,
    pub title: String,
    pub series: Vec<f64>,
}

impl PlotWidget {
    pub fn new(rect: Rect, title: impl Into<String>, series: Vec<f64>) -> Self {
        Self {
            id: WidgetId::new(),
            rect,
            title: title.into(),
            series,
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

        let spark = sparkline(&self.series);

        let spans = vec![
            ColoredSpan::new(format!("📈 {}\n", self.title), Rgba::rgb(0.85, 0.85, 0.95)),
            ColoredSpan::new(format!("{}\n", spark), Rgba::rgb(0.35, 0.9, 0.6)),
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

fn sparkline(values: &[f64]) -> String {
    if values.is_empty() {
        return "(no data)".to_string();
    }
    let blocks = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let min = values
        .iter()
        .cloned()
        .fold(f64::INFINITY, f64::min);
    let max = values
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);

    let span = (max - min).max(1e-9);
    let mut out = String::with_capacity(values.len());
    for &v in values {
        let t = ((v - min) / span).clamp(0.0, 1.0);
        let idx = (t * (blocks.len() as f64 - 1.0)).round() as usize;
        out.push(blocks[idx]);
    }
    out
}
