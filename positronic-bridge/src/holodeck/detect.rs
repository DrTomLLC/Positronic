use crate::holodeck::{ContentDetector, RichContent};

/// Convenience wrapper so the UI overlay can detect rich content from visible output.
pub fn detect_rich(text: &str) -> RichContent {
    ContentDetector::detect_and_parse(text)
}
