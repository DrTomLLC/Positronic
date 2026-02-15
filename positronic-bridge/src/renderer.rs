//! Terminal rendering utilities.
//!
//! Converts PTY snapshots and text to colored span data for the GPU text renderer.
//! Zero UI dependencies — this module produces data structures that gfx::text consumes.

use positronic_core::state_machine::{MyColor, Snapshot};

// ════════════════════════════════════════════════════════════════════
// Color Types (replaces iced::Color)
// ════════════════════════════════════════════════════════════════════

/// RGBA color in 0.0..1.0 range. Drop-in replacement for iced::Color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Convert to [u8; 4] sRGB for GPU upload.
    pub fn to_srgb8(&self) -> [u8; 4] {
        [
            (self.r.clamp(0.0, 1.0) * 255.0) as u8,
            (self.g.clamp(0.0, 1.0) * 255.0) as u8,
            (self.b.clamp(0.0, 1.0) * 255.0) as u8,
            (self.a.clamp(0.0, 1.0) * 255.0) as u8,
        ]
    }

    /// Convert to glyphon-compatible color.
    pub fn to_glyphon(&self) -> glyphon::Color {
        let [r, g, b, a] = self.to_srgb8();
        glyphon::Color::rgba(r, g, b, a)
    }
}

// ════════════════════════════════════════════════════════════════════
// Colored Span (replaces iced::widget::text::Span)
// ════════════════════════════════════════════════════════════════════

/// A run of text with a single foreground color.
#[derive(Debug, Clone)]
pub struct ColoredSpan {
    pub text: String,
    pub color: Rgba,
}

impl ColoredSpan {
    pub fn new(text: impl Into<String>, color: Rgba) -> Self {
        Self {
            text: text.into(),
            color,
        }
    }
}

// ════════════════════════════════════════════════════════════════════
// Theme Names
// ════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeName {
    Default,
    Monokai,
    Solarized,
    Dracula,
}

impl ThemeName {
    pub fn all() -> &'static [ThemeName] {
        &[
            ThemeName::Default,
            ThemeName::Monokai,
            ThemeName::Solarized,
            ThemeName::Dracula,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            ThemeName::Default => "default",
            ThemeName::Monokai => "monokai",
            ThemeName::Solarized => "solarized",
            ThemeName::Dracula => "dracula",
        }
    }

    pub fn from_str(s: &str) -> Option<ThemeName> {
        match s.to_lowercase().as_str() {
            "default" => Some(ThemeName::Default),
            "monokai" => Some(ThemeName::Monokai),
            "solarized" => Some(ThemeName::Solarized),
            "dracula" => Some(ThemeName::Dracula),
            _ => None,
        }
    }

    /// Background color for the theme.
    pub fn bg_color(&self) -> Rgba {
        match self {
            ThemeName::Default => Rgba::rgb(0.06, 0.065, 0.08),
            ThemeName::Monokai => Rgba::rgb(0.15, 0.16, 0.13),
            ThemeName::Solarized => Rgba::rgb(0.0, 0.17, 0.21),
            ThemeName::Dracula => Rgba::rgb(0.16, 0.16, 0.21),
        }
    }

    /// Status bar background.
    pub fn status_bg(&self) -> Rgba {
        Rgba::new(0.08, 0.09, 0.1, 1.0)
    }

    /// Status bar text color.
    pub fn status_fg(&self) -> Rgba {
        Rgba::rgb(0.5, 0.55, 0.6)
    }

    /// Input bar background.
    pub fn input_bg(&self) -> Rgba {
        Rgba::new(0.1, 0.11, 0.13, 1.0)
    }

    /// Input bar text color.
    pub fn input_fg(&self) -> Rgba {
        Rgba::rgb(0.9, 0.9, 0.9)
    }

    /// Cursor color.
    pub fn cursor_color(&self) -> Rgba {
        Rgba::new(0.9, 0.9, 0.9, 0.8)
    }
}

// ════════════════════════════════════════════════════════════════════
// Direct Output Rendering (plain text with emoji color coding)
// ════════════════════════════════════════════════════════════════════

