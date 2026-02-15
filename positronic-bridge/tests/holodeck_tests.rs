// positronic-bridge/tests/holodeck_tests.rs
//
// Integration tests for the Holodeck — Rich Media & Data Engine (Pillar VIII).
// Tests all public API: content detection, JSON/CSV/Markdown parsing,
// DataFrame operations, image metadata, chart specs, and HolodeckManager.

use positronic_bridge::holodeck::{
    CellValue, ChartKind, ChartSeries, ChartSpec, ColumnStats, ContentDetector, ContentType,
    DataFrame, Delimiter, HolodeckEntry, HolodeckManager, ImageFormat, ImageMeta, ImageProtocol,
    JsonContent, MarkdownContent, MarkdownElement, RichContent,
};

// ============================================================================
// ContentType Display
// ============================================================================

#[test]
fn test_content_type_display() {
    assert_eq!(format!("{}", ContentType::PlainText), "text");
    assert_eq!(format!("{}", ContentType::Json), "json");
    assert_eq!(format!("{}", ContentType::Csv), "csv");
    assert_eq!(format!("{}", ContentType::Image), "image");
    assert_eq!(format!("{}", ContentType::Markdown), "markdown");
    assert_eq!(format!("{}", ContentType::AnsiStyled), "ansi");
    assert_eq!(format!("{}", ContentType::Binary), "binary");
}

// ============================================================================
// Content Detection
// ============================================================================

#[test]
fn test_detect_empty() {
    assert_eq!(ContentDetector::detect(""), ContentType::PlainText);
}

#[test]
fn test_detect_plain_text() {
    assert_eq!(ContentDetector::detect("hello world"), ContentType::PlainText);
}

#[test]
fn test_detect_plain_multiline() {
    assert_eq!(
        ContentDetector::detect("line one\nline two\nline three"),
        ContentType::PlainText
    );
}

