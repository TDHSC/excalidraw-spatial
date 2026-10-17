pub mod alignment;
pub mod connectors;
pub mod overlap;

use crate::scene_graph::Scene;

pub const MIN_GAP: i32 = 40;
pub const NEAR_DIST: i32 = 12;
pub const ZONE_CHILD_AREA_RATIO: f64 = 0.25;
pub const ZONE_CONTAINED_AREA_RATIO: f64 = 0.65;
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

/// Usable fraction of a shape's box for its label. Ellipses and diamonds
/// inscribe a smaller rectangle than their bounding box.
pub fn text_factor(type_name: &str) -> f64 {
    match type_name {
        "ellipse" => 0.707,
        "diamond" => 0.5,
        _ => 1.0,
    }
}

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

const RULE_ORDER: &[&str] = &[
    "OVERLAP_COMPONENT",
    "CONTAINER_OVERFLOW",
    "ARROW_CROSSING_NODE",
    "ARROW_LABEL_COLLISION",
    "TEXT_OVERFLOW",
    "DANGLING_ARROW",
    "NEAR_MISS_ALIGNMENT",
    "UNEVEN_SPACING",
    "UNBOUND_TEXT",
];

fn rule_order(rule: &str) -> usize {
    RULE_ORDER
        .iter()
        .position(|&r| r == rule)
        .unwrap_or(usize::MAX)
}

pub fn sort_issues(issues: &mut [LintIssue]) {
    issues.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then_with(|| rule_order(a.rule).cmp(&rule_order(b.rule)))
            .then_with(|| a.targets.cmp(&b.targets))
    });
}

pub fn run_lint(scene: &Scene, file: &str) -> Vec<LintIssue> {
    let mut issues = overlap::overlap_component(scene, file);
    issues.extend(overlap::container_overflow(scene, file));
    issues.extend(connectors::arrow_crossing_node(scene));
    issues.extend(connectors::arrow_label_collision(scene, file));
    issues.extend(connectors::dangling_arrow(scene));
    issues.extend(alignment::text_overflow(scene, file));
    issues.extend(alignment::near_miss_alignment(scene, file));
    issues.extend(alignment::uneven_spacing(scene, file));
    issues.extend(scene.notes.iter().cloned());
    sort_issues(&mut issues);
    issues
}

pub fn inline_text(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn label_str(text: &str) -> String {
    let collapsed = inline_text(text);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(rule: &'static str, severity: Severity, targets: &[&str]) -> LintIssue {
        LintIssue {
            rule,
            severity,
            targets: targets.iter().map(|s| (*s).to_string()).collect(),
            message: String::new(),
            fix: None,
            command: None,
        }
    }

    #[test]
    fn inline_text_collapses_whitespace_without_truncating() {
        assert_eq!(inline_text("Cloudflare WAF\n& CDN"), "Cloudflare WAF & CDN");
        assert_eq!(inline_text("  TLS 1.3\n\tmTLS  "), "TLS 1.3 mTLS");
        assert_eq!(inline_text("   \n\t  "), "");
        let long = "abcdefghijklmnopqrstuvwxyz0123456789ABCD";
        assert_eq!(long.chars().count(), 40);
        assert_eq!(inline_text(long), long);
        assert_eq!(label_str(long), format!("{}…", &long[..30]));
        assert_eq!(label_str("Cloudflare WAF\n& CDN"), "Cloudflare WAF & CDN");
    }

    #[test]
    fn sort_issues_orders_by_severity_then_rule() {
        let mut issues = vec![
            issue("OVERLAP_COMPONENT", Severity::Info, &["z"]),
            issue("UNBOUND_TEXT", Severity::Error, &["a"]),
            issue("CONTAINER_OVERFLOW", Severity::Warn, &["m"]),
        ];
        sort_issues(&mut issues);
        assert_eq!(
            issues
                .iter()
                .map(|i| (i.severity, i.rule))
                .collect::<Vec<_>>(),
            vec![
                (Severity::Error, "UNBOUND_TEXT"),
                (Severity::Warn, "CONTAINER_OVERFLOW"),
                (Severity::Info, "OVERLAP_COMPONENT"),
            ]
        );
    }
}
