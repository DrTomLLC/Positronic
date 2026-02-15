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
//
// All logic is pure Rust with zero UI dependencies. The UI layer
// (iced/wgpu) consumes these structures for rendering.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════
// Content Type Detection
// ═══════════════════════════════════════════════════════════════════

/// The detected content type of a block's output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContentType {
    /// Plain text (default).
    PlainText,
    /// Structured JSON data.
    Json,
    /// Comma-separated (or tab/pipe) tabular data.
    Csv,
    /// Inline image (Sixel, iTerm2, Kitty protocol).
    Image,
    /// Markdown-formatted text.
    Markdown,
    /// ANSI-colored text (already has escape sequences).
    AnsiStyled,
    /// Binary or unparseable content.
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
    /// Plain text lines.
    Text(String),
    /// Structured JSON (pretty-printed string + parsed depth info).
    Json(JsonContent),
    /// Tabular data with headers and rows.
    Table(DataFrame),
    /// Inline image metadata.
    Image(ImageMeta),
    /// Specification for an inline chart.
    Chart(ChartSpec),
    /// Markdown with structure hints.
    Markdown(MarkdownContent),
}

impl RichContent {
    /// Get the content type tag.
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

    /// Approximate rendered size in characters.
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

/// Parsed JSON content with formatting metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonContent {
    /// Pretty-printed JSON string.
    pub pretty: String,
    /// Original raw JSON string.
    pub raw: String,
    /// Maximum nesting depth.
    pub depth: usize,
    /// Number of top-level keys (if object) or elements (if array).
    pub top_level_count: usize,
    /// Whether the root is an array.
    pub is_array: bool,
}

impl JsonContent {
    /// Try to parse raw text as JSON and produce a JsonContent.
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

        Some(Self {
            pretty,
            raw: trimmed.to_string(),
            depth,
            top_level_count,
            is_array,
        })
    }
}

/// Calculate nesting depth of a JSON value.
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
// DataFrame (Tabular Data)
// ═══════════════════════════════════════════════════════════════════

/// The detected delimiter for tabular data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Delimiter {
    Comma,
    Tab,
    Pipe,
    Semicolon,
    Space,
}

impl Delimiter {
    /// Get the character for this delimiter.
    pub fn char(&self) -> char {
        match self {
            Delimiter::Comma => ',',
            Delimiter::Tab => '\t',
            Delimiter::Pipe => '|',
            Delimiter::Semicolon => ';',
            Delimiter::Space => ' ',
        }
    }

    /// Detect the most likely delimiter from the first few lines.
    pub fn detect(text: &str) -> Option<Self> {
        let lines: Vec<&str> = text.lines().take(5).collect();
        if lines.is_empty() {
            return None;
        }

        let candidates = [
            (Delimiter::Comma, ','),
            (Delimiter::Tab, '\t'),
            (Delimiter::Pipe, '|'),
            (Delimiter::Semicolon, ';'),
        ];

        let mut best: Option<(Delimiter, usize)> = None;

        for (delim, ch) in &candidates {
            let counts: Vec<usize> = lines.iter().map(|l| l.matches(*ch).count()).collect();

            // All lines must have at least 1 delimiter
            if counts.iter().all(|&c| c > 0) {
                // Consistency: all lines should have similar count
                let first = counts[0];
                let consistent = counts.iter().all(|&c| c == first);
                if consistent {
                    let score = first * lines.len(); // higher = more delimiters
                    if best.map(|(_, s)| score > s).unwrap_or(true) {
                        best = Some((*delim, score));
                    }
                }
            }
        }

        best.map(|(d, _)| d)
    }
}

impl fmt::Display for Delimiter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Delimiter::Comma => write!(f, ","),
            Delimiter::Tab => write!(f, "\\t"),
            Delimiter::Pipe => write!(f, "|"),
            Delimiter::Semicolon => write!(f, ";"),
            Delimiter::Space => write!(f, "space"),
        }
    }
}

