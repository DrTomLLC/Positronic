use iced::widget::{container, text_input};
use iced::{Element, Length};

#[derive(Debug, Clone)]
pub struct InputEditor {
    pub value: String,
}

impl InputEditor {
    pub fn new() -> Self {
        Self {
            value: String::new(),
        }
    }

    pub fn view<'a, Message>(
        &self,
        on_change: fn(String) -> Message,
        on_submit: Message,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        container(
            text_input("Enter Command...", &self.value)
                .on_input(on_change)
                .on_submit(on_submit)
                .padding(12)
                .size(14)
                .font(iced::Font::MONOSPACE)
                .width(Length::Fill),
        )
        .padding(10)
        .width(Length::Fill)
        .into()
    }
}
