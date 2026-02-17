use std::borrow::Cow;

/// OSC events Positronic cares about (streaming, chunk-safe).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OscEvent {
    /// OSC 7;file://...  (best-effort decoded path)
    Cwd(String),

    /// OSC 133;A  prompt start
    PromptStart,

    /// OSC 133;B  command start (user hit enter / command is about to run)
    CommandStart,

    /// OSC 133;C  (optional marker some shells use; we keep it)
    CommandExecuted,

    /// OSC 133;D;<exit>
    CommandFinished { exit_code: Option<i32> },

    /// Anything else (payload string, without terminator)
    Unknown(String),
}

/// Streaming OSC parser (handles BEL or ST terminators).
#[derive(Debug, Default, Clone)]
pub struct OscParser {
    in_osc: bool,
    saw_esc: bool,
    buf: Vec<u8>,
}

impl OscParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed bytes; returns parsed OSC events.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<OscEvent> {
        let mut out = Vec::new();

        for &b in bytes {
            if !self.in_osc {
                // OSC introducer: ESC ]  or single-byte 0x9d
                if self.saw_esc {
                    self.saw_esc = false;
                    if b == b']' {
                        self.in_osc = true;
                        self.buf.clear();
                        continue;
                    }
                }

                if b == 0x1b {
                    self.saw_esc = true;
                } else if b == 0x9d {
                    self.in_osc = true;
                    self.buf.clear();
                }
                continue;
            }

            // We are inside OSC. Terminators:
            // - BEL (0x07)
            // - ST: ESC \   (0x1b 0x5c)
            if self.saw_esc {
                self.saw_esc = false;
                if b == b'\\' {
                    // ST
                    if let Some(ev) = self.finish_event() {
                        out.push(ev);
                    }
                    self.in_osc = false;
                    continue;
                }
                // Not ST; push literal ESC + this byte.
                self.buf.push(0x1b);
                self.buf.push(b);
                continue;
            }

            if b == 0x1b {
                self.saw_esc = true;
                continue;
            }

            if b == 0x07 {
                // BEL
                if let Some(ev) = self.finish_event() {
                    out.push(ev);
                }
                self.in_osc = false;
                continue;
            }

            self.buf.push(b);
        }

        out
    }

    fn finish_event(&mut self) -> Option<OscEvent> {
        if self.buf.is_empty() {
            return None;
        }

        let payload = String::from_utf8_lossy(&self.buf);
        Some(parse_osc_payload(payload))
    }
}

fn parse_osc_payload(payload: Cow<'_, str>) -> OscEvent {
    let s = payload.trim_matches('\0').trim();

    // OSC 7;file://...
    if let Some(rest) = s.strip_prefix("7;") {
        // Very forgiving decode: accept file://localhost/..., file://..., or raw paths
        return OscEvent::Cwd(decode_file_uri_to_path(rest));
    }

    // OSC 133;...
    if let Some(rest) = s.strip_prefix("133;") {
        // Common forms:
        // 133;A
        // 133;B
        // 133;C
        // 133;D;0
        if rest.starts_with('A') {
            return OscEvent::PromptStart;
        }
        if rest.starts_with('B') {
            return OscEvent::CommandStart;
        }
        if rest.starts_with('C') {
            return OscEvent::CommandExecuted;
        }
        if let Some(d) = rest.strip_prefix("D") {
            let exit_code = d
                .split(';')
                .nth(1)
                .and_then(|x| x.parse::<i32>().ok());
            return OscEvent::CommandFinished { exit_code };
        }
    }

    OscEvent::Unknown(s.to_string())
}

fn decode_file_uri_to_path(uri: &str) -> String {
    let u = uri.trim();

    // file://localhost/C:/...
    // file:///home/...
    if let Some(rest) = u.strip_prefix("file://localhost/") {
        return rest.replace("%20", " ").replace('/', "\\");
    }
    if let Some(rest) = u.strip_prefix("file:///") {
        // unix-ish
        return format!("/{}", rest.replace("%20", " "));
    }
    if let Some(rest) = u.strip_prefix("file://") {
        return rest.replace("%20", " ");
    }

    // fallback: return as-is
    u.to_string()
}
