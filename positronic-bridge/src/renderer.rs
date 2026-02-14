//! Terminal Renderer — Converts PTY snapshots and direct output into
//! colored iced `Span`s for display via `rich_text`.
//!
//! Supports multiple terminal color themes.

use iced::widget::text::Span;
use iced::Color;
use positronic_core::state_machine::{MyColor, Snapshot};

// ════════════════════════════════════════════════════════════════════
// Theme System
// ════════════════════════════════════════════════════════════════════

/// A named terminal color theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeName {
    Default,
    Cyberpunk,
    Solarized,
    Monokai,
}

impl ThemeName {
    pub fn all() -> &'static [ThemeName] {
        &[ThemeName::Default, ThemeName::Cyberpunk, ThemeName::Solarized, ThemeName::Monokai]
    }

    pub fn label(&self) -> &'static str {
        match self {
            ThemeName::Default => "Default",
            ThemeName::Cyberpunk => "Cyberpunk",
            ThemeName::Solarized => "Solarized",
            ThemeName::Monokai => "Monokai",
        }
    }

    pub fn from_str(s: &str) -> Option<ThemeName> {
        match s.to_lowercase().as_str() {
            "default" => Some(ThemeName::Default),
            "cyberpunk" | "cyber" => Some(ThemeName::Cyberpunk),
            "solarized" | "solar" => Some(ThemeName::Solarized),
            "monokai" | "mono" => Some(ThemeName::Monokai),
            _ => None,
        }
    }
}

/// Terminal color palette — maps the 16 named ANSI colors.
struct Palette {
    default_fg: Color,
    black: Color,
    red: Color,
    green: Color,
    yellow: Color,
    blue: Color,
    magenta: Color,
    cyan: Color,
    white: Color,
    bright_black: Color,
    bright_red: Color,
    bright_green: Color,
    bright_yellow: Color,
    bright_blue: Color,
    bright_magenta: Color,
    bright_cyan: Color,
    bright_white: Color,
}

