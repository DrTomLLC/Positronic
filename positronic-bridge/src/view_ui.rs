//! View / UI rendering.
//!
//! Status bar now shows the detected terminal mode (Pager, Continuation, etc.)
//! so the user knows when they're trapped and which key to press.

use crate::app::{AppState, PositronicApp};
use crate::detection::detect_terminal_mode;
use crate::helpers::{format_duration_short, short_path};
use crate::messages::Message;
use crate::renderer;

use iced::widget::{column, container, rich_text, row, scrollable, span, text, text_input};
use iced::{Color, Element, Font, Length, Theme};

// ────────────────────────────────────────────────────────────────
// Title & Theme
// ────────────────────────────────────────────────────────────────

pub fn title(app: &PositronicApp) -> String {
    match &app.state {
        AppState::Booting => "Positronic /// Booting...".into(),
        AppState::Active => "Positronic /// Data Surface".into(),
        AppState::Error(e) => format!("Positronic /// ERROR: {}", &e[..e.len().min(60)]),
    }
}

pub fn theme(_: &PositronicApp) -> Theme {
    Theme::Dark
}

// ────────────────────────────────────────────────────────────────
// View
// ────────────────────────────────────────────────────────────────

pub fn view(app: &PositronicApp) -> Element<'_, Message> {
    let mut layout = column![]
        .spacing(0)
        .padding(0)
        .width(Length::Fill)
        .height(Length::Fill);

    // ── Boot / Error messages ──
    match &app.state {
        AppState::Booting => {
            layout = layout.push(
                container(
                    text("⏳ Booting engine...")
                        .font(Font::MONOSPACE)
                        .size(14),
                )
                    .padding(10),
            );
        }
        AppState::Error(e) => {
            layout = layout.push(
                container(
                    text(format!("❌ {}", e))
                        .font(Font::MONOSPACE)
                        .size(14)
                        .color(Color::from_rgb(1.0, 0.4, 0.4)),
                )
                    .padding(10),
            );
        }
        AppState::Active => {}
    }

    // ── Colored output area ──
    let mut all_spans: Vec<iced::widget::text::Span<'static>> = Vec::new();

    if let Some(ref snapshot) = app.last_snapshot {
        all_spans.extend(renderer::snapshot_to_spans(snapshot, app.theme_name));
    } else if !app.direct_output.is_empty() {
        all_spans.extend(renderer::direct_to_spans(&app.direct_output));
    }

    if all_spans.is_empty() {
        all_spans.push(span("").color(Color::from_rgb(0.5, 0.5, 0.5)));
    }

    let terminal_display = rich_text(all_spans).font(Font::MONOSPACE).size(14);

    let scrollable_output = scrollable(
        container(terminal_display)
            .padding([5, 10])
            .width(Length::Fill),
    )
        .anchor_bottom()
        .height(Length::Fill)
        .width(Length::Fill);

    layout = layout.push(scrollable_output);

    // ── Status bar ──
    let uptime_secs = app.boot_instant.elapsed().as_secs() as i64;
    let uptime_str = format_duration_short(uptime_secs);
    let short_cwd = short_path(&app.cwd);

    // Tab hint
    let tab_hint = if let Some(ref state) = app.tab_state {
        format!(
            "  │  Tab {}/{}",
            state
                .completions
                .iter()
                .position(|c| c == state.current())
                .map(|i| i + 1)
                .unwrap_or(1),
            state.len()
        )
    } else {
        String::new()
    };

    // Terminal mode indicator (pager-trap bugfix)
    let mode_hint = if let Some(ref snapshot) = app.last_snapshot {
        let mode = detect_terminal_mode(snapshot);
        let label = mode.label();
        if label.is_empty() {
            String::new()
        } else {
            format!("  │  {}", label)
        }
    } else {
        String::new()
    };

    let status_text = format!(
        " ⚡ {} cmd  │  ⏱ {}  │  📂 {}  │  🎨 {}{}{}  │  Positronic v0.2.0",
        app.session_cmd_count,
        uptime_str,
        short_cwd,
        app.theme_name.label(),
        tab_hint,
        mode_hint,
    );

    let status_bar = container(
        text(status_text)
            .font(Font::MONOSPACE)
            .size(12)
            .color(Color::from_rgb(0.5, 0.55, 0.6)),
    )
        .width(Length::Fill)
        .padding([3.0, 12.0])
        .style(status_bar_style);

    layout = layout.push(status_bar);

    // ── Input bar ──
    let input = text_input("Type a command… (!help for commands)", &app.input)
        .font(Font::MONOSPACE)
        .size(14)
        .padding(10)
        .on_input(Message::InputChanged)
        .on_submit(Message::InputSent);

    layout = layout.push(container(row![input].width(Length::Fill)).padding([10, 10]));

    layout.into()
}

// ────────────────────────────────────────────────────────────────
// Status bar styling
// ────────────────────────────────────────────────────────────────

fn status_bar_style(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(iced::Background::Color(Color::from_rgba(
            0.08, 0.09, 0.1, 1.0,
        ))),
        border: iced::Border {
            color: Color::from_rgb(0.2, 0.22, 0.25),
            width: 1.0,
            radius: 0.0.into(),
        },
        ..iced::widget::container::Style::default()
    }
}