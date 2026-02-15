//! Application Shell — winit lifecycle, event dispatch, layout.
//!
//! The shell owns the winit event loop and the application state machine.
//! It translates platform events into application actions, manages the
//! engine lifecycle, and coordinates rendering.

pub(crate) mod app;
mod events;
pub(crate) mod layout;

pub use app::run;