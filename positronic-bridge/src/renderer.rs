//! Terminal rendering utilities for converting PTY snapshots and text to colored spans.

use iced::widget::text::Span;
use iced::Color;
use positronic_core::state_machine::{Snapshot, MyColor};

// ════════════════════════════════════════════════════════════════════
// Theme Names (for future theming support)
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
}

// ════════════════════════════════════════════════════════════════════
// Direct Output Rendering (plain text with simple color coding)
// ════════════════════════════════════════════════════════════════════

/// Convert direct output (plain text) to colored spans for display.
/// Colors lines based on their prefix emoji/symbol.
pub fn direct_to_spans(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    for line in text.lines() {
        // Color-code based on line prefix
        let color = if line.starts_with("➜") {
            Color::from_rgb(0.3, 0.85, 0.3) // Green for command echo
        } else if line.starts_with("❌") {
            Color::from_rgb(1.0, 0.35, 0.35) // Red for errors
        } else if line.starts_with("⚡") || line.starts_with("✓") {
            Color::from_rgb(0.3, 0.8, 1.0) // Cyan for success
        } else if line.starts_with("  💡") || line.starts_with("📋") || line.starts_with("📂") || line.starts_with("🎨") {
            Color::from_rgb(0.6, 0.6, 0.85) // Light purple for info
        } else if line.starts_with("🔍") {
            Color::from_rgb(1.0, 0.85, 0.3) // Yellow for search
        } else if line.starts_with("🏆") || line.starts_with("📊") {
            Color::from_rgb(0.3, 0.9, 0.6) // Light green for stats
        } else if line.starts_with("📜") || line.starts_with("📝") || line.starts_with("🔖") {
            Color::from_rgb(0.7, 0.7, 0.9) // Light blue for lists
        } else if line.starts_with("╔") || line.starts_with("║") || line.starts_with("╚") {
            Color::from_rgb(0.4, 0.5, 0.6) // Gray for box drawing
        } else if line.trim().is_empty() {
            Color::from_rgb(0.85, 0.85, 0.85) // Default for empty lines
        } else {
            Color::from_rgb(0.85, 0.85, 0.85) // Light gray default
        };

        spans.push(Span::new(format!("{}\n", line)).color(color));
    }

    spans
}

// ════════════════════════════════════════════════════════════════════
// PTY Snapshot Rendering (colored terminal output with ANSI codes)
// ════════════════════════════════════════════════════════════════════

/// Convert a PTY snapshot (with ANSI colors) to colored spans for iced display.
/// Preserves all color attributes from the terminal emulator.
/// Skips empty leading rows to avoid showing stale terminal garbage.
pub fn snapshot_to_spans(snapshot: &Snapshot, _theme: ThemeName) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let rows = snapshot.rows();

    if rows == 0 {
        return spans;
    }

    // Find the first non-empty row (skip stale garbage at the top)
    let mut first_content_row = 0;
    for row_idx in 0..rows {
        let row = &snapshot[row_idx];
        let has_content = row.iter().any(|(ch, _)| !ch.is_whitespace());
        if has_content {
            first_content_row = row_idx;
            break;
        }
    }

    // Render from first content row to end
    for row_idx in first_content_row..rows {
        let row = &snapshot[row_idx];

        if row.is_empty() {
            // Empty row - just add newline
            spans.push(Span::new("\n").color(Color::from_rgb(0.85, 0.85, 0.85)));
            continue;
        }

        let mut current_text = String::new();
        let mut current_color: Option<Color> = None;

        for (ch, color_attr) in row.iter() {
            // Convert the MyColor to iced Color
            let cell_color = mycolor_to_iced(color_attr);

            // If color changed, flush current span and start new one
            if let Some(prev_color) = current_color {
                if prev_color != cell_color {
                    if !current_text.is_empty() {
                        spans.push(Span::new(current_text.clone()).color(prev_color));
                        current_text.clear();
                    }
                }
            }

            current_color = Some(cell_color);
            current_text.push(*ch);
        }

        // Flush remaining text in this row
        if !current_text.is_empty() {
            let color = current_color.unwrap_or(Color::from_rgb(0.85, 0.85, 0.85));
            spans.push(Span::new(current_text).color(color));
        }

        // Add newline at end of row
        spans.push(Span::new("\n").color(Color::from_rgb(0.85, 0.85, 0.85)));
    }

    spans
}

// ════════════════════════════════════════════════════════════════════
// Color Conversion
// ════════════════════════════════════════════════════════════════════