fn palette_for(theme: ThemeName) -> Palette {
    match theme {
        ThemeName::Default => Palette {
            default_fg:     Color::from_rgb(0.85, 0.85, 0.85),
            black:          Color::from_rgb(0.15, 0.15, 0.15),
            red:            Color::from_rgb(0.90, 0.30, 0.30),
            green:          Color::from_rgb(0.35, 0.90, 0.35),
            yellow:         Color::from_rgb(0.95, 0.85, 0.30),
            blue:           Color::from_rgb(0.40, 0.55, 0.95),
            magenta:        Color::from_rgb(0.85, 0.40, 0.85),
            cyan:           Color::from_rgb(0.40, 0.90, 0.90),
            white:          Color::from_rgb(0.90, 0.90, 0.90),
            bright_black:   Color::from_rgb(0.45, 0.45, 0.45),
            bright_red:     Color::from_rgb(1.0, 0.45, 0.45),
            bright_green:   Color::from_rgb(0.45, 1.0, 0.45),
            bright_yellow:  Color::from_rgb(1.0, 1.0, 0.45),
            bright_blue:    Color::from_rgb(0.55, 0.70, 1.0),
            bright_magenta: Color::from_rgb(1.0, 0.55, 1.0),
            bright_cyan:    Color::from_rgb(0.55, 1.0, 1.0),
            bright_white:   Color::from_rgb(1.0, 1.0, 1.0),
        },
        ThemeName::Cyberpunk => Palette {
            default_fg:     Color::from_rgb(0.0, 1.0, 0.85),      // neon cyan
            black:          Color::from_rgb(0.05, 0.02, 0.10),
            red:            Color::from_rgb(1.0, 0.15, 0.40),      // hot pink
            green:          Color::from_rgb(0.0, 1.0, 0.45),       // neon green
            yellow:         Color::from_rgb(1.0, 0.85, 0.0),       // electric yellow
            blue:           Color::from_rgb(0.20, 0.40, 1.0),      // deep blue
            magenta:        Color::from_rgb(1.0, 0.0, 0.80),       // magenta
            cyan:           Color::from_rgb(0.0, 1.0, 1.0),        // pure cyan
            white:          Color::from_rgb(0.90, 0.90, 0.95),
            bright_black:   Color::from_rgb(0.35, 0.30, 0.45),
            bright_red:     Color::from_rgb(1.0, 0.35, 0.55),
            bright_green:   Color::from_rgb(0.30, 1.0, 0.65),
            bright_yellow:  Color::from_rgb(1.0, 1.0, 0.30),
            bright_blue:    Color::from_rgb(0.40, 0.55, 1.0),
            bright_magenta: Color::from_rgb(1.0, 0.40, 1.0),
            bright_cyan:    Color::from_rgb(0.40, 1.0, 1.0),
            bright_white:   Color::from_rgb(1.0, 1.0, 1.0),
        },
        ThemeName::Solarized => Palette {
            default_fg:     Color::from_rgb(0.51, 0.58, 0.59),     // base0
            black:          Color::from_rgb(0.0, 0.17, 0.21),      // base03
            red:            Color::from_rgb(0.86, 0.20, 0.18),     // red
            green:          Color::from_rgb(0.52, 0.60, 0.0),      // green
            yellow:         Color::from_rgb(0.71, 0.54, 0.0),      // yellow
            blue:           Color::from_rgb(0.15, 0.55, 0.82),     // blue
            magenta:        Color::from_rgb(0.83, 0.21, 0.51),     // magenta
            cyan:           Color::from_rgb(0.16, 0.63, 0.60),     // cyan
            white:          Color::from_rgb(0.93, 0.91, 0.84),     // base3
            bright_black:   Color::from_rgb(0.0, 0.26, 0.33),      // base02
            bright_red:     Color::from_rgb(0.80, 0.29, 0.09),     // orange
            bright_green:   Color::from_rgb(0.35, 0.43, 0.46),     // base01
            bright_yellow:  Color::from_rgb(0.40, 0.48, 0.51),     // base00
            bright_blue:    Color::from_rgb(0.51, 0.58, 0.59),     // base0
            bright_magenta: Color::from_rgb(0.42, 0.44, 0.77),     // violet
            bright_cyan:    Color::from_rgb(0.58, 0.63, 0.63),     // base1
            bright_white:   Color::from_rgb(0.99, 0.96, 0.89),     // base3
        },
        ThemeName::Monokai => Palette {
            default_fg:     Color::from_rgb(0.97, 0.97, 0.95),     // #f8f8f2
            black:          Color::from_rgb(0.15, 0.16, 0.13),     // #272822
            red:            Color::from_rgb(0.98, 0.15, 0.45),     // #f92672
            green:          Color::from_rgb(0.65, 0.89, 0.18),     // #a6e22e
            yellow:         Color::from_rgb(0.90, 0.86, 0.45),     // #e6db74
            blue:           Color::from_rgb(0.40, 0.85, 0.94),     // #66d9ef
            magenta:        Color::from_rgb(0.68, 0.51, 1.0),      // #ae81ff
            cyan:           Color::from_rgb(0.65, 0.89, 0.18),     // reuse green
            white:          Color::from_rgb(0.97, 0.97, 0.95),
            bright_black:   Color::from_rgb(0.46, 0.44, 0.37),     // #75715e
            bright_red:     Color::from_rgb(0.98, 0.42, 0.60),
            bright_green:   Color::from_rgb(0.75, 0.95, 0.40),
            bright_yellow:  Color::from_rgb(0.95, 0.92, 0.60),
            bright_blue:    Color::from_rgb(0.55, 0.90, 0.97),
            bright_magenta: Color::from_rgb(0.78, 0.66, 1.0),
            bright_cyan:    Color::from_rgb(0.75, 0.95, 0.40),
            bright_white:   Color::from_rgb(1.0, 1.0, 1.0),
        },
    }
}

// ════════════════════════════════════════════════════════════════════
// Terminal color conversion
// ════════════════════════════════════════════════════════════════════

/// Convert a positronic `MyColor` into an iced `Color` using the given theme.
pub fn mycolor_to_iced(c: MyColor, theme: ThemeName) -> Color {
    let p = palette_for(theme);
    match c {
        MyColor::Default => p.default_fg,
        MyColor::Black => p.black,
        MyColor::Red => p.red,
        MyColor::Green => p.green,
        MyColor::Yellow => p.yellow,
        MyColor::Blue => p.blue,
        MyColor::Magenta => p.magenta,
        MyColor::Cyan => p.cyan,
        MyColor::White => p.white,
        MyColor::BrightBlack => p.bright_black,
        MyColor::BrightRed => p.bright_red,
        MyColor::BrightGreen => p.bright_green,
        MyColor::BrightYellow => p.bright_yellow,
        MyColor::BrightBlue => p.bright_blue,
        MyColor::BrightMagenta => p.bright_magenta,
        MyColor::BrightCyan => p.bright_cyan,
        MyColor::BrightWhite => p.bright_white,
        MyColor::Rgb(r, g, b) => Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0),
        MyColor::Indexed(idx) => indexed_to_color(idx),
    }
}

