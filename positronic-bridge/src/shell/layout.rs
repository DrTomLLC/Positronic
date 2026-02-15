//! Layout computations.
//!
//! Calculates pixel regions for the terminal output area, status bar,
//! and input bar based on the viewport size.

/// Layout regions in pixels.
#[derive(Debug, Clone, Copy)]
pub struct Layout {
    /// Total viewport width.
    pub width: f32,
    /// Total viewport height.
    pub height: f32,

    /// Terminal output area.
    pub terminal_x: f32,
    pub terminal_y: f32,
    pub terminal_w: f32,
    pub terminal_h: f32,

    /// Status bar area.
    pub status_x: f32,
    pub status_y: f32,
    pub status_w: f32,
    pub status_h: f32,

    /// Input bar area.
    pub input_x: f32,
    pub input_y: f32,
    pub input_w: f32,
    pub input_h: f32,
}

/// Status bar height in pixels.
pub const STATUS_BAR_HEIGHT: f32 = 24.0;

/// Input bar height in pixels.
pub const INPUT_BAR_HEIGHT: f32 = 36.0;

/// Padding for the terminal content area.
pub const TERMINAL_PADDING: f32 = 10.0;

/// Compute the layout for the given viewport.
pub fn compute(viewport: [u32; 2]) -> Layout {
    let w = viewport[0] as f32;
    let h = viewport[1] as f32;

    let input_y = h - INPUT_BAR_HEIGHT;
    let status_y = input_y - STATUS_BAR_HEIGHT;
    let terminal_h = (status_y - TERMINAL_PADDING).max(0.0);

    Layout {
        width: w,
        height: h,

        terminal_x: 0.0,
        terminal_y: 0.0,
        terminal_w: w,
        terminal_h,

        status_x: 0.0,
        status_y,
        status_w: w,
        status_h: STATUS_BAR_HEIGHT,

        input_x: 0.0,
        input_y,
        input_w: w,
        input_h: INPUT_BAR_HEIGHT,
    }
}