/// A cell value in a DataFrame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CellValue {
    /// Textual content.
    Text(String),
    /// Integer number.
    Integer(i64),
    /// Floating-point number.
    Float(f64),
    /// Boolean value.
    Bool(bool),
    /// Empty / null cell.
    Empty,
}

impl CellValue {
    /// Parse a string cell into the most specific type.
    pub fn parse(s: &str) -> Self {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return CellValue::Empty;
        }
        // Try bool
        match trimmed.to_lowercase().as_str() {
            "true" | "yes" => return CellValue::Bool(true),
            "false" | "no" => return CellValue::Bool(false),
            _ => {}
        }
        // Try integer
        if let Ok(i) = trimmed.parse::<i64>() {
            return CellValue::Integer(i);
        }
        // Try float
        if let Ok(f) = trimmed.parse::<f64>() {
            return CellValue::Float(f);
        }
        CellValue::Text(trimmed.to_string())
    }

    /// Whether this cell is numeric (integer or float).
    pub fn is_numeric(&self) -> bool {
        matches!(self, CellValue::Integer(_) | CellValue::Float(_))
    }

    /// Convert to f64 if numeric.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            CellValue::Integer(i) => Some(*i as f64),
            CellValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Display as string.
    pub fn to_display(&self) -> String {
        match self {
            CellValue::Text(s) => s.clone(),
            CellValue::Integer(i) => i.to_string(),
            CellValue::Float(f) => format!("{:.6}", f).trim_end_matches('0').trim_end_matches('.').to_string(),
            CellValue::Bool(b) => b.to_string(),
            CellValue::Empty => String::new(),
        }
    }
}

impl fmt::Display for CellValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_display())
    }
}

/// A structured table of data with named columns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFrame {
    /// Column headers.
    pub headers: Vec<String>,
    /// Row data (each row is a Vec of CellValues, same length as headers).
    pub rows: Vec<Vec<CellValue>>,
    /// The delimiter that was detected/used.
    pub delimiter: Delimiter,
}

impl DataFrame {
    /// Create an empty DataFrame with the given headers.
    pub fn new(headers: Vec<String>, delimiter: Delimiter) -> Self {
        Self {
            headers,
            rows: Vec::new(),
            delimiter,
        }
    }

    /// Parse CSV/TSV text into a DataFrame.
    pub fn parse_csv(text: &str) -> Option<Self> {
        let delimiter = Delimiter::detect(text)?;
        Self::parse_with_delimiter(text, delimiter)
    }

    /// Parse text with a specific delimiter.
    pub fn parse_with_delimiter(text: &str, delimiter: Delimiter) -> Option<Self> {
        let mut lines = text.lines().peekable();
        let header_line = lines.next()?;
        let headers: Vec<String> = split_csv_line(header_line, delimiter.char())
            .into_iter()
            .map(|s| s.trim().to_string())
            .collect();

        if headers.is_empty() || headers.len() < 2 {
            return None; // Need at least 2 columns
        }

        let mut rows = Vec::new();
        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let cells: Vec<String> = split_csv_line(trimmed, delimiter.char());
            let mut row: Vec<CellValue> = cells.iter().map(|c| CellValue::parse(c)).collect();

            // Pad or truncate to match header count
            row.resize_with(headers.len(), || CellValue::Empty);
            row.truncate(headers.len());
            rows.push(row);
        }

        if rows.is_empty() {
            return None;
        }