/// Convert a 256-color index to RGB.
fn indexed_to_color(idx: u8) -> Color {
    match idx {
        0 => Color::from_rgb(0.0, 0.0, 0.0),
        1 => Color::from_rgb(0.80, 0.0, 0.0),
        2 => Color::from_rgb(0.0, 0.80, 0.0),
        3 => Color::from_rgb(0.80, 0.80, 0.0),
        4 => Color::from_rgb(0.0, 0.0, 0.80),
        5 => Color::from_rgb(0.80, 0.0, 0.80),
        6 => Color::from_rgb(0.0, 0.80, 0.80),
        7 => Color::from_rgb(0.75, 0.75, 0.75),
        8 => Color::from_rgb(0.50, 0.50, 0.50),
        9 => Color::from_rgb(1.0, 0.0, 0.0),
        10 => Color::from_rgb(0.0, 1.0, 0.0),
        11 => Color::from_rgb(1.0, 1.0, 0.0),
        12 => Color::from_rgb(0.0, 0.0, 1.0),
        13 => Color::from_rgb(1.0, 0.0, 1.0),
        14 => Color::from_rgb(0.0, 1.0, 1.0),
        15 => Color::from_rgb(1.0, 1.0, 1.0),
        // 16-231: 6x6x6 color cube
        16..=231 => {
            let idx = idx - 16;
            let b = (idx % 6) as f32 / 5.0;
            let g = ((idx / 6) % 6) as f32 / 5.0;
            let r = (idx / 36) as f32 / 5.0;
            Color::from_rgb(r, g, b)
        }
        // 232-255: grayscale ramp
        232..=255 => {
            let level = (idx - 232) as f32 / 23.0;
            Color::from_rgb(level, level, level)
        }
    }
}

// ════════════════════════════════════════════════════════════════════
// Snapshot → Spans (colored terminal output)
// ════════════════════════════════════════════════════════════════════

/// Convert a PTY snapshot into a Vec of colored Spans.
/// Adjacent characters with the same color are coalesced into a single span.
/// Trailing spaces on each line are trimmed. Trailing blank lines removed.
pub fn snapshot_to_spans(snapshot: &Snapshot, theme: ThemeName) -> Vec<Span<'static>> {
    let rows = snapshot.rows();
    let cols = snapshot.cols();

    if rows == 0 || cols == 0 {
        return vec![];
    }

    // First, figure out how many non-blank lines we have
    let mut last_nonempty_row = 0;
    for row_idx in 0..rows {
        let row = &snapshot[row_idx];
        let has_content = row.iter().any(|(c, _)| *c != ' ');
        if has_content {
            last_nonempty_row = row_idx;
        }
    }

    let mut spans: Vec<Span<'static>> = Vec::with_capacity((last_nonempty_row + 1) * 4);

    for row_idx in 0..=last_nonempty_row {
        let row = &snapshot[row_idx];

        // Find the last non-space column
        let mut last_col = 0;
        for col_idx in 0..cols {
            if row[col_idx].0 != ' ' {
                last_col = col_idx;
            }
        }

        // Coalesce adjacent same-color characters
        let mut current_text = String::new();
        let mut current_color = MyColor::Default;
        let end = last_col + 1; // include the last non-space character

        for col_idx in 0..end.min(cols) {
            let (ch, color) = row[col_idx];

            if col_idx == 0 {
                current_color = color;
                current_text.push(ch);
            } else if color == current_color {
                current_text.push(ch);
            } else {
                // Flush the current span
                if !current_text.is_empty() {
                    spans.push(
                        Span::new(current_text.clone())
                            .color(mycolor_to_iced(current_color, theme)),
                    );
                    current_text.clear();
                }
                current_color = color;
                current_text.push(ch);
            }
        }

        // Flush remaining text for this line
        if !current_text.is_empty() {
            spans.push(
                Span::new(current_text).color(mycolor_to_iced(current_color, theme)),
            );
        }

        // Add newline between rows (not after the last)
        if row_idx < last_nonempty_row {
            spans.push(Span::new("\n"));
        }
    }

    spans
}

