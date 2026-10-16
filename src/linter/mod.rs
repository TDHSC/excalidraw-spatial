pub mod alignment;
pub mod connectors;
pub mod overlap;

pub const MIN_GAP: i32 = 40;
pub const NEAR_DIST: i32 = 12;
pub const ZONE_CHILD_AREA_RATIO: f64 = 0.25;
pub const ZONE_TITLE_BAND: i32 = 40;
pub const CONTAINER_WARN_PADDING: i32 = 10;
pub const TEXT_PAD_X: i32 = 20;
pub const TEXT_PAD_Y: i32 = 16;
pub const NEAR_MISS_MAX: i32 = 8;
pub const UNEVEN_DIFF: i32 = 20;
pub const TIGHT_GAP: i32 = 30;
pub const BOUND_TEXT_PADDING: i32 = 5;
pub const BINDING_GAP_BASE: i32 = 5;

pub const SHAPE_TYPES: &[&str] = &[
    "rectangle",
    "ellipse",
    "diamond",
    "stickynote",
    "image",
    "embeddable",
    "iframe",
];

pub const TEXT_SHAPES: &[&str] = &["rectangle", "stickynote", "ellipse", "diamond"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warn,
    Info,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LintIssue {
    pub rule: &'static str,
    pub severity: Severity,
    pub targets: Vec<String>,
    pub message: String,
    pub fix: Option<String>,
    pub command: Option<String>,
}

// run_lint arrives with the lint tickets.

pub fn label_str(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() > 30 {
        let truncated: String = collapsed.chars().take(30).collect();
        format!("{truncated}…")
    } else {
        collapsed
    }
}

pub fn disp(id: &str, label: Option<&str>) -> String {
    match label.map(label_str) {
        Some(s) if !s.is_empty() => format!("{id} \"{s}\""),
        _ => id.to_string(),
    }
}