        Some(Self { headers, rows, delimiter })
    }

    /// Number of rows.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Number of columns.
    pub fn col_count(&self) -> usize {
        self.headers.len()
    }

    /// Get a cell value by row and column index.
    pub fn get(&self, row: usize, col: usize) -> Option<&CellValue> {
        self.rows.get(row).and_then(|r| r.get(col))
    }

    /// Get a column by header name.
    pub fn column(&self, name: &str) -> Option<Vec<&CellValue>> {
        let idx = self.headers.iter().position(|h| h == name)?;
        Some(self.rows.iter().map(|r| &r[idx]).collect())
    }

    /// Get column index by name.
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.headers.iter().position(|h| h == name)
    }

    /// Whether a column is entirely numeric.
    pub fn is_column_numeric(&self, col: usize) -> bool {
        if col >= self.headers.len() {
            return false;
        }
        self.rows.iter().all(|r| {
            matches!(r.get(col), Some(CellValue::Integer(_) | CellValue::Float(_) | CellValue::Empty))
        })
    }

    /// Get numeric column indices (for chart suggestions).
    pub fn numeric_columns(&self) -> Vec<usize> {
        (0..self.col_count()).filter(|&i| self.is_column_numeric(i)).collect()
    }

    /// Compute basic statistics for a numeric column.
    pub fn column_stats(&self, col: usize) -> Option<ColumnStats> {
        if !self.is_column_numeric(col) {
            return None;
        }

        let values: Vec<f64> = self.rows.iter()
            .filter_map(|r| r.get(col).and_then(|c| c.as_f64()))
            .collect();

        if values.is_empty() {
            return None;
        }

        let count = values.len();
        let sum: f64 = values.iter().sum();
        let mean = sum / count as f64;
        let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        Some(ColumnStats { count, sum, mean, min, max })
    }

    /// Max width of each column (for text rendering).
    pub fn column_widths(&self) -> Vec<usize> {
        let mut widths: Vec<usize> = self.headers.iter().map(|h| h.len()).collect();
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(cell.to_display().len());
                }
            }
        }
        widths
    }

    /// Render the table as a formatted ASCII/Unicode table string.
    pub fn to_table_string(&self) -> String {
        let widths = self.column_widths();
        let mut out = String::new();

        // Header
        let header_line: Vec<String> = self.headers.iter().enumerate()
            .map(|(i, h)| format!("{:width$}", h, width = widths[i]))
            .collect();
        out.push_str("│ ");
        out.push_str(&header_line.join(" │ "));
        out.push_str(" │\n");

        // Separator
        let sep: Vec<String> = widths.iter().map(|w| "─".repeat(*w)).collect();
        out.push_str("├─");
        out.push_str(&sep.join("─┼─"));
        out.push_str("─┤\n");

        // Rows
        for row in &self.rows {
            let cells: Vec<String> = row.iter().enumerate()
                .map(|(i, c)| format!("{:width$}", c.to_display(), width = widths.get(i).copied().unwrap_or(0)))
                .collect();
            out.push_str("│ ");
            out.push_str(&cells.join(" │ "));
            out.push_str(" │\n");
        }

        out
    }

    /// Estimated character size for memory budgeting.
    pub fn estimated_char_size(&self) -> usize {
        let header_size: usize = self.headers.iter().map(|h| h.len()).sum();
        let row_size: usize = self.rows.iter()
            .map(|r| r.iter().map(|c| c.to_display().len()).sum::<usize>())
            .sum();
        header_size + row_size
    }
}

/// Split a CSV line respecting quoted fields.
fn split_csv_line(line: &str, delimiter: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '"' {
            if in_quotes {
                // Check for escaped quote ("")
                if chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                in_quotes = true;
            }
        } else if ch == delimiter && !in_quotes {
            fields.push(current.clone());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    fields.push(current);
    fields
}

/// Basic statistics for a numeric column.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnStats {
    pub count: usize,
    pub sum: f64,
    pub mean: f64,
    pub min: f64,
    pub max: f64,
}

impl fmt::Display for ColumnStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "count={}, min={:.2}, max={:.2}, mean={:.2}, sum={:.2}",
            self.count, self.min, self.max, self.mean, self.sum
        )
    }
}

// ═══════════════════════════════════════════════════════════════════
// Image Metadata
// ═══════════════════════════════════════════════════════════════════

/// Inline image protocol type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageProtocol {
    /// DEC Sixel graphics.
    Sixel,
    /// iTerm2 inline image protocol.
    ITerm2,
    /// Kitty graphics protocol.
    Kitty,
}

