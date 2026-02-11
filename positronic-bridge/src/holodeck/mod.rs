use iced::widget::{column, container, text};
use iced::{Background, Border, Color, Element, Length, Theme};

#[derive(Clone, Debug)]
pub enum TerminalBlock {
    Command(String),        // User input echo
    StandardOutput(String), // Normal stdout
    ErrorOutput(String),    // stderr
}

impl TerminalBlock {
    pub fn view<'a, Message>(&self) -> Element<'a, Message>
    where
        Message: 'a,
    {
        let content = match self {
            TerminalBlock::Command(cmd) => {
                container(
                    text(format!("➜ {}", cmd))
                        .font(iced::Font::MONOSPACE)
                        .style(iced::theme::Text::Color(Color::from_rgb(0.0, 1.0, 0.0))) // Green prompt
                        .size(14),
                )
                .padding(5)
                .style(iced::theme::Container::Custom(Box::new(
                    BlockStyle::Command,
                )))
            }
            TerminalBlock::StandardOutput(out) => {
                container(text(out).font(iced::Font::MONOSPACE).size(14))
                    .padding(5)
                    .width(Length::Fill)
            }
            TerminalBlock::ErrorOutput(err) => {
                container(
                    text(err)
                        .font(iced::Font::MONOSPACE)
                        .style(iced::theme::Text::Color(Color::from_rgb(1.0, 0.4, 0.4))) // Red error
                        .size(14),
                )
                .padding(5)
                .width(Length::Fill)
            }
        };

        column![content].into()
    }
}

// Minimal styling for blocks
struct BlockStyle;

impl BlockStyle {
    // Helper types for container styling in new Iced versions
    const Command: Self = Self;
}

impl container::StyleSheet for BlockStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(Background::Color(Color::from_rgba(0.1, 0.1, 0.1, 0.5))),
            border: Border {
                radius: 4.0.into(),
                ..Border::default()
            },
            ..Default::default()
        }
    }
}
