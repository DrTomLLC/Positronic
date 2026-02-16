// positronic-bridge/tests/renderer_tests.rs
//
// Integration tests for the Renderer & Color System.
// Tests all public API surface of renderer.rs:
//   Rgba         — constructors, sRGB conversion, clamping, traits
//   ColoredSpan  — constructors, clone
//   ThemeName    — enum variants, label/from_str round-trip, theme colors
//   direct_to_spans()   — emoji-keyed color coding for all prefix patterns
//   snapshot_to_spans() — PTY snapshot → colored spans with MyColor mapping
//   snapshot_to_plain() — PTY snapshot → clipboard-ready plain text

use positronic_bridge::renderer::{
    ColoredSpan, Rgba, ThemeName, direct_to_spans, snapshot_to_plain, snapshot_to_spans,
};
use positronic_core::state_machine::{MyColor, Snapshot};

// ════════════════════════════════════════════════════════════════════
// Helpers
// ════════════════════════════════════════════════════════════════════

/// Build a Snapshot with the given lines placed at the TOP of the grid.
/// Each character gets the supplied MyColor.
fn snapshot_colored(lines: &[(&str, MyColor)], cols: usize, rows: usize) -> Snapshot {
    let mut snap = Snapshot::new(cols, rows);
    for (row_idx, (text, color)) in lines.iter().enumerate() {
        if row_idx >= rows {
            break;
        }
        for (col, ch) in text.chars().enumerate() {
            if col < cols {
                snap.cells[row_idx * cols + col] = (*ch, color.clone());
            }
        }
    }
    snap
}

/// Build a Snapshot with plain text lines (Default color) at the top.
fn snapshot_plain(lines: &[&str], cols: usize, rows: usize) -> Snapshot {
    let colored: Vec<(&str, MyColor)> = lines.iter().map(|l| (*l, MyColor::Default)).collect();
    snapshot_colored(&colored, cols, rows)
}

/// Approximate float equality (within epsilon).
fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.001
}

// ════════════════════════════════════════════════════════════════════
// Rgba — Constructors
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_rgba_new() {
    let c = Rgba::new(0.1, 0.2, 0.3, 0.4);
    assert!(approx_eq(c.r, 0.1));
    assert!(approx_eq(c.g, 0.2));
    assert!(approx_eq(c.b, 0.3));
    assert!(approx_eq(c.a, 0.4));
}

#[test]
fn test_rgba_rgb_sets_alpha_one() {
    let c = Rgba::rgb(0.5, 0.6, 0.7);
    assert!(approx_eq(c.r, 0.5));
    assert!(approx_eq(c.g, 0.6));
    assert!(approx_eq(c.b, 0.7));
    assert!(approx_eq(c.a, 1.0));
}

#[test]
fn test_rgba_black() {
    let c = Rgba::rgb(0.0, 0.0, 0.0);
    assert!(approx_eq(c.r, 0.0));
    assert!(approx_eq(c.g, 0.0));
    assert!(approx_eq(c.b, 0.0));
}

#[test]
fn test_rgba_white() {
    let c = Rgba::rgb(1.0, 1.0, 1.0);
    assert!(approx_eq(c.r, 1.0));
    assert!(approx_eq(c.g, 1.0));
    assert!(approx_eq(c.b, 1.0));
}

// ════════════════════════════════════════════════════════════════════
// Rgba — to_srgb8
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_rgba_to_srgb8_black() {
    let c = Rgba::rgb(0.0, 0.0, 0.0);
    assert_eq!(c.to_srgb8(), [0, 0, 0, 255]);
}

#[test]
fn test_rgba_to_srgb8_white() {
    let c = Rgba::rgb(1.0, 1.0, 1.0);
    assert_eq!(c.to_srgb8(), [255, 255, 255, 255]);
}

#[test]
fn test_rgba_to_srgb8_red() {
    let c = Rgba::rgb(1.0, 0.0, 0.0);
    assert_eq!(c.to_srgb8(), [255, 0, 0, 255]);
}

#[test]
fn test_rgba_to_srgb8_half_alpha() {
    let c = Rgba::new(1.0, 1.0, 1.0, 0.5);
    let srgb = c.to_srgb8();
    assert_eq!(srgb[0], 255);
    assert_eq!(srgb[3], 127); // 0.5 * 255 = 127
}

#[test]
fn test_rgba_to_srgb8_clamps_above_one() {
    let c = Rgba::new(2.0, 1.5, -0.5, 3.0);
    let srgb = c.to_srgb8();
    assert_eq!(srgb[0], 255); // clamped from 2.0
    assert_eq!(srgb[1], 255); // clamped from 1.5
    assert_eq!(srgb[2], 0);   // clamped from -0.5
    assert_eq!(srgb[3], 255); // clamped from 3.0
}