impl fmt::Display for ImageProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageProtocol::Sixel => write!(f, "sixel"),
            ImageProtocol::ITerm2 => write!(f, "iterm2"),
            ImageProtocol::Kitty => write!(f, "kitty"),
        }
    }
}

/// Image format for decoded images.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    Bmp,
    WebP,
    Svg,
    Unknown,
}

impl ImageFormat {
    /// Detect format from magic bytes.
    pub fn from_magic(bytes: &[u8]) -> Self {
        if bytes.len() < 4 {
            return ImageFormat::Unknown;
        }
        if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
            ImageFormat::Png
        } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            ImageFormat::Jpeg
        } else if bytes.starts_with(b"GIF") {
            ImageFormat::Gif
        } else if bytes.starts_with(b"BM") {
            ImageFormat::Bmp
        } else if bytes.starts_with(b"RIFF") && bytes.len() >= 12 && &bytes[8..12] == b"WEBP" {
            ImageFormat::WebP
        } else if bytes.starts_with(b"<?xml") || bytes.starts_with(b"<svg") {
            ImageFormat::Svg
        } else {
            ImageFormat::Unknown
        }
    }
}

impl fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageFormat::Png => write!(f, "PNG"),
            ImageFormat::Jpeg => write!(f, "JPEG"),
            ImageFormat::Gif => write!(f, "GIF"),
            ImageFormat::Bmp => write!(f, "BMP"),
            ImageFormat::WebP => write!(f, "WebP"),
            ImageFormat::Svg => write!(f, "SVG"),
            ImageFormat::Unknown => write!(f, "unknown"),
        }
    }
}

/// Metadata for an inline image detected in terminal output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageMeta {
    /// Image protocol (how it was transmitted).
    pub protocol: ImageProtocol,
    /// Detected image format.
    pub format: ImageFormat,
    /// Width in pixels (if known).
    pub width: Option<u32>,
    /// Height in pixels (if known).
    pub height: Option<u32>,
    /// Width in terminal cells (for layout).
    pub cell_width: Option<u32>,
    /// Height in terminal cells (for layout).
    pub cell_height: Option<u32>,
    /// File path or name (if available).
    pub source: Option<String>,
    /// Raw data size in bytes.
    pub data_size: usize,
}

impl ImageMeta {
    /// Create minimal image metadata.
    pub fn new(protocol: ImageProtocol, data_size: usize) -> Self {
        Self {
            protocol,
            format: ImageFormat::Unknown,
            width: None,
            height: None,
            cell_width: None,
            cell_height: None,
            source: None,
            data_size,
        }
    }

    /// Aspect ratio (width / height), if both dimensions are known.
    pub fn aspect_ratio(&self) -> Option<f64> {
        match (self.width, self.height) {
            (Some(w), Some(h)) if h > 0 => Some(w as f64 / h as f64),
            _ => None,
        }
    }

    /// Human-readable size string.
    pub fn size_display(&self) -> String {
        if self.data_size < 1024 {
            format!("{} B", self.data_size)
        } else if self.data_size < 1024 * 1024 {
            format!("{:.1} KB", self.data_size as f64 / 1024.0)
        } else {
            format!("{:.1} MB", self.data_size as f64 / (1024.0 * 1024.0))
        }
    }

    /// Summary line for accessibility / status.
    pub fn summary(&self) -> String {
        let dims = match (self.width, self.height) {
            (Some(w), Some(h)) => format!("{}×{}", w, h),
            _ => "unknown size".to_string(),
        };
        format!("{} image ({}), {}", self.format, dims, self.size_display())
    }
}

// ═══════════════════════════════════════════════════════════════════
// Chart Specification
// ═══════════════════════════════════════════════════════════════════

/// Type of chart to render.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartKind {
    Line,
    Bar,
    Scatter,
    Histogram,
    Area,
}

