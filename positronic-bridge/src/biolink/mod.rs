// positronic-bridge/src/biolink/mod.rs
//
// BioLink Module — Voice and Accessibility interfaces for Positronic.
// Handles screen reader support, text-to-speech event queuing,
// and accessibility state management.

use std::collections::VecDeque;

/// Accessibility configuration
#[derive(Debug, Clone)]
pub struct AccessibilityConfig {
    /// Enable screen reader compatible output
    pub screen_reader_enabled: bool,
    /// Enable text-to-speech announcements
    pub tts_enabled: bool,
    /// Enable high contrast mode
    pub high_contrast: bool,
    /// Enable dyslexia-friendly font
    pub dyslexia_font: bool,
    /// Font scale multiplier (1.0 = default)
    pub font_scale: f32,
}

impl Default for AccessibilityConfig {
    fn default() -> Self {
        Self {
            screen_reader_enabled: false,
            tts_enabled: false,
            high_contrast: false,
            dyslexia_font: false,
            font_scale: 1.0,
        }
    }
}

/// Events that should be announced to the user via screen reader or TTS
#[derive(Debug, Clone, PartialEq)]
pub enum BioLinkEvent {
    /// A command finished executing
    CommandComplete { command: String, exit_code: i32 },
    /// Long-running job finished
    JobFinished { description: String, success: bool },
    /// Error occurred
    ErrorOccurred(String),
    /// Peer activity (from Hive)
    PeerActivity(String),
    /// Hardware event
    DeviceEvent(String),
    /// Custom announcement
    Announcement(String),
}

impl BioLinkEvent {
    /// Convert the event to a screen-reader-friendly text string.
    pub fn to_screen_reader_text(&self) -> String {
        match self {
            BioLinkEvent::CommandComplete { command, exit_code } => {
                if *exit_code == 0 {
                    format!("Command succeeded: {}", command)
                } else {
                    format!("Command failed with code {}: {}", exit_code, command)
                }
            }
            BioLinkEvent::JobFinished { description, success } => {
                if *success {
                    format!("Job complete: {}", description)
                } else {
                    format!("Job failed: {}", description)
                }
            }
            BioLinkEvent::ErrorOccurred(msg) => format!("Error: {}", msg),
            BioLinkEvent::PeerActivity(msg) => format!("Peer: {}", msg),
            BioLinkEvent::DeviceEvent(msg) => format!("Device: {}", msg),
            BioLinkEvent::Announcement(msg) => msg.clone(),
        }
    }

    /// Priority level for announcement ordering (lower = higher priority)
    pub fn priority(&self) -> u8 {
        match self {
            BioLinkEvent::ErrorOccurred(_) => 0,
            BioLinkEvent::CommandComplete { .. } => 1,
            BioLinkEvent::JobFinished { .. } => 2,
            BioLinkEvent::DeviceEvent(_) => 3,
            BioLinkEvent::PeerActivity(_) => 4,
            BioLinkEvent::Announcement(_) => 5,
        }
    }
}

/// The BioLink controller manages accessibility state and event queuing.
pub struct BioLink {
    pub config: AccessibilityConfig,
    /// Queue of pending announcements for TTS/screen reader
    announcement_queue: VecDeque<BioLinkEvent>,
    /// Maximum queue size to prevent memory growth
    max_queue_size: usize,
}

impl BioLink {
    pub fn new() -> Self {
        Self {
            config: AccessibilityConfig::default(),
            announcement_queue: VecDeque::new(),
            max_queue_size: 100,
        }
    }

    pub fn with_config(config: AccessibilityConfig) -> Self {
        Self {
            config,
            announcement_queue: VecDeque::new(),
            max_queue_size: 100,
        }
    }

    /// Push a new event to the announcement queue.
    /// Events are dropped if accessibility features are disabled.
    pub fn announce(&mut self, event: BioLinkEvent) {
        if !self.config.screen_reader_enabled && !self.config.tts_enabled {
            return;
        }

        // Evict oldest low-priority events if queue is full
        if self.announcement_queue.len() >= self.max_queue_size {
            self.announcement_queue.pop_front();
        }

        self.announcement_queue.push_back(event);
    }

    /// Drain the next pending announcement (FIFO order).
    pub fn next_announcement(&mut self) -> Option<String> {
        self.announcement_queue
            .pop_front()
            .map(|e| e.to_screen_reader_text())
    }

    /// Drain all pending announcements at once.
    pub fn drain_announcements(&mut self) -> Vec<String> {
        self.announcement_queue
            .drain(..)
            .map(|e| e.to_screen_reader_text())
            .collect()
    }

    /// How many announcements are pending.
    pub fn pending_count(&self) -> usize {
        self.announcement_queue.len()
    }

    /// Generate an ARIA-compatible label for a terminal block.
    pub fn block_label(command: &str, output: &str, exit_code: Option<i32>) -> String {
        let status = match exit_code {
            Some(0) => "succeeded",
            Some(code) => {
                return format!(
                    "Command {} failed with exit code {}. Output: {}",
                    command,
                    code,
                    truncate_for_reader(output, 200)
                );
            }
            None => "running",
        };
        format!(
            "Command {} {}. Output: {}",
            command,
            status,
            truncate_for_reader(output, 200)
        )
    }

    /// Format the current input buffer for screen reader feedback.
    pub fn describe_input(buffer: &str, cursor_pos: usize) -> String {
        if buffer.is_empty() {
            return "Input empty".to_string();
        }
        let at_cursor = buffer.chars().nth(cursor_pos).unwrap_or(' ');
        format!(
            "Input: {}. Cursor at position {}, on character {}",
            buffer, cursor_pos, at_cursor
        )
    }
}

impl Default for BioLink {
    fn default() -> Self {
        Self::new()
    }
}

/// Truncate text for screen reader output to avoid excessively long reads.
fn truncate_for_reader(text: &str, max_chars: usize) -> &str {
    if text.len() <= max_chars {
        text
    } else {
        // Find a safe char boundary
        let mut end = max_chars;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        &text[..end]
    }
}