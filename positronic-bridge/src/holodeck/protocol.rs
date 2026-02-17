use crate::holodeck::{DataFrame, ImageMeta, JsonContent, MarkdownContent, RichContent};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= (self.x + self.w) && py >= self.y && py <= (self.y + self.h)
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    CopyText(String),
    RunCommand(String),
    None,
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    Panel { title: String },
    Text { text: String },
    Button { label: String, action: Action },

    Json { title: String, pretty: String },
    Table { title: String, preview: String, copy_tsv: String },
    Image { title: String, meta: ImageMeta },
    Markdown { title: String, preview: String },
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: Uuid,
    pub kind: NodeKind,
    pub rect: Rect,
}

#[derive(Debug, Clone)]
pub struct HolodeckDoc {
    pub nodes: Vec<Node>,
}

impl HolodeckDoc {
    pub fn empty() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn from_rich(rich: &RichContent) -> Self {
        match rich {
            RichContent::Json(j) => doc_from_json(j),
            RichContent::Table(df) => doc_from_table(df),
            RichContent::Image(img) => doc_from_image(img),
            RichContent::Markdown(md) => doc_from_markdown(md),
            RichContent::Text(t) => doc_from_text(t),
            RichContent::Chart(_) => doc_from_text("📊 Chart: (renderer not wired yet)"),
        }
    }
}

fn doc_from_text(text: &str) -> HolodeckDoc {
    let mut nodes = Vec::new();

    nodes.push(Node {
        id: Uuid::new_v4(),
        kind: NodeKind::Panel {
            title: "Holodeck".into(),
        },
        rect: Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 },
    });

    let preview = truncate(text, 1200);
    nodes.push(Node {
        id: Uuid::new_v4(),
        kind: NodeKind::Text { text: preview },
        rect: Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 },
    });

    HolodeckDoc { nodes }
}

fn doc_from_json(j: &JsonContent) -> HolodeckDoc {
    let mut nodes = Vec::new();
    nodes.push(Node {
        id: Uuid::new_v4(),
        kind: NodeKind::Panel { title: "Holodeck · JSON".into() },
        rect: Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 },
    });

    nodes.push(Node {
        id: Uuid::new_v4(),
        kind: NodeKind::Button {
            label: "Copy JSON".into(),
            action: Action::CopyText(j.pretty.clone()),
        },
        rect: Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 },
    });

    nodes.push(Node {
        id: Uuid::new_v4(),
        kind: NodeKind::Json {
            title: "Preview".into(),
            pretty: truncate(&j.pretty, 2400),
        },
        rect: Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 },
    });

    HolodeckDoc { nodes }
}

fn doc_from_table(df: &DataFrame) -> HolodeckDoc {
    let mut nodes = Vec::new();
    nodes.push(Node {
        id: Uuid::new_v4(),
        kind: NodeKind::Panel { title: "Holodeck · Table".into() },
        rect: Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 },
    });

    let (preview, tsv) = table_preview_and_tsv(df, 18);
    nodes.push(Node {
        id: Uuid::new_v4(),
        kind: NodeKind::Button {
            label: "Copy TSV".into(),
            action: Action::CopyText(tsv.clone()),
        },
        rect: Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 },
    });

    nodes.push(Node {
        id: Uuid::new_v4(),
        kind: NodeKind::Table {
            title: "Preview".into(),
            preview,
            copy_tsv: tsv,
        },
        rect: Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 },
    });

    HolodeckDoc { nodes }
}

fn doc_from_image(img: &ImageMeta) -> HolodeckDoc {
    let mut nodes = Vec::new();
    nodes.push(Node {
        id: Uuid::new_v4(),
        kind: NodeKind::Panel { title: "Holodeck · Image".into() },
        rect: Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 },
    });

    nodes.push(Node {
        id: Uuid::new_v4(),
        kind: NodeKind::Image {
            title: "Image".into(),
            meta: img.clone(),
        },
        rect: Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 },
    });

    HolodeckDoc { nodes }
}

fn doc_from_markdown(md: &MarkdownContent) -> HolodeckDoc {
    let mut nodes = Vec::new();
    nodes.push(Node {
        id: Uuid::new_v4(),
        kind: NodeKind::Panel { title: "Holodeck · Markdown".into() },
        rect: Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 },
    });

    nodes.push(Node {
        id: Uuid::new_v4(),
        kind: NodeKind::Markdown {
            title: "Preview".into(),
            preview: truncate(&md.source, 2400),
        },
        rect: Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 },
    });

    HolodeckDoc { nodes }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut out = s[..max].to_string();
    out.push_str("\n…(truncated)…");
    out
}

fn table_preview_and_tsv(df: &DataFrame, max_rows: usize) -> (String, String) {
    // DataFrame in your holodeck supports headers + rows.
    let headers = df.headers.clone();
    let mut rows = df.rows.clone();

    if rows.len() > max_rows {
        rows.truncate(max_rows);
    }

    let mut tsv = String::new();
    tsv.push_str(&headers.join("\t"));
    tsv.push('\n');
    for r in &df.rows {
        tsv.push_str(&r.iter().map(|c| format!("{:?}", c)).collect::<Vec<_>>().join("\t"));
        tsv.push('\n');
    }

    // Preview as aligned-ish text
    let mut preview = String::new();
    preview.push_str(&headers.join(" | "));
    preview.push('\n');
    preview.push_str(&headers.iter().map(|_| "---").collect::<Vec<_>>().join("|"));
    preview.push('\n');
    for r in rows {
        preview.push_str(&r.iter().map(|c| format!("{:?}", c)).collect::<Vec<_>>().join(" | "));
        preview.push('\n');
    }

    (preview, tsv)
}