/// Convert MyColor from state machine to iced Color
fn mycolor_to_iced(my_color: &MyColor) -> Color {
    match my_color {
        MyColor::Default => Color::from_rgb(0.85, 0.85, 0.85),
        MyColor::Black => Color::from_rgb(0.1, 0.1, 0.1),
        MyColor::Red => Color::from_rgb(0.9, 0.3, 0.3),
        MyColor::Green => Color::from_rgb(0.3, 0.85, 0.3),
        MyColor::Yellow => Color::from_rgb(0.95, 0.85, 0.3),
        MyColor::Blue => Color::from_rgb(0.4, 0.6, 1.0),
        MyColor::Magenta => Color::from_rgb(0.9, 0.4, 0.9),
        MyColor::Cyan => Color::from_rgb(0.3, 0.85, 0.85),
        MyColor::White => Color::from_rgb(0.95, 0.95, 0.95),
        MyColor::BrightBlack => Color::from_rgb(0.5, 0.5, 0.5),
        MyColor::BrightRed => Color::from_rgb(1.0, 0.5, 0.5),
        MyColor::BrightGreen => Color::from_rgb(0.5, 1.0, 0.5),
        MyColor::BrightYellow => Color::from_rgb(1.0, 1.0, 0.5),
        MyColor::BrightBlue => Color::from_rgb(0.5, 0.7, 1.0),
        MyColor::BrightMagenta => Color::from_rgb(1.0, 0.5, 1.0),
        MyColor::BrightCyan => Color::from_rgb(0.5, 1.0, 1.0),
        MyColor::BrightWhite => Color::from_rgb(1.0, 1.0, 1.0),
        MyColor::Indexed(idx) => indexed_color(*idx),
        MyColor::Rgb(r, g, b) => Color::from_rgb(*r as f32 / 255.0, *g as f32 / 255.0, *b as f32 / 255.0),
    }
}

/// Convert indexed color (0-255) to RGB using standard xterm-256 color palette
fn indexed_color(idx: u8) -> Color {
    match idx {
        // 0-7: Standard colors
        0 => Color::from_rgb(0.0, 0.0, 0.0),       // Black
        1 => Color::from_rgb(0.8, 0.0, 0.0),       // Red
        2 => Color::from_rgb(0.0, 0.8, 0.0),       // Green
        3 => Color::from_rgb(0.8, 0.8, 0.0),       // Yellow
        4 => Color::from_rgb(0.0, 0.0, 0.8),       // Blue
        5 => Color::from_rgb(0.8, 0.0, 0.8),       // Magenta
        6 => Color::from_rgb(0.0, 0.8, 0.8),       // Cyan
        7 => Color::from_rgb(0.75, 0.75, 0.75),    // White

        // 8-15: Bright colors
        8 => Color::from_rgb(0.5, 0.5, 0.5),       // Bright Black (Gray)
        9 => Color::from_rgb(1.0, 0.0, 0.0),       // Bright Red
        10 => Color::from_rgb(0.0, 1.0, 0.0),      // Bright Green
        11 => Color::from_rgb(1.0, 1.0, 0.0),      // Bright Yellow
        12 => Color::from_rgb(0.0, 0.0, 1.0),      // Bright Blue
        13 => Color::from_rgb(1.0, 0.0, 1.0),      // Bright Magenta
        14 => Color::from_rgb(0.0, 1.0, 1.0),      // Bright Cyan
        15 => Color::from_rgb(1.0, 1.0, 1.0),      // Bright White

        // 16-231: 216-color cube (6x6x6)
        16..=231 => {
            let idx = idx - 16;
            let r = ((idx / 36) % 6) as f32 / 5.0;
            let g = ((idx / 6) % 6) as f32 / 5.0;
            let b = (idx % 6) as f32 / 5.0;
            Color::from_rgb(r, g, b)
        }

        // 232-255: Grayscale ramp
        232..=255 => {
            let gray = ((idx - 232) as f32 * 10.0 + 8.0) / 255.0;
            Color::from_rgb(gray, gray, gray)
        }
    }
}

// ════════════════════════════════════════════════════════════════════
// Plain Text Conversion (for clipboard)
// ════════════════════════════════════════════════════════════════════

/// Convert snapshot to plain text (for clipboard copy).
/// Strips all color codes and returns just the text content.
/// Skips empty leading rows.
pub fn snapshot_to_plain(snapshot: &Snapshot) -> String {
    let mut result = String::new();
    let rows = snapshot.rows();

    if rows == 0 {
        return result;
    }

    // Find first non-empty row
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
        let line: String = row.iter().map(|(c, _)| *c).collect();
        let trimmed = line.trim_end();

        // Skip completely empty lines at the VERY end, but keep internal spacing
        if row_idx == rows - 1 && trimmed.is_empty() {
            continue;
        }

        result.push_str(trimmed);
        result.push('\n');
    }

    // Remove trailing newline
    if result.ends_with('\n') {
        result.pop();
    }

    result
}

// ════════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_labels() {
        assert_eq!(ThemeName::Default.label(), "default");
        assert_eq!(ThemeName::Monokai.label(), "monokai");
    }

    #[test]
    fn test_theme_from_str() {
        assert_eq!(ThemeName::from_str("default"), Some(ThemeName::Default));
        assert_eq!(ThemeName::from_str("MONOKAI"), Some(ThemeName::Monokai));
        assert_eq!(ThemeName::from_str("invalid"), None);
    }

    #[test]
    fn test_direct_spans_empty() {
        let spans = direct_to_spans("");
        assert!(spans.is_empty());
    }

    #[test]
    fn test_direct_spans_single_line() {
        let spans = direct_to_spans("test");
        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn test_indexed_color_standard() {
        let black = indexed_color(0);
        let red = indexed_color(1);
        assert_eq!(black.r, 0.0);
        assert_eq!(red.r, 0.8);
    }

    #[test]
    fn test_color_conversion() {
        let default = mycolor_to_iced(&MyColor::Default);
        assert!(default.r > 0.8); // Light color

        let red = mycolor_to_iced(&MyColor::Red);
        assert!(red.r > 0.5);
        assert!(red.g < 0.5);
    }
}
