use iced::{Application, Settings};

pub fn main() -> iced::Result {
    PositronicApp::run(Settings::default())
}

struct PositronicApp {
    // State will go here
}

impl Application for PositronicApp {
    type Executor = iced::executor::Default;
    type Message = ();
    type Theme = iced::Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, iced::Command<Self::Message>) {
        (Self {}, iced::Command::none())
    }

    fn title(&self) -> String {
        String::from("Positronic")
    }

    fn update(&mut self, _message: Self::Message) -> iced::Command<Self::Message> {
        iced::Command::none()
    }

    fn view(&self) -> iced::Element<Self::Message> {
        iced::widget::text("Positronic Systems Online").into()
    }
}
