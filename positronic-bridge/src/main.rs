//! Positronic Bridge — the iced GUI shell.
//!
//! The binary entry point. All logic lives in the library target.

use positronic_bridge::app::boot;
use positronic_bridge::keyboard::subscription;
use positronic_bridge::update::update;
use positronic_bridge::view_ui::{title, theme, view};

use iced::Settings;

pub fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .init();

    eprintln!("=== Positronic Starting ===");

    iced::application(boot, update, view)
        .title(title)
        .theme(theme)
        .subscription(subscription)
        .settings(Settings {
            antialiasing: true,
            ..Settings::default()
        })
        .run()
}