impl fmt::Display for ChartKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChartKind::Line => write!(f, "line"),
            ChartKind::Bar => write!(f, "bar"),
            ChartKind::Scatter => write!(f, "scatter"),
            ChartKind::Histogram => write!(f, "histogram"),
            ChartKind::Area => write!(f, "area"),
        }
    }
}

/// A specification for rendering an inline chart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartSpec {
    /// Chart type.
    pub kind: ChartKind,
    /// Chart title.
    pub title: Option<String>,
    /// X-axis label.
    pub x_label: Option<String>,
    /// Y-axis label.
    pub y_label: Option<String>,
    /// Data series (name → data points).
    pub series: Vec<ChartSeries>,
    /// Width in cells (suggested).
    pub width: u32,
    /// Height in cells (suggested).
    pub height: u32,
}

impl ChartSpec {
    /// Create a chart spec from DataFrame columns.
    pub fn from_dataframe(
        df: &DataFrame,
        x_col: usize,
        y_col: usize,
        kind: ChartKind,
    ) -> Option<Self> {
        if x_col >= df.col_count() || y_col >= df.col_count() {
            return None;
        }

        let points: Vec<(f64, f64)> = df.rows.iter()
            .filter_map(|row| {
                let x = row.get(x_col)?.as_f64()?;
                let y = row.get(y_col)?.as_f64()?;
                Some((x, y))
            })
            .collect();

        if points.is_empty() {
            return None;
        }

        Some(Self {
            kind,
            title: None,
            x_label: Some(df.headers[x_col].clone()),
            y_label: Some(df.headers[y_col].clone()),
            series: vec![ChartSeries {
                name: df.headers[y_col].clone(),
                points,
            }],
            width: 80,
            height: 20,
        })
    }

    /// Total number of data points across all series.
    pub fn total_points(&self) -> usize {
        self.series.iter().map(|s| s.points.len()).sum()
    }
}

/// A named data series for a chart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartSeries {
    /// Series name (for legend).
    pub name: String,
    /// Data points as (x, y) pairs.
    pub points: Vec<(f64, f64)>,
}

impl ChartSeries {
    /// X range of this series.
    pub fn x_range(&self) -> Option<(f64, f64)> {
        if self.points.is_empty() {
            return None;
        }
        let xs: Vec<f64> = self.points.iter().map(|p| p.0).collect();
        Some((
            xs.iter().cloned().fold(f64::INFINITY, f64::min),
            xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        ))
    }

    /// Y range of this series.
    pub fn y_range(&self) -> Option<(f64, f64)> {
        if self.points.is_empty() {
            return None;
        }
        let ys: Vec<f64> = self.points.iter().map(|p| p.1).collect();
        Some((
            ys.iter().cloned().fold(f64::INFINITY, f64::min),
            ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        ))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Markdown Content
// ═══════════════════════════════════════════════════════════════════

/// A structural element in Markdown content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkdownElement {
    /// Heading with level (1-6) and text.
    Heading(u8, String),
    /// A paragraph of text.
    Paragraph(String),
    /// A fenced code block with optional language tag.
    CodeBlock { language: Option<String>, code: String },
    /// A bullet list item.
    ListItem(String),
    /// A numbered list item.
    OrderedItem(usize, String),
    /// A blockquote line.
    Blockquote(String),
    /// A horizontal rule.
    HorizontalRule,
}

/// Parsed Markdown structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownContent {
    /// Raw source text.
    pub source: String,
    /// Detected structural elements (in order).
    pub elements: Vec<MarkdownElement>,
}

