// positronic-bridge/src/holodeck/mod.rs
//
// The Holodeck — Rich Media & Data Engine (Roadmap Pillar VIII).
//
// Transforms raw terminal output into structured, renderable content:
// - Content type detection (plain text, JSON, CSV, images, Sixel, Markdown)
// - CSV parsing into DataFrames (headers + typed rows)
// - JSON pretty-printing and structure analysis
// - Image metadata extraction (Sixel/iTerm2 inline protocols)
// - Chart specifications for inline plotting
// - Markdown structure detection

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════
// Content Type Detection
// ═══════════════════════════════════════════════════════════════════

/// The detected content type of a block's output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContentType {
    PlainText,
    Json,
    Csv,
    Image,
    Markdown,
    AnsiStyled,
    Binary,
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentType::PlainText => write!(f, "text"),
            ContentType::Json => write!(f, "json"),
            ContentType::Csv => write!(f, "csv"),
            ContentType::Image => write!(f, "image"),
            ContentType::Markdown => write!(f, "markdown"),
            ContentType::AnsiStyled => write!(f, "ansi"),
            ContentType::Binary => write!(f, "binary"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Rich Content
// ═══════════════════════════════════════════════════════════════════

/// Parsed rich content ready for rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RichContent {
    Text(String),
    Json(JsonContent),
    Table(DataFrame),
    Image(ImageMeta),
    Chart(ChartSpec),
    Markdown(MarkdownContent),
}

impl RichContent {
    pub fn content_type(&self) -> ContentType {
        match self {
            RichContent::Text(_) => ContentType::PlainText,
            RichContent::Json(_) => ContentType::Json,
            RichContent::Table(_) => ContentType::Csv,
            RichContent::Image(_) => ContentType::Image,
            RichContent::Chart(_) => ContentType::PlainText,
            RichContent::Markdown(_) => ContentType::Markdown,
        }
    }