/// Convert direct output (plain text) to colored spans for display.
pub fn direct_to_spans(text: &str) -> Vec<ColoredSpan> {
    let mut spans = Vec::new();

    for line in text.lines() {
        let color = if line.starts_with("➜") {
            Rgba::rgb(0.3, 0.85, 0.3)
        } else if line.starts_with("❌") {
            Rgba::rgb(1.0, 0.35, 0.35)
        } else if line.starts_with("⚡") || line.starts_with("✓") {
            Rgba::rgb(0.3, 0.8, 1.0)
        } else if line.starts_with("  💡")
            || line.starts_with("📋")
            || line.starts_with("📂")
            || line.starts_with("🎨")
        {
            Rgba::rgb(0.6, 0.6, 0.85)
        } else if line.starts_with("🔍") {
            Rgba::rgb(1.0, 0.85, 0.3)
        } else if line.starts_with("🏆") || line.starts_with("📊") {
            Rgba::rgb(0.3, 0.9, 0.6)
        } else if line.starts_with("📜") || line.starts_with("📝") || line.starts_with("🔖") {
            Rgba::rgb(0.7, 0.7, 0.9)
        } else if line.starts_with("╔") || line.starts_with("║") || line.starts_with("╚") {
            Rgba::rgb(0.4, 0.5, 0.6)
        } else {
            Rgba::rgb(0.85, 0.85, 0.85)
        };

        spans.push(ColoredSpan::new(format!("{}\n", line), color));
    }

    spans
}

// ════════════════════════════════════════════════════════════════════
// PTY Snapshot Rendering
// ════════════════════════════════════════════════════════════════════

/// Convert a PTY snapshot (with ANSI colors) to colored spans.
/// Skips empty leading rows to avoid stale terminal garbage.
pub fn snapshot_to_spans(snapshot: &Snapshot, _theme: ThemeName) -> Vec<ColoredSpan> {
    let mut spans = Vec::new();
    let rows = snapshot.rows();

    if rows == 0 {
        return spans;
    }

    // Find the first non-empty row
    let mut first_content_row = 0;
    for row_idx in 0..rows {
        let row = &snapshot[row_idx];
        let has_content = row.iter().any(|(ch, _)| !ch.is_whitespace());
        if has_content {
            first_content_row = row_idx;
            break;
        }
    }

    for row_idx in first_content_row..rows {
        let row = &snapshot[row_idx];

        if row.is_empty() {
            spans.push(ColoredSpan::new("\n", Rgba::rgb(0.85, 0.85, 0.85)));
            continue;
        }

        let mut current_text = String::new();
        let mut current_color: Option<Rgba> = None;

        for (ch, color_attr) in row.iter() {
            let cell_color = mycolor_to_rgba(color_attr);

            if let Some(prev_color) = current_color {
                if prev_color != cell_color && !current_text.is_empty() {
                    spans.push(ColoredSpan::new(current_text.clone(), prev_color));
                    current_text.clear();
                }
            }

            current_color = Some(cell_color);
            current_text.push(*ch);
        }

        // Flush remaining text
        if !current_text.is_empty() {
            let color = current_color.unwrap_or(Rgba::rgb(0.85, 0.85, 0.85));
            spans.push(ColoredSpan::new(current_text, color));
        }

        spans.push(ColoredSpan::new("\n", Rgba::rgb(0.85, 0.85, 0.85)));
    }

    spans
}

// ════════════════════════════════════════════════════════════════════
// Plain Text (clipboard)
// ════════════════════════════════════════════════════════════════════

/// Convert snapshot to plain text for clipboard copy.
pub fn snapshot_to_plain(snapshot: &Snapshot) -> String {
    let mut out = String::new();
    let rows = snapshot.rows();

    if rows == 0 {
        return out;
    }

    let mut first_content_row = 0;
    for row_idx in 0..rows {
        let row = &snapshot[row_idx];
        if row.iter().any(|(ch, _)| !ch.is_whitespace()) {
            first_content_row = row_idx;
            break;
        }
    }

    for row_idx in first_content_row..rows {
        let row = &snapshot[row_idx];
        let line: String = row.iter().map(|(c, _)| *c).collect();
        out.push_str(line.trim_end());
        out.push('\n');
    }

    out
}