impl MarkdownContent {
    /// Parse Markdown text into structural elements.
    pub fn parse(text: &str) -> Self {
        let mut elements = Vec::new();
        let mut lines = text.lines().peekable();
        let mut in_code_block = false;
        let mut code_lang: Option<String> = None;
        let mut code_buf = String::new();

        while let Some(line) = lines.next() {
            // Fenced code block toggle
            if line.trim_start().starts_with("```") {
                if in_code_block {
                    elements.push(MarkdownElement::CodeBlock {
                        language: code_lang.take(),
                        code: code_buf.trim_end().to_string(),
                    });
                    code_buf.clear();
                    in_code_block = false;
                } else {
                    in_code_block = true;
                    let lang = line.trim_start().trim_start_matches('`').trim();
                    code_lang = if lang.is_empty() { None } else { Some(lang.to_string()) };
                }
                continue;
            }

            if in_code_block {
                if !code_buf.is_empty() {
                    code_buf.push('\n');
                }
                code_buf.push_str(line);
                continue;
            }

            let trimmed = line.trim();

            // Empty line
            if trimmed.is_empty() {
                continue;
            }

            // Horizontal rule
            if is_horizontal_rule(trimmed) {
                elements.push(MarkdownElement::HorizontalRule);
                continue;
            }

            // Headings
            if trimmed.starts_with('#') {
                let level = trimmed.chars().take_while(|&c| c == '#').count().min(6) as u8;
                let text = trimmed[level as usize..].trim_start().to_string();
                elements.push(MarkdownElement::Heading(level, text));
                continue;
            }

            // Blockquote
            if trimmed.starts_with('>') {
                let text = trimmed[1..].trim_start().to_string();
                elements.push(MarkdownElement::Blockquote(text));
                continue;
            }

            // Unordered list item
            if (trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ "))
                && trimmed.len() > 2
            {
                let text = trimmed[2..].to_string();
                elements.push(MarkdownElement::ListItem(text));
                continue;
            }

            // Ordered list item
            if let Some(item) = parse_ordered_item(trimmed) {
                elements.push(item);
                continue;
            }

            // Default: paragraph
            elements.push(MarkdownElement::Paragraph(trimmed.to_string()));
        }

        // Unclosed code block
        if in_code_block && !code_buf.is_empty() {
            elements.push(MarkdownElement::CodeBlock {
                language: code_lang,
                code: code_buf.trim_end().to_string(),
            });
        }

        Self {
            source: text.to_string(),
            elements,
        }
    }

    /// Count headings.
    pub fn heading_count(&self) -> usize {
        self.elements.iter().filter(|e| matches!(e, MarkdownElement::Heading(_, _))).count()
    }

    /// Count code blocks.
    pub fn code_block_count(&self) -> usize {
        self.elements.iter().filter(|e| matches!(e, MarkdownElement::CodeBlock { .. })).count()
    }
}

fn is_horizontal_rule(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.len() < 3 {
        return false;
    }
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

/// Heuristic content type detection for terminal output.
pub struct ContentDetector;

impl ContentDetector {
    /// Detect the content type of raw text output.
    pub fn detect(text: &str) -> ContentType {
        let trimmed = text.trim();

        if trimmed.is_empty() {
            return ContentType::PlainText;
        }

        // Binary detection: high ratio of non-printable characters
        if Self::looks_binary(trimmed) {
            return ContentType::Binary;
        }

        // Sixel/iTerm2/Kitty image protocol detection
        if Self::has_image_protocol(trimmed) {
            return ContentType::Image;
        }

        // JSON detection
        if Self::looks_like_json(trimmed) {
            return ContentType::Json;
        }

        // CSV/TSV detection (must come before Markdown to avoid false positives)
        if Self::looks_like_csv(trimmed) {
            return ContentType::Csv;
        }

        // Markdown detection
        if Self::looks_like_markdown(trimmed) {
            return ContentType::Markdown;
        }

        // ANSI escape sequence detection
        if Self::has_ansi_escapes(trimmed) {
            return ContentType::AnsiStyled;
        }

        ContentType::PlainText
    }

    /// Detect content type and parse into RichContent.
    pub fn detect_and_parse(text: &str) -> RichContent {
        let content_type = Self::detect(text);
        match content_type {
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
            ContentType::Markdown => {
                RichContent::Markdown(MarkdownContent::parse(text))
            }
            _ => RichContent::Text(text.to_string()),
        }
    }

    /// Check if text looks like binary data.
    pub fn looks_binary(text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        let total = text.len().min(1024); // Sample first 1KB
        let sample = &text[..total];
        let non_printable = sample.bytes()
            .filter(|b| *b < 0x20 && *b != b'\n' && *b != b'\r' && *b != b'\t' && *b != 0x1B)
            .count();
        // More than 10% non-printable = binary
        non_printable * 10 > total
    }

    /// Check for inline image protocol markers.
    pub fn has_image_protocol(text: &str) -> bool {
        // Sixel: ESC P (DCS) followed by sixel data
        if text.contains("\x1bP") || text.contains("\u{90}") {
            return true;
        }
        // iTerm2: ESC ] 1337 ; File=
        if text.contains("\x1b]1337;File=") {
            return true;
        }
        // Kitty: ESC _ G
        if text.contains("\x1b_G") {
            return true;
        }
        false
    }

    /// Check if text looks like JSON.
    pub fn looks_like_json(text: &str) -> bool {
        let trimmed = text.trim();
        // Must start with { or [
        if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
            return false;
        }
        // Must end with } or ]
        if !trimmed.ends_with('}') && !trimmed.ends_with(']') {
            return false;
        }
        // Quick validation: try to parse
        serde_json::from_str::<serde_json::Value>(trimmed).is_ok()
    }

    /// Check if text looks like CSV/TSV.
    pub fn looks_like_csv(text: &str) -> bool {
        let lines: Vec<&str> = text.lines().collect();
        if lines.len() < 2 {
            return false;
        }

        Delimiter::detect(text).is_some()
    }

    /// Check if text looks like Markdown.
    pub fn looks_like_markdown(text: &str) -> bool {
        let lines: Vec<&str> = text.lines().take(20).collect();
        if lines.is_empty() {
            return false;
        }

        let mut md_signals = 0;
        for line in &lines {
            let trimmed = line.trim();
            if trimmed.starts_with('#') { md_signals += 2; }
            if trimmed.starts_with("```") { md_signals += 3; }
            if trimmed.starts_with("- ") || trimmed.starts_with("* ") { md_signals += 1; }
            if trimmed.starts_with("> ") { md_signals += 1; }
            if is_horizontal_rule(trimmed) { md_signals += 1; }
            if trimmed.contains("**") || trimmed.contains("__") { md_signals += 1; }
            if trimmed.contains("`") && trimmed.len() > 3 { md_signals += 1; }
        }

        // Need enough signals relative to line count
        md_signals >= 2
    }

    /// Check for ANSI escape sequences.
    pub fn has_ansi_escapes(text: &str) -> bool {
        text.contains("\x1b[")
    }
}

// ═══════════════════════════════════════════════════════════════════
// Holodeck Manager
// ═══════════════════════════════════════════════════════════════════

/// A single rich content entry in the Holodeck.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolodeckEntry {
    /// Entry ID (monotonically increasing).
    pub id: u64,
    /// The detected content type.
    pub content_type: ContentType,
    /// The parsed rich content.
    pub content: RichContent,
    /// Original raw text (for fallback rendering).
    pub raw: String,
    /// Whether this entry has been viewed/rendered.
    pub rendered: bool,
    /// Optional metadata tags.
    pub tags: HashMap<String, String>,
}

