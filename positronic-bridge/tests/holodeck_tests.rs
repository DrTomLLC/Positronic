// positronic-bridge/tests/holodeck_tests.rs
//
// Integration tests for the Holodeck rich content system (Pillar I).
// Tests content detection, parsing (JSON, CSV, Markdown), the
// HolodeckManager lifecycle, and entry type filtering.

use positronic_bridge::holodeck::{
    ContentType, HolodeckManager, MarkdownContent, MarkdownElement, RichContent,
};

// ============================================================================
// Content Detection
// ============================================================================

#[test]
fn test_detect_plain_text() {
    let mut mgr = HolodeckManager::new();
    let id = mgr.ingest("hello world");
    assert_eq!(mgr.get(id).unwrap().content_type, ContentType::PlainText);
}

#[test]
fn test_detect_json_object() {
    let mut mgr = HolodeckManager::new();
    let id = mgr.ingest(r#"{"key": "value"}"#);
    let entry = mgr.get(id).unwrap();
    assert_eq!(entry.content_type, ContentType::Json);
    assert!(matches!(entry.content, RichContent::Json(_)));
}

#[test]
fn test_detect_json_array() {
    let mut mgr = HolodeckManager::new();
    let id = mgr.ingest(r#"[1, 2, 3]"#);
    assert_eq!(mgr.get(id).unwrap().content_type, ContentType::Json);
}

#[test]
fn test_detect_csv() {
    let mut mgr = HolodeckManager::new();
    let id = mgr.ingest("a,b,c\n1,2,3\n4,5,6");
    let entry = mgr.get(id).unwrap();
    assert_eq!(entry.content_type, ContentType::Csv);
    assert!(matches!(entry.content, RichContent::Table(_)));
}

// ============================================================================
// HolodeckManager — Lifecycle
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

// ============================================================================
// HolodeckManager — Filtering
// ============================================================================

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

// ============================================================================
// HolodeckManager — Render Tracking
// ============================================================================

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

// ============================================================================
// HolodeckManager — Stats
// ============================================================================

#[test]
fn test_manager_stats() {
    let mut mgr = HolodeckManager::new();
    mgr.ingest("hello");
    mgr.ingest(r#"{"a": 1}"#);
    mgr.ingest("x,y\n1,2");

    let stats = mgr.stats();
    assert_eq!(stats.total_entries, 3);
    assert!(stats.total_chars > 0);
}

#[test]
fn test_manager_stats_display() {
    let mut mgr = HolodeckManager::new();
    mgr.ingest("hello");
    let display = format!("{}", mgr.stats());
    assert!(display.contains("1 entries"));
}

// ============================================================================
// Markdown Parsing
// ============================================================================

#[test]
fn test_markdown_heading() {
    let parsed = MarkdownContent::parse("# Title\n## Subtitle");
    assert_eq!(parsed.elements.len(), 2);
    assert_eq!(
        parsed.elements[0],
        MarkdownElement::Heading(1, "Title".to_string())
    );
    assert_eq!(
        parsed.elements[1],
        MarkdownElement::Heading(2, "Subtitle".to_string())
    );
}

#[test]
fn test_markdown_code_block() {
    let md = "```rust\nfn main() {}\n```";
    let parsed = MarkdownContent::parse(md);
    assert_eq!(parsed.code_block_count(), 1);
    match &parsed.elements[0] {
        MarkdownElement::CodeBlock { language, code } => {
            assert_eq!(language.as_deref(), Some("rust"));
            assert!(code.contains("fn main()"));
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