// ════════════════════════════════════════════════════════════════════
// Color Conversion
// ════════════════════════════════════════════════════════════════════

/// Convert MyColor from state machine to Rgba.
fn mycolor_to_rgba(my_color: &MyColor) -> Rgba {
    match my_color {
        MyColor::Default => Rgba::rgb(0.85, 0.85, 0.85),
        MyColor::Black => Rgba::rgb(0.1, 0.1, 0.1),
        MyColor::Red => Rgba::rgb(0.9, 0.3, 0.3),
        MyColor::Green => Rgba::rgb(0.3, 0.85, 0.3),
        MyColor::Yellow => Rgba::rgb(0.95, 0.85, 0.3),
        MyColor::Blue => Rgba::rgb(0.4, 0.6, 1.0),
        MyColor::Magenta => Rgba::rgb(0.9, 0.4, 0.9),
        MyColor::Cyan => Rgba::rgb(0.3, 0.85, 0.85),
        MyColor::White => Rgba::rgb(0.95, 0.95, 0.95),
        MyColor::BrightBlack => Rgba::rgb(0.5, 0.5, 0.5),
        MyColor::BrightRed => Rgba::rgb(1.0, 0.5, 0.5),
        MyColor::BrightGreen => Rgba::rgb(0.5, 1.0, 0.5),
        MyColor::BrightYellow => Rgba::rgb(1.0, 1.0, 0.5),
        MyColor::BrightBlue => Rgba::rgb(0.5, 0.7, 1.0),
        MyColor::BrightMagenta => Rgba::rgb(1.0, 0.5, 1.0),
        MyColor::BrightCyan => Rgba::rgb(0.5, 1.0, 1.0),
        MyColor::BrightWhite => Rgba::rgb(1.0, 1.0, 1.0),
        MyColor::Indexed(idx) => indexed_color(*idx),
        MyColor::Rgb(r, g, b) => {
            Rgba::rgb(*r as f32 / 255.0, *g as f32 / 255.0, *b as f32 / 255.0)
        }
    }
}

/// Convert indexed color (0-255) to Rgba using standard xterm-256 palette.
fn indexed_color(idx: u8) -> Rgba {
    match idx {
        0 => Rgba::rgb(0.0, 0.0, 0.0),
        1 => Rgba::rgb(0.8, 0.0, 0.0),
        2 => Rgba::rgb(0.0, 0.8, 0.0),
        3 => Rgba::rgb(0.8, 0.8, 0.0),
        4 => Rgba::rgb(0.0, 0.0, 0.8),
        5 => Rgba::rgb(0.8, 0.0, 0.8),
        6 => Rgba::rgb(0.0, 0.8, 0.8),
        7 => Rgba::rgb(0.75, 0.75, 0.75),
        8 => Rgba::rgb(0.5, 0.5, 0.5),
        9 => Rgba::rgb(1.0, 0.0, 0.0),
        10 => Rgba::rgb(0.0, 1.0, 0.0),
        11 => Rgba::rgb(1.0, 1.0, 0.0),
        12 => Rgba::rgb(0.0, 0.0, 1.0),
        13 => Rgba::rgb(1.0, 0.0, 1.0),
        14 => Rgba::rgb(0.0, 1.0, 1.0),
        15 => Rgba::rgb(1.0, 1.0, 1.0),
        16..=231 => {
            let i = idx - 16;
            let r = ((i / 36) % 6) as f32 / 5.0;
            let g = ((i / 6) % 6) as f32 / 5.0;
            let b = (i % 6) as f32 / 5.0;
            Rgba::rgb(r, g, b)
        }
        232..=255 => {
            let gray = ((idx - 232) as f32 * 10.0 + 8.0) / 255.0;
            Rgba::rgb(gray, gray, gray)
        }
    }
}