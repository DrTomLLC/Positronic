use regex::Regex;
use std::sync::OnceLock;

/// Sanitize sensitive information from strings before sending to AI.
pub struct PrivacyGuard;

static IP_REGEX: OnceLock<Regex> = OnceLock::new();
static EMAIL_REGEX: OnceLock<Regex> = OnceLock::new();
static API_KEY_REGEX: OnceLock<Regex> = OnceLock::new();

impl PrivacyGuard {
    /// Scrub PII from the input string.
    pub fn scrub(input: &str) -> String {
        let mut scrubbed = input.to_string();

        // 1. Scrub IPv4 Addresses
        let ip_re = IP_REGEX
            .get_or_init(|| Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").expect("Invalid IP Regex"));
        scrubbed = ip_re.replace_all(&scrubbed, "[REDACTED_IP]").to_string();

        // 2. Scrub Emails
        let email_re = EMAIL_REGEX.get_or_init(|| {
            Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b")
                .expect("Invalid Email Regex")
        });
        scrubbed = email_re
            .replace_all(&scrubbed, "[REDACTED_EMAIL]")
            .to_string();

        // 3. Scrub Generic API Keys (sk-..., gh_...)
        // This is a heuristic and not exhaustive.
        let key_re = API_KEY_REGEX.get_or_init(|| {
            Regex::new(r"\b(sk-[a-zA-Z0-9]{20,}|gh[pousr]_[a-zA-Z0-9]{20,})\b")
                .expect("Invalid Key Regex")
        });
        scrubbed = key_re.replace_all(&scrubbed, "[REDACTED_KEY]").to_string();

        scrubbed
    }
}