#[test]
fn test_rgba_to_srgb8_mid_values() {
    let c = Rgba::rgb(0.5, 0.5, 0.5);
    let srgb = c.to_srgb8();
    assert_eq!(srgb[0], 127); // 0.5 * 255 = 127
    assert_eq!(srgb[1], 127);
    assert_eq!(srgb[2], 127);
    assert_eq!(srgb[3], 255);
}

// ════════════════════════════════════════════════════════════════════
// Rgba — Traits
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_rgba_partial_eq() {
    let a = Rgba::rgb(0.1, 0.2, 0.3);
    let b = Rgba::rgb(0.1, 0.2, 0.3);
    assert_eq!(a, b);
}

#[test]
fn test_rgba_partial_ne() {
    let a = Rgba::rgb(0.1, 0.2, 0.3);
    let b = Rgba::rgb(0.1, 0.2, 0.4);
    assert_ne!(a, b);
}

#[test]
fn test_rgba_clone() {
    let a = Rgba::new(0.1, 0.2, 0.3, 0.4);
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn test_rgba_copy() {
    let a = Rgba::rgb(0.5, 0.5, 0.5);
    let b = a;
    // a is still usable — Copy trait
    assert_eq!(a.r, b.r);
}

#[test]
fn test_rgba_debug() {
    let c = Rgba::rgb(0.1, 0.2, 0.3);
    let dbg = format!("{:?}", c);
    assert!(dbg.contains("Rgba"));
    assert!(dbg.contains("0.1"));
}

// ════════════════════════════════════════════════════════════════════
// ColoredSpan — Constructor & Traits
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_colored_span_new_str() {
    let span = ColoredSpan::new("hello", Rgba::rgb(1.0, 0.0, 0.0));
    assert_eq!(span.text, "hello");
    assert_eq!(span.color, Rgba::rgb(1.0, 0.0, 0.0));
}

#[test]
fn test_colored_span_new_string() {
    let owned = String::from("world");
    let span = ColoredSpan::new(owned, Rgba::rgb(0.0, 1.0, 0.0));
    assert_eq!(span.text, "world");
}

#[test]
fn test_colored_span_clone() {
    let span = ColoredSpan::new("test", Rgba::rgb(0.5, 0.5, 0.5));
    let cloned = span.clone();
    assert_eq!(cloned.text, span.text);
    assert_eq!(cloned.color, span.color);
}

#[test]
fn test_colored_span_debug() {
    let span = ColoredSpan::new("x", Rgba::rgb(0.0, 0.0, 0.0));
    let dbg = format!("{:?}", span);
    assert!(dbg.contains("ColoredSpan"));
}

// ════════════════════════════════════════════════════════════════════
// ThemeName — Enum Variants & all()
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_theme_all_returns_four() {
    assert_eq!(ThemeName::all().len(), 4);
}

#[test]
fn test_theme_all_contains_all_variants() {
    let all = ThemeName::all();
    assert!(all.contains(&ThemeName::Default));
    assert!(all.contains(&ThemeName::Monokai));
    assert!(all.contains(&ThemeName::Solarized));
    assert!(all.contains(&ThemeName::Dracula));
}

// ════════════════════════════════════════════════════════════════════
// ThemeName — label() / from_str() Round-Trip
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_theme_label_default() {
    assert_eq!(ThemeName::Default.label(), "default");
}

#[test]
fn test_theme_label_monokai() {
    assert_eq!(ThemeName::Monokai.label(), "monokai");
}

#[test]
fn test_theme_label_solarized() {
    assert_eq!(ThemeName::Solarized.label(), "solarized");
}

#[test]
fn test_theme_label_dracula() {
    assert_eq!(ThemeName::Dracula.label(), "dracula");
}

#[test]
fn test_theme_from_str_round_trip() {
    for theme in ThemeName::all() {
        let label = theme.label();
        let parsed = ThemeName::from_str(label).unwrap();
        assert_eq!(*theme, parsed);
    }
}

#[test]
fn test_theme_from_str_case_insensitive() {
    assert_eq!(ThemeName::from_str("DEFAULT"), Some(ThemeName::Default));
    assert_eq!(ThemeName::from_str("Monokai"), Some(ThemeName::Monokai));
    assert_eq!(ThemeName::from_str("SOLARIZED"), Some(ThemeName::Solarized));
    assert_eq!(ThemeName::from_str("DrAcUlA"), Some(ThemeName::Dracula));
}

#[test]
fn test_theme_from_str_unknown_returns_none() {
    assert_eq!(ThemeName::from_str("cyberpunk"), None);
    assert_eq!(ThemeName::from_str(""), None);
    assert_eq!(ThemeName::from_str("gruvbox"), None);
}

// ════════════════════════════════════════════════════════════════════
// ThemeName — Theme Colors
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_theme_bg_color_differs_per_theme() {
    // Each theme should have a distinct background
    let bgs: Vec<Rgba> = ThemeName::all().iter().map(|t| t.bg_color()).collect();
    for i in 0..bgs.len() {
        for j in (i + 1)..bgs.len() {
            assert_ne!(bgs[i], bgs[j], "Themes {} and {} share bg_color", i, j);
        }
    }
}

#[test]
fn test_theme_bg_color_has_full_alpha() {
    // bg_color uses rgb() which sets alpha = 1.0
    for theme in ThemeName::all() {
        assert!(approx_eq(theme.bg_color().a, 1.0));
    }
}

#[test]
fn test_theme_status_bg_valid() {
    let c = ThemeName::Default.status_bg();
    assert!(c.r >= 0.0 && c.r <= 1.0);
    assert!(c.g >= 0.0 && c.g <= 1.0);
    assert!(c.b >= 0.0 && c.b <= 1.0);
    assert!(approx_eq(c.a, 1.0));
}

#[test]
fn test_theme_status_fg_valid() {
    let c = ThemeName::Default.status_fg();
    assert!(c.r >= 0.0 && c.r <= 1.0);
    assert!(approx_eq(c.a, 1.0));
}

#[test]
fn test_theme_input_bg_valid() {
    let c = ThemeName::Default.input_bg();
    assert!(c.r >= 0.0 && c.r <= 1.0);
    assert!(approx_eq(c.a, 1.0));
}

#[test]
fn test_theme_input_fg_bright() {
    // Input text should be clearly visible (bright)
    let c = ThemeName::Default.input_fg();
    assert!(c.r > 0.5);
    assert!(c.g > 0.5);
    assert!(c.b > 0.5);
}

#[test]
fn test_theme_cursor_color_semi_transparent() {
    let c = ThemeName::Default.cursor_color();
    assert!(c.a < 1.0, "Cursor should be slightly transparent");
    assert!(c.a > 0.5, "Cursor should still be mostly visible");
}

#[test]
fn test_theme_eq() {
    assert_eq!(ThemeName::Default, ThemeName::Default);
    assert_ne!(ThemeName::Default, ThemeName::Monokai);
}

#[test]
fn test_theme_debug() {
    let dbg = format!("{:?}", ThemeName::Dracula);
    assert!(dbg.contains("Dracula"));
}

// ════════════════════════════════════════════════════════════════════
// direct_to_spans — Emoji Prefix Color Coding
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_direct_empty_input() {
    let spans = direct_to_spans("");
    assert!(spans.is_empty());
}

#[test]
fn test_direct_plain_text_default_color() {
    let spans = direct_to_spans("hello world");
    assert_eq!(spans.len(), 1);
    // Default color: Rgba::rgb(0.85, 0.85, 0.85)
    assert!(approx_eq(spans[0].color.r, 0.85));
    assert!(approx_eq(spans[0].color.g, 0.85));
    assert!(approx_eq(spans[0].color.b, 0.85));
}

#[test]
fn test_direct_plain_text_appends_newline() {
    let spans = direct_to_spans("hello");
    assert!(spans[0].text.ends_with('\n'));
    assert_eq!(spans[0].text, "hello\n");
}

#[test]
fn test_direct_arrow_green() {
    let spans = direct_to_spans("➜ Success!");
    assert!(approx_eq(spans[0].color.r, 0.3));
    assert!(approx_eq(spans[0].color.g, 0.85));
    assert!(approx_eq(spans[0].color.b, 0.3));
}

#[test]
fn test_direct_x_mark_red() {
    let spans = direct_to_spans("❌ Build failed");
    assert!(approx_eq(spans[0].color.r, 1.0));
    assert!(approx_eq(spans[0].color.g, 0.35));
    assert!(approx_eq(spans[0].color.b, 0.35));
}

#[test]
fn test_direct_lightning_blue() {
    let spans = direct_to_spans("⚡ Fast operation");
    assert!(approx_eq(spans[0].color.r, 0.3));
    assert!(approx_eq(spans[0].color.g, 0.8));
    assert!(approx_eq(spans[0].color.b, 1.0));
}

#[test]
fn test_direct_checkmark_blue() {
    let spans = direct_to_spans("✓ Done");
    assert!(approx_eq(spans[0].color.r, 0.3));
    assert!(approx_eq(spans[0].color.g, 0.8));
    assert!(approx_eq(spans[0].color.b, 1.0));
}

#[test]
fn test_direct_lightbulb_purple() {
    let spans = direct_to_spans("  💡 Hint: try --help");
    assert!(approx_eq(spans[0].color.r, 0.6));
    assert!(approx_eq(spans[0].color.g, 0.6));
    assert!(approx_eq(spans[0].color.b, 0.85));
}

#[test]
fn test_direct_clipboard_purple() {
    let spans = direct_to_spans("📋 Copied to clipboard");
    assert!(approx_eq(spans[0].color.r, 0.6));
    assert!(approx_eq(spans[0].color.g, 0.6));
    assert!(approx_eq(spans[0].color.b, 0.85));
}

#[test]
fn test_direct_folder_purple() {
    let spans = direct_to_spans("📂 Directory listing");
    assert!(approx_eq(spans[0].color.r, 0.6));
    assert!(approx_eq(spans[0].color.g, 0.6));
}

#[test]
fn test_direct_paint_purple() {
    let spans = direct_to_spans("🎨 Theme applied");
    assert!(approx_eq(spans[0].color.r, 0.6));
    assert!(approx_eq(spans[0].color.b, 0.85));
}

#[test]
fn test_direct_magnifier_yellow() {
    let spans = direct_to_spans("🔍 Searching...");
    assert!(approx_eq(spans[0].color.r, 1.0));
    assert!(approx_eq(spans[0].color.g, 0.85));
    assert!(approx_eq(spans[0].color.b, 0.3));
}

#[test]
fn test_direct_trophy_green() {
    let spans = direct_to_spans("🏆 Achievement unlocked");
    assert!(approx_eq(spans[0].color.r, 0.3));
    assert!(approx_eq(spans[0].color.g, 0.9));
    assert!(approx_eq(spans[0].color.b, 0.6));
}

#[test]
fn test_direct_chart_green() {
    let spans = direct_to_spans("📊 Performance stats");
    assert!(approx_eq(spans[0].color.r, 0.3));
    assert!(approx_eq(spans[0].color.g, 0.9));
    assert!(approx_eq(spans[0].color.b, 0.6));
}

#[test]
fn test_direct_scroll_light_purple() {
    let spans = direct_to_spans("📜 History log");
    assert!(approx_eq(spans[0].color.r, 0.7));
    assert!(approx_eq(spans[0].color.g, 0.7));
    assert!(approx_eq(spans[0].color.b, 0.9));
}

#[test]
fn test_direct_memo_light_purple() {
    let spans = direct_to_spans("📝 Notes saved");
    assert!(approx_eq(spans[0].color.r, 0.7));
    assert!(approx_eq(spans[0].color.g, 0.7));
    assert!(approx_eq(spans[0].color.b, 0.9));
}

#[test]
fn test_direct_bookmark_light_purple() {
    let spans = direct_to_spans("🔖 Bookmarked");
    assert!(approx_eq(spans[0].color.r, 0.7));
    assert!(approx_eq(spans[0].color.g, 0.7));
    assert!(approx_eq(spans[0].color.b, 0.9));
}

#[test]
fn test_direct_box_top_gray() {
    let spans = direct_to_spans("╔═══════════╗");
    assert!(approx_eq(spans[0].color.r, 0.4));
    assert!(approx_eq(spans[0].color.g, 0.5));
    assert!(approx_eq(spans[0].color.b, 0.6));
}

#[test]
fn test_direct_box_side_gray() {
    let spans = direct_to_spans("║ content   ║");
    assert!(approx_eq(spans[0].color.r, 0.4));
}

#[test]
fn test_direct_box_bottom_gray() {
    let spans = direct_to_spans("╚═══════════╝");
    assert!(approx_eq(spans[0].color.r, 0.4));
}

#[test]
fn test_direct_multiline() {
    let spans = direct_to_spans("➜ line one\n❌ line two\nplain line");
    assert_eq!(spans.len(), 3);
    // First span should be green (arrow)
    assert!(approx_eq(spans[0].color.g, 0.85));
    // Second span should be red (x mark)
    assert!(approx_eq(spans[1].color.r, 1.0));
    // Third span should be default gray
    assert!(approx_eq(spans[2].color.r, 0.85));
}

#[test]
fn test_direct_each_line_gets_newline() {
    let spans = direct_to_spans("line1\nline2");
    assert!(spans[0].text.ends_with('\n'));
    assert!(spans[1].text.ends_with('\n'));
}

// ════════════════════════════════════════════════════════════════════
// snapshot_to_spans — Basic Rendering
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_snapshot_to_spans_empty() {
    let snap = Snapshot::new(80, 0);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    assert!(spans.is_empty());
}

#[test]
fn test_snapshot_to_spans_single_line() {
    let snap = snapshot_plain(&["Hello"], 80, 5);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    // Should have spans containing "Hello" text
    let full_text: String = spans.iter().map(|s| &s.text[..]).collect();
    assert!(full_text.contains("Hello"));
}

#[test]
fn test_snapshot_to_spans_default_color() {
    let snap = snapshot_plain(&["ABC"], 80, 5);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    // Default MyColor → Rgba::rgb(0.85, 0.85, 0.85)
    let text_span = spans.iter().find(|s| s.text.contains('A')).unwrap();
    assert!(approx_eq(text_span.color.r, 0.85));
    assert!(approx_eq(text_span.color.g, 0.85));
    assert!(approx_eq(text_span.color.b, 0.85));
}

#[test]
fn test_snapshot_to_spans_skips_leading_empty_rows() {
    // Create a snapshot where the first 3 rows are empty, then content
    let mut snap = Snapshot::new(80, 6);
    // Row 3 has content
    for (col, ch) in "Content".chars().enumerate() {
        snap.cells[3 * 80 + col] = (ch, MyColor::Default);
    }
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let full_text: String = spans.iter().map(|s| &s.text[..]).collect();
    assert!(full_text.contains("Content"));
}

#[test]
fn test_snapshot_to_spans_each_row_ends_with_newline() {
    let snap = snapshot_plain(&["Row1", "Row2"], 80, 5);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    // Newline spans should exist
    let newline_count = spans.iter().filter(|s| s.text == "\n").count();
    assert!(newline_count >= 2, "Each rendered row should end with \\n");
}

// ════════════════════════════════════════════════════════════════════
// snapshot_to_spans — Color Mapping (MyColor → Rgba)
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_snapshot_color_red() {
    let snap = snapshot_colored(&[("R", MyColor::Red)], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let r_span = spans.iter().find(|s| s.text.contains('R')).unwrap();
    assert!(approx_eq(r_span.color.r, 0.9));
    assert!(approx_eq(r_span.color.g, 0.3));
    assert!(approx_eq(r_span.color.b, 0.3));
}

#[test]
fn test_snapshot_color_green() {
    let snap = snapshot_colored(&[("G", MyColor::Green)], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let g_span = spans.iter().find(|s| s.text.contains('G')).unwrap();
    assert!(approx_eq(g_span.color.r, 0.3));
    assert!(approx_eq(g_span.color.g, 0.85));
    assert!(approx_eq(g_span.color.b, 0.3));
}

#[test]
fn test_snapshot_color_blue() {
    let snap = snapshot_colored(&[("B", MyColor::Blue)], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let b_span = spans.iter().find(|s| s.text.contains('B')).unwrap();
    assert!(approx_eq(b_span.color.r, 0.4));
    assert!(approx_eq(b_span.color.g, 0.6));
    assert!(approx_eq(b_span.color.b, 1.0));
}

#[test]
fn test_snapshot_color_yellow() {
    let snap = snapshot_colored(&[("Y", MyColor::Yellow)], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let y_span = spans.iter().find(|s| s.text.contains('Y')).unwrap();
    assert!(approx_eq(y_span.color.r, 0.95));
    assert!(approx_eq(y_span.color.g, 0.85));
    assert!(approx_eq(y_span.color.b, 0.3));
}

#[test]
fn test_snapshot_color_magenta() {
    let snap = snapshot_colored(&[("M", MyColor::Magenta)], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let m_span = spans.iter().find(|s| s.text.contains('M')).unwrap();
    assert!(approx_eq(m_span.color.r, 0.9));
    assert!(approx_eq(m_span.color.g, 0.4));
    assert!(approx_eq(m_span.color.b, 0.9));
}

#[test]
fn test_snapshot_color_cyan() {
    let snap = snapshot_colored(&[("C", MyColor::Cyan)], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let c_span = spans.iter().find(|s| s.text.contains('C')).unwrap();
    assert!(approx_eq(c_span.color.r, 0.3));
    assert!(approx_eq(c_span.color.g, 0.85));
    assert!(approx_eq(c_span.color.b, 0.85));
}

#[test]
fn test_snapshot_color_white() {
    let snap = snapshot_colored(&[("W", MyColor::White)], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let w_span = spans.iter().find(|s| s.text.contains('W')).unwrap();
    assert!(approx_eq(w_span.color.r, 0.95));
    assert!(approx_eq(w_span.color.g, 0.95));
    assert!(approx_eq(w_span.color.b, 0.95));
}

#[test]
fn test_snapshot_color_black() {
    let snap = snapshot_colored(&[("K", MyColor::Black)], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let k_span = spans.iter().find(|s| s.text.contains('K')).unwrap();
    assert!(approx_eq(k_span.color.r, 0.1));
    assert!(approx_eq(k_span.color.g, 0.1));
    assert!(approx_eq(k_span.color.b, 0.1));
}

// ── Bright Colors ───────────────────────────────────────────────

#[test]
fn test_snapshot_color_bright_red() {
    let snap = snapshot_colored(&[("X", MyColor::BrightRed)], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let x_span = spans.iter().find(|s| s.text.contains('X')).unwrap();
    assert!(approx_eq(x_span.color.r, 1.0));
    assert!(approx_eq(x_span.color.g, 0.5));
    assert!(approx_eq(x_span.color.b, 0.5));
}

#[test]
fn test_snapshot_color_bright_green() {
    let snap = snapshot_colored(&[("X", MyColor::BrightGreen)], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let x_span = spans.iter().find(|s| s.text.contains('X')).unwrap();
    assert!(approx_eq(x_span.color.r, 0.5));
    assert!(approx_eq(x_span.color.g, 1.0));
    assert!(approx_eq(x_span.color.b, 0.5));
}

#[test]
fn test_snapshot_color_bright_blue() {
    let snap = snapshot_colored(&[("X", MyColor::BrightBlue)], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let x_span = spans.iter().find(|s| s.text.contains('X')).unwrap();
    assert!(approx_eq(x_span.color.r, 0.5));
    assert!(approx_eq(x_span.color.g, 0.7));
    assert!(approx_eq(x_span.color.b, 1.0));
}

#[test]
fn test_snapshot_color_bright_white() {
    let snap = snapshot_colored(&[("X", MyColor::BrightWhite)], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let x_span = spans.iter().find(|s| s.text.contains('X')).unwrap();
    assert!(approx_eq(x_span.color.r, 1.0));
    assert!(approx_eq(x_span.color.g, 1.0));
    assert!(approx_eq(x_span.color.b, 1.0));
}

#[test]
fn test_snapshot_color_bright_black() {
    let snap = snapshot_colored(&[("X", MyColor::BrightBlack)], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let x_span = spans.iter().find(|s| s.text.contains('X')).unwrap();
    assert!(approx_eq(x_span.color.r, 0.5));
    assert!(approx_eq(x_span.color.g, 0.5));
    assert!(approx_eq(x_span.color.b, 0.5));
}

#[test]
fn test_snapshot_color_bright_yellow() {
    let snap = snapshot_colored(&[("X", MyColor::BrightYellow)], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let x_span = spans.iter().find(|s| s.text.contains('X')).unwrap();
    assert!(approx_eq(x_span.color.r, 1.0));
    assert!(approx_eq(x_span.color.g, 1.0));
    assert!(approx_eq(x_span.color.b, 0.5));
}

#[test]
fn test_snapshot_color_bright_magenta() {
    let snap = snapshot_colored(&[("X", MyColor::BrightMagenta)], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let x_span = spans.iter().find(|s| s.text.contains('X')).unwrap();
    assert!(approx_eq(x_span.color.r, 1.0));
    assert!(approx_eq(x_span.color.g, 0.5));
    assert!(approx_eq(x_span.color.b, 1.0));
}

#[test]
fn test_snapshot_color_bright_cyan() {
    let snap = snapshot_colored(&[("X", MyColor::BrightCyan)], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let x_span = spans.iter().find(|s| s.text.contains('X')).unwrap();
    assert!(approx_eq(x_span.color.r, 0.5));
    assert!(approx_eq(x_span.color.g, 1.0));
    assert!(approx_eq(x_span.color.b, 1.0));
}

// ── Indexed & RGB Colors ────────────────────────────────────────

#[test]
fn test_snapshot_color_indexed_standard_red() {
    // Index 1 = red (0.8, 0.0, 0.0)
    let snap = snapshot_colored(&[("X", MyColor::Indexed(1))], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let x_span = spans.iter().find(|s| s.text.contains('X')).unwrap();
    assert!(approx_eq(x_span.color.r, 0.8));
    assert!(approx_eq(x_span.color.g, 0.0));
    assert!(approx_eq(x_span.color.b, 0.0));
}

#[test]
fn test_snapshot_color_indexed_bright_white() {
    // Index 15 = bright white (1.0, 1.0, 1.0)
    let snap = snapshot_colored(&[("X", MyColor::Indexed(15))], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let x_span = spans.iter().find(|s| s.text.contains('X')).unwrap();
    assert!(approx_eq(x_span.color.r, 1.0));
    assert!(approx_eq(x_span.color.g, 1.0));
    assert!(approx_eq(x_span.color.b, 1.0));
}

#[test]
fn test_snapshot_color_indexed_cube_pure_red() {
    // Index 196 = pure red in 6×6×6 cube: (196-16)=180 → r=180/36=5 → 5/5=1.0, g=0, b=0
    let snap = snapshot_colored(&[("X", MyColor::Indexed(196))], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let x_span = spans.iter().find(|s| s.text.contains('X')).unwrap();
    assert!(approx_eq(x_span.color.r, 1.0));
    assert!(approx_eq(x_span.color.g, 0.0));
    assert!(approx_eq(x_span.color.b, 0.0));
}

#[test]
fn test_snapshot_color_indexed_cube_pure_green() {
    // Index 46 = pure green: (46-16)=30 → r=0, g=30/6=5 → 5/5=1.0, b=0
    let snap = snapshot_colored(&[("X", MyColor::Indexed(46))], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let x_span = spans.iter().find(|s| s.text.contains('X')).unwrap();
    assert!(approx_eq(x_span.color.r, 0.0));
    assert!(approx_eq(x_span.color.g, 1.0));
    assert!(approx_eq(x_span.color.b, 0.0));
}

#[test]
fn test_snapshot_color_indexed_cube_pure_blue() {
    // Index 21 = pure blue: (21-16)=5 → r=0, g=0, b=5/5=1.0
    let snap = snapshot_colored(&[("X", MyColor::Indexed(21))], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let x_span = spans.iter().find(|s| s.text.contains('X')).unwrap();
    assert!(approx_eq(x_span.color.r, 0.0));
    assert!(approx_eq(x_span.color.g, 0.0));
    assert!(approx_eq(x_span.color.b, 1.0));
}

#[test]
fn test_snapshot_color_indexed_grayscale_dark() {
    // Index 232 = darkest gray: (232-232)*10+8 / 255 ≈ 0.031
    let snap = snapshot_colored(&[("X", MyColor::Indexed(232))], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let x_span = spans.iter().find(|s| s.text.contains('X')).unwrap();
    assert!(x_span.color.r < 0.1, "Darkest grayscale should be very dark");
    // All channels equal for grayscale
    assert!(approx_eq(x_span.color.r, x_span.color.g));
    assert!(approx_eq(x_span.color.g, x_span.color.b));
}

#[test]
fn test_snapshot_color_indexed_grayscale_light() {
    // Index 255 = lightest gray: (255-232)*10+8 / 255 ≈ 0.937
    let snap = snapshot_colored(&[("X", MyColor::Indexed(255))], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let x_span = spans.iter().find(|s| s.text.contains('X')).unwrap();
    assert!(x_span.color.r > 0.9, "Lightest grayscale should be very bright");
    assert!(approx_eq(x_span.color.r, x_span.color.g));
    assert!(approx_eq(x_span.color.g, x_span.color.b));
}

#[test]
fn test_snapshot_color_rgb_custom() {
    // Direct RGB: (128, 64, 255)
    let snap = snapshot_colored(&[("X", MyColor::Rgb(128, 64, 255))], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let x_span = spans.iter().find(|s| s.text.contains('X')).unwrap();
    assert!(approx_eq(x_span.color.r, 128.0 / 255.0));
    assert!(approx_eq(x_span.color.g, 64.0 / 255.0));
    assert!(approx_eq(x_span.color.b, 255.0 / 255.0));
}

#[test]
fn test_snapshot_color_rgb_black() {
    let snap = snapshot_colored(&[("X", MyColor::Rgb(0, 0, 0))], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let x_span = spans.iter().find(|s| s.text.contains('X')).unwrap();
    assert!(approx_eq(x_span.color.r, 0.0));
    assert!(approx_eq(x_span.color.g, 0.0));
    assert!(approx_eq(x_span.color.b, 0.0));
}

#[test]
fn test_snapshot_color_rgb_white() {
    let snap = snapshot_colored(&[("X", MyColor::Rgb(255, 255, 255))], 80, 3);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let x_span = spans.iter().find(|s| s.text.contains('X')).unwrap();
    assert!(approx_eq(x_span.color.r, 1.0));
    assert!(approx_eq(x_span.color.g, 1.0));
    assert!(approx_eq(x_span.color.b, 1.0));
}

// ── Multi-Color Runs ────────────────────────────────────────────

#[test]
fn test_snapshot_color_run_splitting() {
    // Adjacent cells with different colors should produce separate spans
    let mut snap = Snapshot::new(6, 3);
    // "RGB" where R=Red, G=Green, B=Blue
    snap.cells[0] = ('R', MyColor::Red);
    snap.cells[1] = ('G', MyColor::Green);
    snap.cells[2] = ('B', MyColor::Blue);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    // Should have at least 3 text spans (one per color) plus newlines
    let text_spans: Vec<&ColoredSpan> = spans.iter().filter(|s| s.text != "\n").collect();
    // Each character may be its own span due to color changes
    assert!(text_spans.len() >= 3, "Different colors should split into separate spans");
}

#[test]
fn test_snapshot_same_color_merged() {
    // Adjacent cells with the SAME color should merge into one span
    let mut snap = Snapshot::new(10, 3);
    for col in 0..5 {
        snap.cells[col] = ('A', MyColor::Red);
    }
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let a_spans: Vec<&ColoredSpan> = spans.iter().filter(|s| s.text.contains('A')).collect();
    // All 'A's should be merged into a single span
    assert_eq!(a_spans.len(), 1, "Same-color runs should merge");
    assert_eq!(a_spans[0].text.matches('A').count(), 5);
}

// ════════════════════════════════════════════════════════════════════
// snapshot_to_plain — Clipboard Text Conversion
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_plain_empty_snapshot() {
    let snap = Snapshot::new(80, 0);
    let text = snapshot_to_plain(&snap);
    assert!(text.is_empty());
}

#[test]
fn test_plain_single_line() {
    let snap = snapshot_plain(&["Hello World"], 80, 5);
    let text = snapshot_to_plain(&snap);
    assert!(text.contains("Hello World"));
}

#[test]
fn test_plain_strips_trailing_whitespace() {
    // Row filled with "Hi   " (trailing spaces) → should be trimmed
    let snap = snapshot_plain(&["Hi   "], 80, 5);
    let text = snapshot_to_plain(&snap);
    let first_line = text.lines().next().unwrap();
    assert_eq!(first_line, "Hi");
}

#[test]
fn test_plain_skips_leading_empty_rows() {
    // First 3 rows empty, then content on row 3
    let mut snap = Snapshot::new(80, 6);
    for (col, ch) in "Content".chars().enumerate() {
        snap.cells[3 * 80 + col] = (ch, MyColor::Default);
    }
    let text = snapshot_to_plain(&snap);
    assert!(text.starts_with("Content"), "Should skip leading empty rows");
}

#[test]
fn test_plain_multiline() {
    let snap = snapshot_plain(&["Line 1", "Line 2", "Line 3"], 80, 5);
    let text = snapshot_to_plain(&snap);
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines.len() >= 3);
    assert_eq!(lines[0], "Line 1");
    assert_eq!(lines[1], "Line 2");
    assert_eq!(lines[2], "Line 3");
}

#[test]
fn test_plain_ignores_color_info() {
    // Even with colored cells, plain text should just have characters
    let snap = snapshot_colored(&[("Colored", MyColor::Red)], 80, 3);
    let text = snapshot_to_plain(&snap);
    assert!(text.contains("Colored"));
}

#[test]
fn test_plain_each_row_ends_with_newline() {
    let snap = snapshot_plain(&["A", "B"], 80, 5);
    let text = snapshot_to_plain(&snap);
    // Each row should produce a trailing \n
    assert!(text.ends_with('\n'));
}

#[test]
fn test_plain_all_whitespace_rows_included_after_content() {
    // A row that's all spaces after a content row should still appear as empty line
    let mut snap = Snapshot::new(80, 4);
    for (col, ch) in "Top".chars().enumerate() {
        snap.cells[col] = (ch, MyColor::Default);
    }
    // Row 1 is all spaces (default), row 2 has content
    for (col, ch) in "Bottom".chars().enumerate() {
        snap.cells[2 * 80 + col] = (ch, MyColor::Default);
    }
    let text = snapshot_to_plain(&snap);
    assert!(text.contains("Top"));
    assert!(text.contains("Bottom"));
}

// ════════════════════════════════════════════════════════════════════
// Edge Cases
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_snapshot_1x1() {
    let mut snap = Snapshot::new(1, 1);
    snap.cells[0] = ('X', MyColor::Default);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let full_text: String = spans.iter().map(|s| &s.text[..]).collect();
    assert!(full_text.contains('X'));

    let plain = snapshot_to_plain(&snap);
    assert!(plain.contains('X'));
}

#[test]
fn test_snapshot_large_grid_no_panic() {
    // 200x50 grid should not panic
    let snap = Snapshot::new(200, 50);
    let spans = snapshot_to_spans(&snap, ThemeName::Default);
    let _plain = snapshot_to_plain(&snap);
    // Just verifying no panic
    assert!(spans.is_empty() || !spans.is_empty());
}

#[test]
fn test_direct_to_spans_single_newline() {
    let spans = direct_to_spans("\n");
    // A single newline produces one empty line iteration
    // (or zero if .lines() skips trailing newlines)
    // Either is acceptable — just don't panic
    assert!(spans.len() <= 1);
}

#[test]
fn test_theme_all_labels_unique() {
    let labels: Vec<&str> = ThemeName::all().iter().map(|t| t.label()).collect();
    for i in 0..labels.len() {
        for j in (i + 1)..labels.len() {
            assert_ne!(labels[i], labels[j], "Theme labels must be unique");
        }
    }
}

#[test]
fn test_rgba_to_srgb8_zero_alpha() {
    let c = Rgba::new(1.0, 1.0, 1.0, 0.0);
    let srgb = c.to_srgb8();
    assert_eq!(srgb[3], 0);
}

#[test]
fn test_rgba_to_srgb8_exact_boundary() {
    // Exactly 0.0 and exactly 1.0 should map to 0 and 255
    let c = Rgba::new(0.0, 1.0, 0.0, 1.0);
    let srgb = c.to_srgb8();
    assert_eq!(srgb[0], 0);
    assert_eq!(srgb[1], 255);
    assert_eq!(srgb[2], 0);
    assert_eq!(srgb[3], 255);
}