/// The Holodeck Manager: manages a collection of rich content entries.
#[derive(Debug)]
pub struct HolodeckManager {
    /// All entries in order.
    entries: Vec<HolodeckEntry>,
    /// Next entry ID.
    next_id: u64,
    /// Maximum entries to keep in memory.
    max_entries: usize,
    /// Content detection enabled.
    auto_detect: bool,
}

impl HolodeckManager {
    /// Create a new Holodeck manager.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
            max_entries: 200,
            auto_detect: true,
        }
    }

    /// Create with custom capacity.
    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
            max_entries,
            auto_detect: true,
        }
    }

    /// Ingest raw text: auto-detect type, parse, and store.
    pub fn ingest(&mut self, raw: &str) -> u64 {
        let content = if self.auto_detect {
            ContentDetector::detect_and_parse(raw)
        } else {
            RichContent::Text(raw.to_string())
        };
        let content_type = ContentDetector::detect(raw);

        let id = self.next_id;
        self.next_id += 1;

        self.entries.push(HolodeckEntry {
            id,
            content_type,
            content,
            raw: raw.to_string(),
            rendered: false,
            tags: HashMap::new(),
        });

        self.enforce_limits();
        id
    }

    /// Ingest pre-parsed content.
    pub fn ingest_rich(&mut self, content: RichContent, raw: &str) -> u64 {
        let content_type = content.content_type();
        let id = self.next_id;
        self.next_id += 1;

        self.entries.push(HolodeckEntry {
            id,
            content_type,
            content,
            raw: raw.to_string(),
            rendered: false,
            tags: HashMap::new(),
        });

        self.enforce_limits();
        id
    }

    /// Get an entry by ID.
    pub fn get(&self, id: u64) -> Option<&HolodeckEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Get a mutable entry by ID.
    pub fn get_mut(&mut self, id: u64) -> Option<&mut HolodeckEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    /// Get the latest entry.
    pub fn latest(&self) -> Option<&HolodeckEntry> {
        self.entries.last()
    }

    /// Remove an entry by ID.
    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < before
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the manager is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Filter entries by content type.
    pub fn by_type(&self, ct: ContentType) -> Vec<&HolodeckEntry> {
        self.entries.iter().filter(|e| e.content_type == ct).collect()
    }

    /// Get all JSON entries.
    pub fn json_entries(&self) -> Vec<&HolodeckEntry> {
        self.by_type(ContentType::Json)
    }

    /// Get all table/CSV entries.
    pub fn table_entries(&self) -> Vec<&HolodeckEntry> {
        self.by_type(ContentType::Csv)
    }

    /// Get all image entries.
    pub fn image_entries(&self) -> Vec<&HolodeckEntry> {
        self.by_type(ContentType::Image)
    }

    /// Mark an entry as rendered.
    pub fn mark_rendered(&mut self, id: u64) {
        if let Some(entry) = self.get_mut(id) {
            entry.rendered = true;
        }
    }

    /// Get unrendered entries.
    pub fn unrendered(&self) -> Vec<&HolodeckEntry> {
        self.entries.iter().filter(|e| !e.rendered).collect()
    }

    /// Set a tag on an entry.
    pub fn set_tag(&mut self, id: u64, key: &str, value: &str) {
        if let Some(entry) = self.get_mut(id) {
            entry.tags.insert(key.to_string(), value.to_string());
        }
    }

    /// Enable/disable auto content detection.
    pub fn set_auto_detect(&mut self, enabled: bool) {
        self.auto_detect = enabled;
    }

    /// Whether auto-detect is enabled.
    pub fn auto_detect_enabled(&self) -> bool {
        self.auto_detect
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Summary statistics.
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

    /// Enforce max entry limits.
    fn enforce_limits(&mut self) {
        while self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }
}

impl Default for HolodeckManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary statistics for the Holodeck.
#[derive(Debug, Clone)]
pub struct HolodeckStats {
    pub total_entries: usize,
    pub type_counts: HashMap<ContentType, usize>,
    pub total_chars: usize,
    pub unrendered: usize,
}

impl fmt::Display for HolodeckStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} entries ({} unrendered), {} chars",
            self.total_entries, self.unrendered, self.total_chars
        )
    }
}

// ═══════════════════════════════════════════════════════════════════
// Inline unit tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_plain_text() {
        assert_eq!(ContentDetector::detect("hello world"), ContentType::PlainText);
    }

    #[test]
    fn test_detect_json() {
        assert_eq!(ContentDetector::detect(r#"{"key": "value"}"#), ContentType::Json);
    }

    #[test]
    fn test_detect_csv() {
        let csv = "name,age,city\nAlice,30,NYC\nBob,25,LA";
        assert_eq!(ContentDetector::detect(csv), ContentType::Csv);
    }

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