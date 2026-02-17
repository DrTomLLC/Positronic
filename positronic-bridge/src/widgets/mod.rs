//! Widget primitives for Holodeck (interactive rendering).
//!
//! These widgets render using the existing pipelines:
//! - QuadPipeline (rectangles/backgrounds)
//! - TextEngine  (glyphon monospace text)
//!
//! No "mode switch". Holodeck widgets appear automatically when content is detected
//! and disappear when raw/fullscreen contexts are active (Core ModeSnapshot).

pub mod button;
pub mod image;
pub mod plot;
pub mod table;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WidgetId(pub Uuid);

impl WidgetId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerKind {
    Down,
    Up,
    Move,
    Wheel { delta_y: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerEvent {
    pub x: f32,
    pub y: f32,
    pub kind: PointerKind,
}

#[derive(Debug, Clone)]
pub enum WidgetAction {
    None,
    /// Ask the app to send a command to the PTY.
    SendCommand(String),
    /// Copy this text to clipboard.
    CopyText(String),
}