// ════════════════════════════════════════════════════════════════════
// Direct output → Spans (! command results)
// ════════════════════════════════════════════════════════════════════

/// Color categories for direct output lines.
const COLOR_PROMPT: Color = Color { r: 0.35, g: 0.90, b: 0.35, a: 1.0 };    // green
const COLOR_ERROR: Color = Color { r: 1.0, g: 0.45, b: 0.45, a: 1.0 };      // red
const COLOR_SUCCESS: Color = Color { r: 0.35, g: 0.90, b: 0.35, a: 1.0 };   // green
const COLOR_INFO: Color = Color { r: 0.55, g: 0.70, b: 1.0, a: 1.0 };       // blue
const COLOR_WARN: Color = Color { r: 1.0, g: 0.85, b: 0.30, a: 1.0 };       // yellow
const COLOR_MUTED: Color = Color { r: 0.55, g: 0.55, b: 0.55, a: 1.0 };     // gray
const COLOR_HEADER: Color = Color { r: 0.55, g: 0.85, b: 1.0, a: 1.0 };     // light cyan
const COLOR_DEFAULT: Color = Color { r: 0.82, g: 0.82, b: 0.82, a: 1.0 };   // light gray

/// Convert direct output text (from ! commands) into colored spans.
/// Applies heuristic coloring based on emoji prefixes and patterns.
pub fn direct_to_spans(text: &str) -> Vec<Span<'static>> {
    if text.is_empty() {
        return vec![];
    }

    let lines: Vec<&str> = text.lines().collect();
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(lines.len() * 2);

    for (i, line) in lines.iter().enumerate() {
        let color = classify_line(line);
        spans.push(Span::new(line.to_string()).color(color));

        if i + 1 < lines.len() {
            spans.push(Span::new("\n"));
        }
    }

    spans
}

/// Classify a line of direct output to determine its display color.
fn classify_line(line: &str) -> Color {
    let trimmed = line.trim();

    // Empty lines
    if trimmed.is_empty() {
        return COLOR_DEFAULT;
    }

    // Prompt echo
    if trimmed.starts_with("➜") {
        return COLOR_PROMPT;
    }

    // Errors
    if trimmed.starts_with("❌") || trimmed.contains("error:") || trimmed.contains("Error:") {
        return COLOR_ERROR;
    }

    // Success
    if trimmed.starts_with("✅") {
        return COLOR_SUCCESS;
    }

    // Warnings / suggestions
    if trimmed.starts_with("💡") || trimmed.starts_with("⚠") {
        return COLOR_WARN;
    }

    // Info / status
    if trimmed.starts_with("📊") || trimmed.starts_with("📜") || trimmed.starts_with("🔗")
        || trimmed.starts_with("🔖") || trimmed.starts_with("📤") || trimmed.starts_with("📡")
        || trimmed.starts_with("🔌")
    {
        return COLOR_INFO;
    }

    // Headers / branding
    if trimmed.starts_with("⚡") || trimmed.starts_with("🧠") || trimmed.starts_with("🔒")
        || trimmed.starts_with("🚀")
    {
        return COLOR_HEADER;
    }

    // Section dividers
    if trimmed.starts_with("─") || trimmed.starts_with("═") || trimmed.starts_with("··") {
        return COLOR_MUTED;
    }

    // Indented content (sub-items under a header)
    if line.starts_with("  ") {
        // Check for key=value or label: value patterns
        if trimmed.contains("→") || trimmed.contains("│") {
            return COLOR_INFO;
        }
        return COLOR_DEFAULT;
    }

    COLOR_DEFAULT
}

// ════════════════════════════════════════════════════════════════════
// Plain text export (for clipboard copy)
// ════════════════════════════════════════════════════════════════════

/// Extract plain text from a snapshot (for clipboard copy).
pub fn snapshot_to_plain(snapshot: &Snapshot) -> String {
    let rows = snapshot.rows();
    let cols = snapshot.cols();

    let mut lines: Vec<String> = Vec::with_capacity(rows);
    for row_idx in 0..rows {
        let row = &snapshot[row_idx];
        let mut s: String = row.iter().map(|(c, _)| *c).collect();
        // Trim trailing spaces
        while s.ends_with(' ') {
            s.pop();
        }
        lines.push(s);
    }

    // Trim trailing empty lines
    while matches!(lines.last(), Some(l) if l.trim().is_empty()) {
        lines.pop();
    }

    lines.join("\n")
}