    pub fn char_size(&self) -> usize {
        match self {
            RichContent::Text(s) => s.len(),
            RichContent::Json(j) => j.pretty.len(),
            RichContent::Table(df) => df.estimated_char_size(),
            RichContent::Image(_) => 0,
            RichContent::Chart(_) => 0,
            RichContent::Markdown(md) => md.source.len(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// JSON Content
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonContent {
    pub pretty: String,
    pub raw: String,
    pub depth: usize,
    pub top_level_count: usize,
    pub is_array: bool,
}

impl JsonContent {
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        let value: serde_json::Value = serde_json::from_str(trimmed).ok()?;
        let pretty = serde_json::to_string_pretty(&value).unwrap_or_else(|_| trimmed.to_string());
        let depth = json_depth(&value);
        let top_level_count = match &value {
            serde_json::Value::Object(map) => map.len(),
            serde_json::Value::Array(arr) => arr.len(),
            _ => 1,
        };
        let is_array = value.is_array();
        Some(Self { pretty, raw: trimmed.to_string(), depth, top_level_count, is_array })
    }
}

fn json_depth(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Object(map) => {
            1 + map.values().map(json_depth).max().unwrap_or(0)
        }
        serde_json::Value::Array(arr) => {
            1 + arr.iter().map(json_depth).max().unwrap_or(0)
        }
        _ => 0,
    }
}

// ═══════════════════════════════════════════════════════════════════
// DataFrame (CSV/TSV)
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFrame {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<CellValue>>,
    pub delimiter: char,
}

impl DataFrame {
    pub fn parse_csv(text: &str) -> Option<Self> {
        let delimiter = Delimiter::detect(text)?;
        let lines: Vec<&str> = text.lines().collect();
        if lines.len() < 2 {
            return None;
        }
        let headers: Vec<String> = lines[0].split(delimiter.ch).map(|s| s.trim().to_string()).collect();
        let mut rows = Vec::new();
        for line in &lines[1..] {
            let cells: Vec<CellValue> = line.split(delimiter.ch).map(|s| CellValue::parse(s.trim())).collect();
            rows.push(cells);
        }
        Some(Self { headers, rows, delimiter: delimiter.ch })
    }

    pub fn row_count(&self) -> usize { self.rows.len() }
    pub fn col_count(&self) -> usize { self.headers.len() }
    pub fn estimated_char_size(&self) -> usize {
        let header_size: usize = self.headers.iter().map(|h| h.len()).sum();
        let row_size: usize = self.rows.iter()
            .map(|row| row.iter().map(|c| c.display_len()).sum::<usize>())
            .sum();
        header_size + row_size
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CellValue {
    Text(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
    Empty,
}

impl CellValue {
    pub fn parse(s: &str) -> Self {
        let s = s.trim();
        if s.is_empty() { return CellValue::Empty; }
        if let Ok(i) = s.parse::<i64>() { return CellValue::Integer(i); }
        if let Ok(f) = s.parse::<f64>() { return CellValue::Float(f); }
        if s.eq_ignore_ascii_case("true") { return CellValue::Bool(true); }
        if s.eq_ignore_ascii_case("false") { return CellValue::Bool(false); }
        CellValue::Text(s.to_string())
    }

    pub fn display_len(&self) -> usize {
        match self {
            CellValue::Text(s) => s.len(),
            CellValue::Integer(i) => format!("{}", i).len(),
            CellValue::Float(f) => format!("{}", f).len(),
            CellValue::Bool(b) => if *b { 4 } else { 5 },
            CellValue::Empty => 0,
        }
    }
}

struct Delimiter {
    ch: char,
}

impl Delimiter {
    fn detect(text: &str) -> Option<Delimiter> {
        let lines: Vec<&str> = text.lines().take(5).collect();
        if lines.len() < 2 { return None; }
        for &ch in &[',', '\t', '|', ';'] {
            let counts: Vec<usize> = lines.iter().map(|l| l.matches(ch).count()).collect();
            if counts[0] > 0 && counts.iter().all(|&c| c == counts[0]) {
                return Some(Delimiter { ch });
            }
        }
        None
    }
}

// ═══════════════════════════════════════════════════════════════════
// Image / Chart (stubs for future rendering)
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageMeta {
    pub protocol: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub data_offset: usize,
    pub data_len: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartSpec {
    pub chart_type: String,
    pub title: Option<String>,
    pub x_label: Option<String>,
    pub y_label: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════
// Markdown
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownContent {
    pub source: String,
    pub elements: Vec<MarkdownElement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MarkdownElement {
    Heading(u8, String),
    Paragraph(String),
    CodeBlock { language: Option<String>, code: String },
    ListItem(String),
    OrderedItem(usize, String),
    Blockquote(String),
    HorizontalRule,
}

impl MarkdownContent {
    pub fn parse(text: &str) -> Self {
        let mut elements = Vec::new();
        let mut in_code_block = false;
        let mut code_lang: Option<String> = None;
        let mut code_buf = String::new();

        for line in text.lines() {
            if in_code_block {
                if line.trim_start().starts_with("```") {
                    elements.push(MarkdownElement::CodeBlock { language: code_lang.take(), code: code_buf.clone() });
                    code_buf.clear();
                    in_code_block = false;
                } else {
                    if !code_buf.is_empty() { code_buf.push('\n'); }
                    code_buf.push_str(line);
                }
                continue;
            }

            let trimmed = line.trim();

            if trimmed.starts_with("```") {
                in_code_block = true;
                let lang = trimmed.strip_prefix("```").unwrap().trim();
                code_lang = if lang.is_empty() { None } else { Some(lang.to_string()) };
                continue;
            }

            if is_horizontal_rule(trimmed) {
                elements.push(MarkdownElement::HorizontalRule);
            } else if let Some(rest) = trimmed.strip_prefix("# ") {
                elements.push(MarkdownElement::Heading(1, rest.to_string()));
            } else if let Some(rest) = trimmed.strip_prefix("## ") {
                elements.push(MarkdownElement::Heading(2, rest.to_string()));
            } else if let Some(rest) = trimmed.strip_prefix("### ") {
                elements.push(MarkdownElement::Heading(3, rest.to_string()));
            } else if let Some(rest) = trimmed.strip_prefix("> ") {
                elements.push(MarkdownElement::Blockquote(rest.to_string()));
            } else if let Some(rest) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
                elements.push(MarkdownElement::ListItem(rest.to_string()));
            } else if let Some(el) = parse_ordered_item(trimmed) {
                elements.push(el);
            } else if !trimmed.is_empty() {
                elements.push(MarkdownElement::Paragraph(trimmed.to_string()));
            }
        }

        // Close unclosed code block
        if in_code_block && !code_buf.is_empty() {
            elements.push(MarkdownElement::CodeBlock { language: code_lang, code: code_buf });
        }

        Self { source: text.to_string(), elements }
    }

    pub fn heading_count(&self) -> usize {
        self.elements.iter().filter(|e| matches!(e, MarkdownElement::Heading(..))).count()
    }

    pub fn code_block_count(&self) -> usize {
        self.elements.iter().filter(|e| matches!(e, MarkdownElement::CodeBlock { .. })).count()
    }
}

fn is_horizontal_rule(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.len() < 3 { return false; }
    let ch = trimmed.chars().next().unwrap();
    (ch == '-' || ch == '*' || ch == '_')
        && trimmed.chars().all(|c| c == ch || c == ' ')
        && trimmed.chars().filter(|&c| c == ch).count() >= 3
}

fn parse_ordered_item(s: &str) -> Option<MarkdownElement> {
    let dot_pos = s.find(". ")?;
    let num_str = &s[..dot_pos];
    let num: usize = num_str.parse().ok()?;
    let text = s[dot_pos + 2..].to_string();
    Some(MarkdownElement::OrderedItem(num, text))
}

// ═══════════════════════════════════════════════════════════════════
// Content Detector
// ═══════════════════════════════════════════════════════════════════

pub struct ContentDetector;

impl ContentDetector {
    pub fn detect(text: &str) -> ContentType {
        let trimmed = text.trim();
        if trimmed.is_empty() { return ContentType::PlainText; }
        if Self::looks_binary(trimmed) { return ContentType::Binary; }
        if Self::has_image_protocol(trimmed) { return ContentType::Image; }
        if Self::looks_like_json(trimmed) { return ContentType::Json; }
        if Self::looks_like_csv(trimmed) { return ContentType::Csv; }
        if Self::looks_like_markdown(trimmed) { return ContentType::Markdown; }
        if Self::has_ansi_escapes(trimmed) { return ContentType::AnsiStyled; }
        ContentType::PlainText
    }

    pub fn detect_and_parse(text: &str) -> RichContent {
        match Self::detect(text) {
            ContentType::Json => {
                if let Some(json) = JsonContent::parse(text) {
                    return RichContent::Json(json);
                }
                RichContent::Text(text.to_string())
            }
            ContentType::Csv => {
                if let Some(df) = DataFrame::parse_csv(text) {
                    return RichContent::Table(df);
                }
                RichContent::Text(text.to_string())
            }
            ContentType::Markdown => RichContent::Markdown(MarkdownContent::parse(text)),
            _ => RichContent::Text(text.to_string()),
        }
    }

    pub fn looks_binary(text: &str) -> bool {
        if text.is_empty() { return false; }
        let total = text.len().min(1024);
        let sample = &text[..total];
        let non_printable = sample.bytes()
            .filter(|b| *b < 0x20 && *b != b'\n' && *b != b'\r' && *b != b'\t' && *b != 0x1B)
            .count();
        non_printable * 10 > total
    }

    pub fn has_image_protocol(text: &str) -> bool {
        text.contains("\x1bP") || text.contains("\u{90}") || text.contains("\x1b]1337;File=") || text.contains("\x1b_G")
    }

    pub fn looks_like_json(text: &str) -> bool {
        let trimmed = text.trim();
        if !trimmed.starts_with('{') && !trimmed.starts_with('[') { return false; }
        if !trimmed.ends_with('}') && !trimmed.ends_with(']') { return false; }
        serde_json::from_str::<serde_json::Value>(trimmed).is_ok()
    }

    pub fn looks_like_csv(text: &str) -> bool {
        let lines: Vec<&str> = text.lines().collect();
        if lines.len() < 2 { return false; }
        Delimiter::detect(text).is_some()
    }

    pub fn looks_like_markdown(text: &str) -> bool {
        let lines: Vec<&str> = text.lines().take(20).collect();
        if lines.is_empty() { return false; }
        let mut md_signals = 0;
        for line in &lines {
            let trimmed = line.trim();
            if trimmed.starts_with('#') { md_signals += 2; }
            if trimmed.starts_with("```") { md_signals += 3; }
            if trimmed.starts_with("- ") || trimmed.starts_with("* ") { md_signals += 1; }
            if trimmed.starts_with("> ") { md_signals += 1; }
            if is_horizontal_rule(trimmed) { md_signals += 1; }
            if trimmed.contains("**") || trimmed.contains("__") { md_signals += 1; }
            if trimmed.contains('`') && trimmed.len() > 3 { md_signals += 1; }
        }
        md_signals >= 2
    }

    pub fn has_ansi_escapes(text: &str) -> bool {
        text.contains("\x1b[")
    }
}

// ═══════════════════════════════════════════════════════════════════
// Holodeck Manager
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolodeckEntry {
    pub id: u64,
    pub content_type: ContentType,
    pub content: RichContent,
    pub raw: String,
    pub rendered: bool,
    pub tags: HashMap<String, String>,
}

#[derive(Debug)]
pub struct HolodeckManager {
    entries: Vec<HolodeckEntry>,
    next_id: u64,
    max_entries: usize,
    auto_detect: bool,
}

impl HolodeckManager {
    pub fn new() -> Self {
        Self { entries: Vec::new(), next_id: 1, max_entries: 200, auto_detect: true }
    }

    pub fn with_capacity(max_entries: usize) -> Self {
        Self { entries: Vec::new(), next_id: 1, max_entries, auto_detect: true }
    }

    pub fn ingest(&mut self, raw: &str) -> u64 {
        let content = if self.auto_detect {
            ContentDetector::detect_and_parse(raw)
        } else {
            RichContent::Text(raw.to_string())
        };
        let content_type = ContentDetector::detect(raw);
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(HolodeckEntry { id, content_type, content, raw: raw.to_string(), rendered: false, tags: HashMap::new() });
        self.enforce_limits();
        id
    }

    pub fn ingest_rich(&mut self, content: RichContent, raw: &str) -> u64 {
        let content_type = content.content_type();
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(HolodeckEntry { id, content_type, content, raw: raw.to_string(), rendered: false, tags: HashMap::new() });
        self.enforce_limits();
        id
    }

    pub fn get(&self, id: u64) -> Option<&HolodeckEntry> { self.entries.iter().find(|e| e.id == id) }
    pub fn get_mut(&mut self, id: u64) -> Option<&mut HolodeckEntry> { self.entries.iter_mut().find(|e| e.id == id) }
    pub fn latest(&self) -> Option<&HolodeckEntry> { self.entries.last() }
    pub fn remove(&mut self, id: u64) -> bool { let before = self.entries.len(); self.entries.retain(|e| e.id != id); self.entries.len() < before }
    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn by_type(&self, ct: ContentType) -> Vec<&HolodeckEntry> { self.entries.iter().filter(|e| e.content_type == ct).collect() }
    pub fn json_entries(&self) -> Vec<&HolodeckEntry> { self.by_type(ContentType::Json) }
    pub fn table_entries(&self) -> Vec<&HolodeckEntry> { self.by_type(ContentType::Csv) }
    pub fn image_entries(&self) -> Vec<&HolodeckEntry> { self.by_type(ContentType::Image) }

    pub fn mark_rendered(&mut self, id: u64) { if let Some(e) = self.get_mut(id) { e.rendered = true; } }
    pub fn unrendered(&self) -> Vec<&HolodeckEntry> { self.entries.iter().filter(|e| !e.rendered).collect() }
    pub fn set_tag(&mut self, id: u64, key: &str, value: &str) { if let Some(e) = self.get_mut(id) { e.tags.insert(key.to_string(), value.to_string()); } }
    pub fn set_auto_detect(&mut self, enabled: bool) { self.auto_detect = enabled; }
    pub fn auto_detect_enabled(&self) -> bool { self.auto_detect }
    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn stats(&self) -> HolodeckStats {
        let mut type_counts = HashMap::new();
        let mut total_chars = 0;
        for entry in &self.entries {
            *type_counts.entry(entry.content_type).or_insert(0) += 1;
            total_chars += entry.content.char_size();
        }
        HolodeckStats {
            total_entries: self.entries.len(),
            type_counts,
            total_chars,
            unrendered: self.entries.iter().filter(|e| !e.rendered).count(),
        }
    }

    fn enforce_limits(&mut self) {
        while self.entries.len() > self.max_entries { self.entries.remove(0); }
    }
}

impl Default for HolodeckManager {
    fn default() -> Self { Self::new() }
}

#[derive(Debug, Clone)]
pub struct HolodeckStats {
    pub total_entries: usize,
    pub type_counts: HashMap<ContentType, usize>,
    pub total_chars: usize,
    pub unrendered: usize,
}

impl fmt::Display for HolodeckStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} entries ({} unrendered), {} chars", self.total_entries, self.unrendered, self.total_chars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_plain_text() { assert_eq!(ContentDetector::detect("hello world"), ContentType::PlainText); }

    #[test]
    fn test_detect_json() { assert_eq!(ContentDetector::detect(r#"{"key": "value"}"#), ContentType::Json); }

    #[test]
    fn test_detect_csv() { assert_eq!(ContentDetector::detect("name,age,city\nAlice,30,NYC\nBob,25,LA"), ContentType::Csv); }

    #[test]
    fn test_parse_json() {
        let j = JsonContent::parse(r#"{"a": 1, "b": [2, 3]}"#).unwrap();
        assert_eq!(j.top_level_count, 2);
        assert!(!j.is_array);
    }

    #[test]
    fn test_cell_value_parse() {
        assert_eq!(CellValue::parse("42"), CellValue::Integer(42));
        assert_eq!(CellValue::parse("3.14"), CellValue::Float(3.14));
        assert_eq!(CellValue::parse("true"), CellValue::Bool(true));
        assert_eq!(CellValue::parse(""), CellValue::Empty);
        assert_eq!(CellValue::parse("hello"), CellValue::Text("hello".to_string()));
    }
}