#[test]
fn test_detect_json_object() {
    assert_eq!(
        ContentDetector::detect(r#"{"name": "Alice", "age": 30}"#),
        ContentType::Json
    );
}

#[test]
fn test_detect_json_array() {
    assert_eq!(
        ContentDetector::detect(r#"[1, 2, 3, 4]"#),
        ContentType::Json
    );
}

#[test]
fn test_detect_json_with_whitespace() {
    let json = r#"
    {
        "key": "value"
    }
    "#;
    assert_eq!(ContentDetector::detect(json), ContentType::Json);
}

#[test]
fn test_detect_invalid_json() {
    // Looks like JSON but isn't valid
    assert_ne!(ContentDetector::detect("{invalid json}"), ContentType::Json);
}

#[test]
fn test_detect_csv() {
    let csv = "name,age,city\nAlice,30,NYC\nBob,25,LA";
    assert_eq!(ContentDetector::detect(csv), ContentType::Csv);
}

#[test]
fn test_detect_tsv() {
    let tsv = "name\tage\tcity\nAlice\t30\tNYC\nBob\t25\tLA";
    assert_eq!(ContentDetector::detect(tsv), ContentType::Csv);
}

#[test]
fn test_detect_pipe_delimited() {
    let pipe = "name|age|city\nAlice|30|NYC\nBob|25|LA";
    assert_eq!(ContentDetector::detect(pipe), ContentType::Csv);
}

#[test]
fn test_detect_csv_single_line_is_not_csv() {
    // Need at least 2 lines
    assert_ne!(ContentDetector::detect("name,age,city"), ContentType::Csv);
}

#[test]
fn test_detect_markdown() {
    let md = "# Title\n\nSome text\n\n## Section\n\n- item 1\n- item 2";
    assert_eq!(ContentDetector::detect(md), ContentType::Markdown);
}

#[test]
fn test_detect_markdown_code_block() {
    let md = "# Example\n\n```rust\nfn main() {}\n```\n\nDone.";
    assert_eq!(ContentDetector::detect(md), ContentType::Markdown);
}

#[test]
fn test_detect_ansi_escapes() {
    let ansi = "\x1b[31mRed text\x1b[0m and normal";
    assert_eq!(ContentDetector::detect(ansi), ContentType::AnsiStyled);
}

#[test]
fn test_detect_sixel_image() {
    assert!(ContentDetector::has_image_protocol("\x1bPsixel data here"));
}

#[test]
fn test_detect_iterm2_image() {
    assert!(ContentDetector::has_image_protocol("\x1b]1337;File=inline=1:data"));
}

#[test]
fn test_detect_kitty_image() {
    assert!(ContentDetector::has_image_protocol("\x1b_Gdata"));
}

#[test]
fn test_no_image_protocol() {
    assert!(!ContentDetector::has_image_protocol("regular text"));
}

#[test]
fn test_looks_binary() {
    let binary: String = (0..100).map(|i| (i % 16) as u8 as char).collect();
    assert!(ContentDetector::looks_binary(&binary));
}

#[test]
fn test_not_binary() {
    assert!(!ContentDetector::looks_binary("normal text with\nnewlines\tand tabs"));
}

#[test]
fn test_detect_and_parse_json() {
    let content = ContentDetector::detect_and_parse(r#"{"key": "value"}"#);
    assert!(matches!(content, RichContent::Json(_)));
}

#[test]
fn test_detect_and_parse_csv() {
    let csv = "a,b,c\n1,2,3\n4,5,6";
    let content = ContentDetector::detect_and_parse(csv);
    assert!(matches!(content, RichContent::Table(_)));
}

#[test]
fn test_detect_and_parse_markdown() {
    let md = "# Hello\n\nWorld\n\n## Sub\n\n- item";
    let content = ContentDetector::detect_and_parse(md);
    assert!(matches!(content, RichContent::Markdown(_)));
}

#[test]
fn test_detect_and_parse_plain() {
    let content = ContentDetector::detect_and_parse("hello world");
    assert!(matches!(content, RichContent::Text(_)));
}

// ============================================================================
// RichContent
// ============================================================================

#[test]
fn test_rich_content_type() {
    let text = RichContent::Text("hello".to_string());
    assert_eq!(text.content_type(), ContentType::PlainText);

    let json = RichContent::Json(JsonContent::parse(r#"{"a":1}"#).unwrap());
    assert_eq!(json.content_type(), ContentType::Json);
}

#[test]
fn test_rich_content_char_size() {
    let text = RichContent::Text("hello".to_string());
    assert_eq!(text.char_size(), 5);
}

// ============================================================================
// JSON Content
// ============================================================================

#[test]
fn test_json_parse_object() {
    let j = JsonContent::parse(r#"{"name": "Alice", "age": 30}"#).unwrap();
    assert_eq!(j.top_level_count, 2);
    assert!(!j.is_array);
    assert!(j.depth >= 1);
}

#[test]
fn test_json_parse_array() {
    let j = JsonContent::parse(r#"[1, 2, 3]"#).unwrap();
    assert_eq!(j.top_level_count, 3);
    assert!(j.is_array);
}

#[test]
fn test_json_parse_nested() {
    let j = JsonContent::parse(r#"{"a": {"b": {"c": 1}}}"#).unwrap();
    assert_eq!(j.depth, 3);
}

#[test]
fn test_json_parse_invalid() {
    assert!(JsonContent::parse("not json").is_none());
}

#[test]
fn test_json_parse_empty_object() {
    let j = JsonContent::parse("{}").unwrap();
    assert_eq!(j.top_level_count, 0);
    assert_eq!(j.depth, 1);
}

#[test]
fn test_json_parse_empty_array() {
    let j = JsonContent::parse("[]").unwrap();
    assert_eq!(j.top_level_count, 0);
    assert!(j.is_array);
}

#[test]
fn test_json_pretty_print() {
    let j = JsonContent::parse(r#"{"a":1}"#).unwrap();
    assert!(j.pretty.contains('\n')); // Pretty printed has newlines
    assert!(j.raw.len() < j.pretty.len());
}

// ============================================================================
// Delimiter Detection
// ============================================================================

#[test]
fn test_detect_comma_delimiter() {
    let csv = "a,b,c\n1,2,3\n4,5,6";
    assert_eq!(Delimiter::detect(csv), Some(Delimiter::Comma));
}

#[test]
fn test_detect_tab_delimiter() {
    let tsv = "a\tb\tc\n1\t2\t3\n4\t5\t6";
    assert_eq!(Delimiter::detect(tsv), Some(Delimiter::Tab));
}

#[test]
fn test_detect_pipe_delimiter() {
    let pipe = "a|b|c\n1|2|3\n4|5|6";
    assert_eq!(Delimiter::detect(pipe), Some(Delimiter::Pipe));
}

#[test]
fn test_detect_semicolon_delimiter() {
    let semi = "a;b;c\n1;2;3\n4;5;6";
    assert_eq!(Delimiter::detect(semi), Some(Delimiter::Semicolon));
}

#[test]
fn test_detect_no_delimiter() {
    assert_eq!(Delimiter::detect("hello world"), None);
}

#[test]
fn test_delimiter_char() {
    assert_eq!(Delimiter::Comma.char(), ',');
    assert_eq!(Delimiter::Tab.char(), '\t');
    assert_eq!(Delimiter::Pipe.char(), '|');
}

#[test]
fn test_delimiter_display() {
    assert_eq!(format!("{}", Delimiter::Comma), ",");
    assert_eq!(format!("{}", Delimiter::Tab), "\\t");
}

// ============================================================================
// CellValue
// ============================================================================

#[test]
fn test_cell_parse_integer() {
    assert_eq!(CellValue::parse("42"), CellValue::Integer(42));
    assert_eq!(CellValue::parse("-100"), CellValue::Integer(-100));
    assert_eq!(CellValue::parse("0"), CellValue::Integer(0));
}

#[test]
fn test_cell_parse_float() {
    assert_eq!(CellValue::parse("3.14"), CellValue::Float(3.14));
    assert_eq!(CellValue::parse("-0.5"), CellValue::Float(-0.5));
}

#[test]
fn test_cell_parse_bool() {
    assert_eq!(CellValue::parse("true"), CellValue::Bool(true));
    assert_eq!(CellValue::parse("false"), CellValue::Bool(false));
    assert_eq!(CellValue::parse("TRUE"), CellValue::Bool(true));
    assert_eq!(CellValue::parse("yes"), CellValue::Bool(true));
    assert_eq!(CellValue::parse("no"), CellValue::Bool(false));
}

#[test]
fn test_cell_parse_empty() {
    assert_eq!(CellValue::parse(""), CellValue::Empty);
    assert_eq!(CellValue::parse("   "), CellValue::Empty);
}

#[test]
fn test_cell_parse_text() {
    assert_eq!(CellValue::parse("hello"), CellValue::Text("hello".to_string()));
}

#[test]
fn test_cell_is_numeric() {
    assert!(CellValue::Integer(42).is_numeric());
    assert!(CellValue::Float(3.14).is_numeric());
    assert!(!CellValue::Text("hi".to_string()).is_numeric());
    assert!(!CellValue::Bool(true).is_numeric());
    assert!(!CellValue::Empty.is_numeric());
}

#[test]
fn test_cell_as_f64() {
    assert_eq!(CellValue::Integer(42).as_f64(), Some(42.0));
    assert_eq!(CellValue::Float(3.14).as_f64(), Some(3.14));
    assert_eq!(CellValue::Text("hi".to_string()).as_f64(), None);
}

#[test]
fn test_cell_display() {
    assert_eq!(CellValue::Integer(42).to_display(), "42");
    assert_eq!(CellValue::Bool(true).to_display(), "true");
    assert_eq!(CellValue::Empty.to_display(), "");
    assert_eq!(format!("{}", CellValue::Integer(99)), "99");
}

#[test]
fn test_cell_float_display_trims() {
    let val = CellValue::Float(3.0);
    let display = val.to_display();
    assert_eq!(display, "3");
}

// ============================================================================
// DataFrame Parsing
// ============================================================================

#[test]
fn test_dataframe_parse_csv() {
    let csv = "name,age,city\nAlice,30,NYC\nBob,25,LA";
    let df = DataFrame::parse_csv(csv).unwrap();
    assert_eq!(df.col_count(), 3);
    assert_eq!(df.row_count(), 2);
    assert_eq!(df.headers, vec!["name", "age", "city"]);
}

#[test]
fn test_dataframe_parse_tsv() {
    let tsv = "name\tage\nAlice\t30\nBob\t25";
    let df = DataFrame::parse_csv(tsv).unwrap();
    assert_eq!(df.col_count(), 2);
    assert_eq!(df.row_count(), 2);
    assert_eq!(df.delimiter, Delimiter::Tab);
}

#[test]
fn test_dataframe_parse_quoted_csv() {
    let csv = r#"name,bio
Alice,"She said ""hello"""
Bob,"Line 1, Line 2""#;
    let df = DataFrame::parse_csv(csv).unwrap();
    assert_eq!(df.row_count(), 2);
    // Alice's bio should have the inner quotes
    let bio = df.get(0, 1).unwrap();
    match bio {
        CellValue::Text(s) => assert!(s.contains("hello")),
        _ => panic!("Expected text"),
    }
}

#[test]
fn test_dataframe_parse_typed_cells() {
    let csv = "label,count,rate,active\nalpha,10,0.5,true\nbeta,20,1.5,false";
    let df = DataFrame::parse_csv(csv).unwrap();
    assert_eq!(df.get(0, 1), Some(&CellValue::Integer(10)));
    assert_eq!(df.get(0, 2), Some(&CellValue::Float(0.5)));
    assert_eq!(df.get(0, 3), Some(&CellValue::Bool(true)));
}

#[test]
fn test_dataframe_parse_no_data_rows() {
    let csv = "a,b,c\n";
    assert!(DataFrame::parse_csv(csv).is_none());
}

#[test]
fn test_dataframe_parse_single_column() {
    let csv = "name\nAlice\nBob";
    // Needs at least 2 columns
    assert!(DataFrame::parse_csv(csv).is_none());
}

#[test]
fn test_dataframe_column_by_name() {
    let csv = "x,y\n1,10\n2,20\n3,30";
    let df = DataFrame::parse_csv(csv).unwrap();
    let col = df.column("y").unwrap();
    assert_eq!(col.len(), 3);
    assert_eq!(col[0].as_f64(), Some(10.0));
}

#[test]
fn test_dataframe_column_index() {
    let csv = "a,b,c\n1,2,3";
    let df = DataFrame::parse_csv(csv).unwrap();
    assert_eq!(df.column_index("b"), Some(1));
    assert_eq!(df.column_index("z"), None);
}

#[test]
fn test_dataframe_is_column_numeric() {
    let csv = "name,value\nAlice,10\nBob,20";
    let df = DataFrame::parse_csv(csv).unwrap();
    assert!(!df.is_column_numeric(0)); // name
    assert!(df.is_column_numeric(1)); // value
}

#[test]
fn test_dataframe_numeric_columns() {
    let csv = "label,x,y\na,1,10\nb,2,20";
    let df = DataFrame::parse_csv(csv).unwrap();
    let numeric = df.numeric_columns();
    assert_eq!(numeric, vec![1, 2]);
}

#[test]
fn test_dataframe_column_stats() {
    let csv = "x,y\n1,10\n2,20\n3,30";
    let df = DataFrame::parse_csv(csv).unwrap();
    let stats = df.column_stats(1).unwrap();
    assert_eq!(stats.count, 3);
    assert!((stats.mean - 20.0).abs() < f64::EPSILON);
    assert!((stats.min - 10.0).abs() < f64::EPSILON);
    assert!((stats.max - 30.0).abs() < f64::EPSILON);
}

#[test]
fn test_dataframe_column_stats_non_numeric() {
    let csv = "name,age\nAlice,30\nBob,25";
    let df = DataFrame::parse_csv(csv).unwrap();
    assert!(df.column_stats(0).is_none());
}

#[test]
fn test_dataframe_column_widths() {
    let csv = "name,age\nAlice,30\nBobothy,5";
    let df = DataFrame::parse_csv(csv).unwrap();
    let widths = df.column_widths();
    assert_eq!(widths[0], 7); // "Bobothy"
    assert_eq!(widths[1], 3); // "age"
}

#[test]
fn test_dataframe_to_table_string() {
    let csv = "a,b\n1,2\n3,4";
    let df = DataFrame::parse_csv(csv).unwrap();
    let table = df.to_table_string();
    assert!(table.contains("│"));
    assert!(table.contains("─"));
    assert!(table.contains("a"));
    assert!(table.contains("1"));
}

#[test]
fn test_dataframe_new_empty() {
    let df = DataFrame::new(vec!["x".to_string(), "y".to_string()], Delimiter::Comma);
    assert_eq!(df.row_count(), 0);
    assert_eq!(df.col_count(), 2);
}

#[test]
fn test_dataframe_get_out_of_bounds() {
    let csv = "a,b\n1,2";
    let df = DataFrame::parse_csv(csv).unwrap();
    assert!(df.get(5, 0).is_none());
    assert!(df.get(0, 5).is_none());
}

#[test]
fn test_dataframe_mismatched_row_lengths() {
    let csv = "a,b,c\n1,2\n3,4,5,6";
    let df = DataFrame::parse_csv(csv).unwrap();
    // Short rows should be padded, long rows truncated
    assert_eq!(df.rows[0].len(), 3);
    assert_eq!(df.rows[1].len(), 3);
}

#[test]
fn test_column_stats_display() {
    let stats = ColumnStats {
        count: 3,
        sum: 60.0,
        mean: 20.0,
        min: 10.0,
        max: 30.0,
    };
    let display = format!("{}", stats);
    assert!(display.contains("count=3"));
    assert!(display.contains("mean=20.00"));
}

// ============================================================================
// Image Metadata
// ============================================================================

#[test]
fn test_image_meta_new() {
    let meta = ImageMeta::new(ImageProtocol::Sixel, 1024);
    assert_eq!(meta.protocol, ImageProtocol::Sixel);
    assert_eq!(meta.data_size, 1024);
    assert!(meta.width.is_none());
}

#[test]
fn test_image_meta_aspect_ratio() {
    let mut meta = ImageMeta::new(ImageProtocol::ITerm2, 0);
    meta.width = Some(800);
    meta.height = Some(400);
    assert!((meta.aspect_ratio().unwrap() - 2.0).abs() < f64::EPSILON);
}

#[test]
fn test_image_meta_aspect_ratio_unknown() {
    let meta = ImageMeta::new(ImageProtocol::Kitty, 0);
    assert!(meta.aspect_ratio().is_none());
}

#[test]
fn test_image_meta_size_display() {
    assert_eq!(ImageMeta::new(ImageProtocol::Sixel, 500).size_display(), "500 B");
    assert_eq!(ImageMeta::new(ImageProtocol::Sixel, 2048).size_display(), "2.0 KB");
    assert_eq!(
        ImageMeta::new(ImageProtocol::Sixel, 2 * 1024 * 1024).size_display(),
        "2.0 MB"
    );
}

#[test]
fn test_image_meta_summary() {
    let mut meta = ImageMeta::new(ImageProtocol::Sixel, 1024);
    meta.format = ImageFormat::Png;
    meta.width = Some(800);
    meta.height = Some(600);
    let summary = meta.summary();
    assert!(summary.contains("PNG"));
    assert!(summary.contains("800×600"));
    assert!(summary.contains("1.0 KB"));
}

#[test]
fn test_image_protocol_display() {
    assert_eq!(format!("{}", ImageProtocol::Sixel), "sixel");
    assert_eq!(format!("{}", ImageProtocol::ITerm2), "iterm2");
    assert_eq!(format!("{}", ImageProtocol::Kitty), "kitty");
}

#[test]
fn test_image_format_from_magic_png() {
    let bytes = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    assert_eq!(ImageFormat::from_magic(&bytes), ImageFormat::Png);
}

#[test]
fn test_image_format_from_magic_jpeg() {
    let bytes = [0xFF, 0xD8, 0xFF, 0xE0];
    assert_eq!(ImageFormat::from_magic(&bytes), ImageFormat::Jpeg);
}

#[test]
fn test_image_format_from_magic_gif() {
    assert_eq!(ImageFormat::from_magic(b"GIF89a"), ImageFormat::Gif);
}

#[test]
fn test_image_format_from_magic_bmp() {
    assert_eq!(ImageFormat::from_magic(b"BM\x00\x00"), ImageFormat::Bmp);
}

#[test]
fn test_image_format_from_magic_webp() {
    let bytes = b"RIFF\x00\x00\x00\x00WEBP";
    assert_eq!(ImageFormat::from_magic(bytes), ImageFormat::WebP);
}

#[test]
fn test_image_format_from_magic_svg() {
    assert_eq!(ImageFormat::from_magic(b"<svg xmlns"), ImageFormat::Svg);
    assert_eq!(ImageFormat::from_magic(b"<?xml ver"), ImageFormat::Svg);
}

#[test]
fn test_image_format_from_magic_unknown() {
    assert_eq!(ImageFormat::from_magic(b"random"), ImageFormat::Unknown);
}

#[test]
fn test_image_format_short_bytes() {
    assert_eq!(ImageFormat::from_magic(b"ab"), ImageFormat::Unknown);
}

#[test]
fn test_image_format_display() {
    assert_eq!(format!("{}", ImageFormat::Png), "PNG");
    assert_eq!(format!("{}", ImageFormat::Unknown), "unknown");
}

// ============================================================================
// Chart Specs
// ============================================================================

#[test]
fn test_chart_kind_display() {
    assert_eq!(format!("{}", ChartKind::Line), "line");
    assert_eq!(format!("{}", ChartKind::Bar), "bar");
    assert_eq!(format!("{}", ChartKind::Scatter), "scatter");
}

#[test]
fn test_chart_from_dataframe() {
    let csv = "x,y\n1,10\n2,20\n3,30";
    let df = DataFrame::parse_csv(csv).unwrap();
    let chart = ChartSpec::from_dataframe(&df, 0, 1, ChartKind::Line).unwrap();
    assert_eq!(chart.kind, ChartKind::Line);
    assert_eq!(chart.total_points(), 3);
    assert_eq!(chart.x_label, Some("x".to_string()));
    assert_eq!(chart.y_label, Some("y".to_string()));
}

#[test]
fn test_chart_from_dataframe_invalid_cols() {
    let csv = "a,b\n1,2";
    let df = DataFrame::parse_csv(csv).unwrap();
    assert!(ChartSpec::from_dataframe(&df, 0, 99, ChartKind::Bar).is_none());
}

#[test]
fn test_chart_from_dataframe_non_numeric() {
    let csv = "name,value\nAlice,10\nBob,20";
    let df = DataFrame::parse_csv(csv).unwrap();
    // Column 0 is text, so no numeric points
    let chart = ChartSpec::from_dataframe(&df, 0, 1, ChartKind::Line);
    assert!(chart.is_none());
}

#[test]
fn test_chart_series_ranges() {
    let series = ChartSeries {
        name: "test".to_string(),
        points: vec![(1.0, 10.0), (2.0, 20.0), (3.0, 30.0)],
    };
    let (xmin, xmax) = series.x_range().unwrap();
    assert!((xmin - 1.0).abs() < f64::EPSILON);
    assert!((xmax - 3.0).abs() < f64::EPSILON);

    let (ymin, ymax) = series.y_range().unwrap();
    assert!((ymin - 10.0).abs() < f64::EPSILON);
    assert!((ymax - 30.0).abs() < f64::EPSILON);
}

#[test]
fn test_chart_series_empty_ranges() {
    let series = ChartSeries {
        name: "empty".to_string(),
        points: vec![],
    };
    assert!(series.x_range().is_none());
    assert!(series.y_range().is_none());
}

// ============================================================================
// Markdown Parsing
// ============================================================================

#[test]
fn test_markdown_headings() {
    let md = "# Title\n## Section\n### Sub";
    let parsed = MarkdownContent::parse(md);
    assert_eq!(parsed.heading_count(), 3);
    assert_eq!(
        parsed.elements[0],
        MarkdownElement::Heading(1, "Title".to_string())
    );
    assert_eq!(
        parsed.elements[1],
        MarkdownElement::Heading(2, "Section".to_string())
    );
}

#[test]
fn test_markdown_code_block() {
    let md = "# Example\n\n```rust\nfn main() {}\n```";
    let parsed = MarkdownContent::parse(md);
    assert_eq!(parsed.code_block_count(), 1);
    match &parsed.elements[1] {
        MarkdownElement::CodeBlock { language, code } => {
            assert_eq!(language.as_deref(), Some("rust"));
            assert!(code.contains("fn main"));
        }
        _ => panic!("Expected CodeBlock"),
    }
}

#[test]
fn test_markdown_list_items() {
    let md = "- item one\n- item two\n* item three";
    let parsed = MarkdownContent::parse(md);
    assert_eq!(parsed.elements.len(), 3);
    assert_eq!(
        parsed.elements[0],
        MarkdownElement::ListItem("item one".to_string())
    );
}

#[test]
fn test_markdown_ordered_list() {
    let md = "1. First\n2. Second\n3. Third";
    let parsed = MarkdownContent::parse(md);
    assert_eq!(
        parsed.elements[0],
        MarkdownElement::OrderedItem(1, "First".to_string())
    );
    assert_eq!(
        parsed.elements[2],
        MarkdownElement::OrderedItem(3, "Third".to_string())
    );
}

#[test]
fn test_markdown_blockquote() {
    let md = "> A wise quote\n> Second line";
    let parsed = MarkdownContent::parse(md);
    assert_eq!(
        parsed.elements[0],
        MarkdownElement::Blockquote("A wise quote".to_string())
    );
}

#[test]
fn test_markdown_horizontal_rule() {
    let md = "Above\n---\nBelow";
    let parsed = MarkdownContent::parse(md);
    assert!(parsed
        .elements
        .iter()
        .any(|e| matches!(e, MarkdownElement::HorizontalRule)));
}

#[test]
fn test_markdown_paragraph() {
    let md = "Just a paragraph";
    let parsed = MarkdownContent::parse(md);
    assert_eq!(
        parsed.elements[0],
        MarkdownElement::Paragraph("Just a paragraph".to_string())
    );
}

#[test]
fn test_markdown_unclosed_code_block() {
    let md = "```python\nprint('hello')";
    let parsed = MarkdownContent::parse(md);
    assert_eq!(parsed.code_block_count(), 1);
}

#[test]
fn test_markdown_empty() {
    let parsed = MarkdownContent::parse("");
    assert!(parsed.elements.is_empty());
}

#[test]
fn test_markdown_source_preserved() {
    let md = "# Hello World";
    let parsed = MarkdownContent::parse(md);
    assert_eq!(parsed.source, md);
}

// ============================================================================
// HolodeckManager
// ============================================================================

#[test]
fn test_manager_new() {
    let mgr = HolodeckManager::new();
    assert!(mgr.is_empty());
    assert_eq!(mgr.len(), 0);
}

#[test]
fn test_manager_default() {
    let mgr = HolodeckManager::default();
    assert!(mgr.is_empty());
}

#[test]
fn test_manager_ingest_text() {
    let mut mgr = HolodeckManager::new();
    let id = mgr.ingest("hello world");
    assert_eq!(mgr.len(), 1);
    let entry = mgr.get(id).unwrap();
    assert_eq!(entry.content_type, ContentType::PlainText);
}

#[test]
fn test_manager_ingest_json() {
    let mut mgr = HolodeckManager::new();
    let id = mgr.ingest(r#"{"key": "value"}"#);
    let entry = mgr.get(id).unwrap();
    assert_eq!(entry.content_type, ContentType::Json);
    assert!(matches!(entry.content, RichContent::Json(_)));
}

#[test]
fn test_manager_ingest_csv() {
    let mut mgr = HolodeckManager::new();
    let id = mgr.ingest("a,b,c\n1,2,3\n4,5,6");
    let entry = mgr.get(id).unwrap();
    assert_eq!(entry.content_type, ContentType::Csv);
    assert!(matches!(entry.content, RichContent::Table(_)));
}

#[test]
fn test_manager_ingest_rich() {
    let mut mgr = HolodeckManager::new();
    let content = RichContent::Text("custom".to_string());
    let id = mgr.ingest_rich(content, "custom");
    let entry = mgr.get(id).unwrap();
    assert_eq!(entry.content_type, ContentType::PlainText);
}

#[test]
fn test_manager_monotonic_ids() {
    let mut mgr = HolodeckManager::new();
    let id1 = mgr.ingest("a");
    let id2 = mgr.ingest("b");
    let id3 = mgr.ingest("c");
    assert!(id1 < id2);
    assert!(id2 < id3);
}

#[test]
fn test_manager_get_nonexistent() {
    let mgr = HolodeckManager::new();
    assert!(mgr.get(999).is_none());
}

#[test]
fn test_manager_latest() {
    let mut mgr = HolodeckManager::new();
    mgr.ingest("first");
    mgr.ingest("second");
    mgr.ingest("third");
    assert_eq!(mgr.latest().unwrap().raw, "third");
}

#[test]
fn test_manager_latest_empty() {
    let mgr = HolodeckManager::new();
    assert!(mgr.latest().is_none());
}

#[test]
fn test_manager_remove() {
    let mut mgr = HolodeckManager::new();
    let id = mgr.ingest("test");
    assert!(mgr.remove(id));
    assert!(mgr.is_empty());
}

#[test]
fn test_manager_remove_nonexistent() {
    let mut mgr = HolodeckManager::new();
    assert!(!mgr.remove(999));
}

#[test]
fn test_manager_by_type() {
    let mut mgr = HolodeckManager::new();
    mgr.ingest("plain text");
    mgr.ingest(r#"{"json": true}"#);
    mgr.ingest("more plain");
    mgr.ingest("a,b\n1,2\n3,4");

    assert_eq!(mgr.by_type(ContentType::PlainText).len(), 2);
    assert_eq!(mgr.json_entries().len(), 1);
    assert_eq!(mgr.table_entries().len(), 1);
    assert_eq!(mgr.image_entries().len(), 0);
}

#[test]
fn test_manager_mark_rendered() {
    let mut mgr = HolodeckManager::new();
    let id = mgr.ingest("test");
    assert!(!mgr.get(id).unwrap().rendered);
    mgr.mark_rendered(id);
    assert!(mgr.get(id).unwrap().rendered);
}

#[test]
fn test_manager_unrendered() {
    let mut mgr = HolodeckManager::new();
    let id1 = mgr.ingest("one");
    let _id2 = mgr.ingest("two");
    mgr.mark_rendered(id1);
    assert_eq!(mgr.unrendered().len(), 1);
}

#[test]
fn test_manager_tags() {
    let mut mgr = HolodeckManager::new();
    let id = mgr.ingest("test");
    mgr.set_tag(id, "source", "stdout");
    assert_eq!(
        mgr.get(id).unwrap().tags.get("source").unwrap(),
        "stdout"
    );
}

#[test]
fn test_manager_auto_detect_toggle() {
    let mut mgr = HolodeckManager::new();
    assert!(mgr.auto_detect_enabled());
    mgr.set_auto_detect(false);
    assert!(!mgr.auto_detect_enabled());

    // With auto-detect off, JSON should be stored as text
    let id = mgr.ingest(r#"{"key": "value"}"#);
    assert!(matches!(mgr.get(id).unwrap().content, RichContent::Text(_)));
}

#[test]
fn test_manager_clear() {
    let mut mgr = HolodeckManager::new();
    mgr.ingest("a");
    mgr.ingest("b");
    mgr.clear();
    assert!(mgr.is_empty());
}

#[test]
fn test_manager_enforces_limits() {
    let mut mgr = HolodeckManager::with_capacity(5);
    for i in 0..10 {
        mgr.ingest(&format!("entry {}", i));
    }
    assert_eq!(mgr.len(), 5);
    // Latest entries should be preserved
    assert_eq!(mgr.latest().unwrap().raw, "entry 9");
}

#[test]
fn test_manager_stats() {
    let mut mgr = HolodeckManager::new();
    mgr.ingest("text one");
    mgr.ingest("text two");
    mgr.ingest(r#"{"json": true}"#);

    let stats = mgr.stats();
    assert_eq!(stats.total_entries, 3);
    assert_eq!(stats.unrendered, 3);
    assert!(stats.total_chars > 0);
    assert_eq!(stats.type_counts.get(&ContentType::PlainText), Some(&2));
    assert_eq!(stats.type_counts.get(&ContentType::Json), Some(&1));
}

#[test]
fn test_manager_stats_display() {
    let mut mgr = HolodeckManager::new();
    mgr.ingest("test");
    let stats = mgr.stats();
    let display = format!("{}", stats);
    assert!(display.contains("1 entries"));
}

#[test]
fn test_manager_raw_preserved() {
    let mut mgr = HolodeckManager::new();
    let raw = r#"{"key": "value"}"#;
    let id = mgr.ingest(raw);
    assert_eq!(mgr.get(id).unwrap().raw, raw);
}

// ============================================================================
// Serialization
// ============================================================================

#[test]
fn test_serialize_content_type() {
    let json = serde_json::to_string(&ContentType::Json).unwrap();
    let deserialized: ContentType = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, ContentType::Json);
}

#[test]
fn test_serialize_cell_value() {
    let cell = CellValue::Integer(42);
    let json = serde_json::to_string(&cell).unwrap();
    let deserialized: CellValue = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, CellValue::Integer(42));
}

#[test]
fn test_serialize_dataframe() {
    let csv = "a,b\n1,2\n3,4";
    let df = DataFrame::parse_csv(csv).unwrap();
    let json = serde_json::to_string(&df).unwrap();
    let deserialized: DataFrame = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.row_count(), 2);
    assert_eq!(deserialized.col_count(), 2);
}

#[test]
fn test_serialize_rich_content() {
    let content = RichContent::Text("hello".to_string());
    let json = serde_json::to_string(&content).unwrap();
    let deserialized: RichContent = serde_json::from_str(&json).unwrap();
    assert!(matches!(deserialized, RichContent::Text(_)));
}

#[test]
fn test_serialize_json_content() {
    let j = JsonContent::parse(r#"{"a": 1}"#).unwrap();
    let json = serde_json::to_string(&j).unwrap();
    let deserialized: JsonContent = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.top_level_count, 1);
}

#[test]
fn test_serialize_image_meta() {
    let mut meta = ImageMeta::new(ImageProtocol::Sixel, 1024);
    meta.format = ImageFormat::Png;
    meta.width = Some(800);
    let json = serde_json::to_string(&meta).unwrap();
    let deserialized: ImageMeta = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.protocol, ImageProtocol::Sixel);
    assert_eq!(deserialized.width, Some(800));
}

#[test]
fn test_serialize_chart_spec() {
    let spec = ChartSpec {
        kind: ChartKind::Line,
        title: Some("Test".to_string()),
        x_label: None,
        y_label: None,
        series: vec![ChartSeries {
            name: "data".to_string(),
            points: vec![(1.0, 2.0)],
        }],
        width: 80,
        height: 20,
    };
    let json = serde_json::to_string(&spec).unwrap();
    let deserialized: ChartSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.kind, ChartKind::Line);
}

#[test]
fn test_serialize_markdown() {
    let md = MarkdownContent::parse("# Hello\n\nWorld");
    let json = serde_json::to_string(&md).unwrap();
    let deserialized: MarkdownContent = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.heading_count(), 1);
}

// ============================================================================
// Edge Cases & Stress
// ============================================================================

#[test]
fn test_csv_with_empty_cells() {
    let csv = "a,b,c\n1,,3\n,2,";
    let df = DataFrame::parse_csv(csv).unwrap();
    assert_eq!(df.get(0, 1), Some(&CellValue::Empty));
    assert_eq!(df.get(1, 0), Some(&CellValue::Empty));
}

#[test]
fn test_csv_with_spaces_in_headers() {
    let csv = " name , age \nAlice,30";
    let df = DataFrame::parse_csv(csv).unwrap();
    assert_eq!(df.headers[0], "name");
    assert_eq!(df.headers[1], "age");
}

#[test]
fn test_json_deeply_nested() {
    let json = r#"{"a":{"b":{"c":{"d":{"e":1}}}}}"#;
    let j = JsonContent::parse(json).unwrap();
    assert_eq!(j.depth, 5);
}

#[test]
fn test_unicode_csv() {
    let csv = "名前,年齢\nアリス,30\nボブ,25";
    let df = DataFrame::parse_csv(csv).unwrap();
    assert_eq!(df.headers[0], "名前");
    assert_eq!(df.row_count(), 2);
}

#[test]
fn test_large_csv() {
    let mut csv = "x,y\n".to_string();
    for i in 0..1000 {
        csv.push_str(&format!("{},{}\n", i, i * 2));
    }
    let df = DataFrame::parse_csv(&csv).unwrap();
    assert_eq!(df.row_count(), 1000);
}

#[test]
fn test_manager_ingest_many() {
    let mut mgr = HolodeckManager::new();
    for i in 0..100 {
        mgr.ingest(&format!("entry {}", i));
    }
    assert_eq!(mgr.len(), 100);
}

#[test]
fn test_horizontal_rule_variants() {
    // All should be detected as horizontal rules
    let rules = ["---", "***", "___", "- - -", "* * *", "----------"];
    for rule in &rules {
        let md = format!("Above\n{}\nBelow", rule);
        let parsed = MarkdownContent::parse(&md);
        assert!(
            parsed.elements.iter().any(|e| matches!(e, MarkdownElement::HorizontalRule)),
            "Failed to detect horizontal rule: {}",
            rule
        );
    }
}

#[test]
fn test_markdown_complex() {
    let md = "\
# Title

Some intro paragraph.

## Section One

- bullet one
- bullet two

> A wise person said this

```python
def hello():
    print('hi')
```

1. First item
2. Second item

---

## Conclusion

Done.
";
    let parsed = MarkdownContent::parse(md);
    assert_eq!(parsed.heading_count(), 3);
    assert_eq!(parsed.code_block_count(), 1);
    assert!(parsed.elements.len() >= 10);
}