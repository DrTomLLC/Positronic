use iced::widget::{container, text};
use iced::{Background, Border, Color, Element, Length, Theme};

#[derive(Clone, Debug)]
pub enum TerminalBlock {
    Command(String),        // User input echo
    StandardOutput(String), // Normal stdout
    ErrorOutput(String),    // stderr
}

impl TerminalBlock {
    pub fn view<'a, Message: 'a>(&'a self) -> Element<'a, Message> {
        match self {
            TerminalBlock::Command(cmd) => {
                let line = text(format!("➜ {}", cmd))
                    .font(iced::Font::MONOSPACE)
                    .size(14)
                    .color(Color::from_rgb(0.0, 1.0, 0.0));

                container(line)
                    .padding(5)
                    .width(Length::Fill)
                    .style(command_block_style)
                    .into()
            }
            TerminalBlock::StandardOutput(out) => {
                let line = text(out).font(iced::Font::MONOSPACE).size(14);

                container(line).padding(5).width(Length::Fill).into()
            }
            TerminalBlock::ErrorOutput(err) => {
                let line = text(err)
                    .font(iced::Font::MONOSPACE)
                    .size(14)
                    .color(Color::from_rgb(1.0, 0.4, 0.4));

                container(line)
                    .padding(5)
                    .width(Length::Fill)
                    .style(error_block_style)
                    .into()
            }
        }
    }
}

/// iced 0.14 container styling: plain function pointer.
fn command_block_style(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Color::from_rgba(0.1, 0.1, 0.1, 0.5))),
        border: Border {
            radius: 4.0.into(),
            ..Border::default()
        },
        ..iced::widget::container::Style::default()
    }
}

fn error_block_style(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Color::from_rgba(0.2, 0.05, 0.05, 0.5))),
        border: Border {
            radius: 4.0.into(),
            color: Color::from_rgb(0.5, 0.1, 0.1),
            width: 1.0,
        },
        ..iced::widget::container::Style::default()
    }
}