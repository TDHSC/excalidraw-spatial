pub mod ai_report;
pub mod ascii_map;

use std::collections::HashMap;

use crate::geometry::{Point, Rect};
use crate::linter::{LintIssue, Severity};
use crate::scene_graph::{Attach, Component, ComponentKind, Scene};
use crate::spatial::{self, Track};

pub fn format_inspect_json(file: &str, scene: &Scene, issues: &[LintIssue]) -> String {
    let report = InspectJson {
        file,
        canvas: ascii_map::canvas_rect(scene).map(round_rect),
        components: round_components(scene),
        containers: scene
            .containers
            .iter()
            .map(|container| {
                let kids = spatial::children(scene, &container.id);
                InspectContainer {
                    id: container.id.clone(),
                    kind: container.kind,
                    name: container.name.clone(),
                    rect: round_rect(container.rect),
                    parent: container.parent.clone(),
                    padding: if kids.is_empty() {
                        None
                    } else {
                        spatial::padding(scene, &container.id).map(|pad| InspectPadding {
                            top: round_i64(pad.top),
                            right: round_i64(pad.right),
                            bottom: round_i64(pad.bottom),
                            left: round_i64(pad.left),
                        })
                    },
                    children: kids.into_iter().map(|(id, _)| id).collect(),
                }
            })
            .collect(),
        connectors: scene
            .connectors
            .iter()
            .map(|connector| InspectConnector {
                id: connector.id.clone(),
                kind: connector.kind.clone(),
                elbowed: connector.elbowed,
                start: round_attach(&connector.start),
                end: round_attach(&connector.end),
                label: connector.label.clone(),
                points: connector.points.iter().copied().map(round_point).collect(),
            })
            .collect(),
        rows: track_json(scene, Axis::Row),
        columns: track_json(scene, Axis::Column),
        issues,
    };
    let mut s = serde_json::to_string_pretty(&report).expect("inspect json");
    s.push('\n');
    s
}

#[derive(serde::Serialize)]
struct InspectJson<'a> {
    file: &'a str,
    canvas: Option<InspectRect>,
    components: Vec<InspectComponent>,
    containers: Vec<InspectContainer>,
    connectors: Vec<InspectConnector>,
    rows: Vec<InspectTrack>,
    columns: Vec<InspectTrack>,
    issues: &'a [LintIssue],
}

#[derive(serde::Serialize)]
struct InspectRect {
    x1: i64,
    y1: i64,
    x2: i64,
    y2: i64,
}

#[derive(serde::Serialize)]
struct InspectPoint {
    x: i64,
    y: i64,
}

#[derive(serde::Serialize)]
struct InspectComponent {
    id: String,
    kind: ComponentKind,
    #[serde(rename = "type")]
    type_name: String,
    label: Option<String>,
    rect: InspectRect,
    center: InspectPoint,
    parent: Option<String>,
    neighbors: Option<InspectNeighbors>,
}

#[derive(serde::Serialize)]
struct InspectNeighbor {
    id: String,
    gap: i64,
}

#[derive(serde::Serialize)]
struct InspectNeighbors {
    left: Option<InspectNeighbor>,
    right: Option<InspectNeighbor>,
    top: Option<InspectNeighbor>,
    bottom: Option<InspectNeighbor>,
}

#[derive(serde::Serialize)]
struct InspectContainer {
    id: String,
    kind: crate::scene_graph::ContainerKind,
    name: Option<String>,
    rect: InspectRect,
    parent: Option<String>,
    padding: Option<InspectPadding>,
    children: Vec<String>,
}

#[derive(serde::Serialize)]
struct InspectPadding {
    top: i64,
    right: i64,
    bottom: i64,
    left: i64,
}

#[derive(serde::Serialize)]
struct InspectConnector {
    id: String,
    kind: String,
    elbowed: bool,
    start: InspectAttach,
    end: InspectAttach,
    label: Option<String>,
    points: Vec<[i64; 2]>,
}

#[derive(serde::Serialize)]
#[serde(tag = "attach", rename_all = "lowercase")]
enum InspectAttach {
    Bound { target: String, element: String },
    Near { target: String, distance: i64 },
    Free,
}

#[derive(serde::Serialize)]
struct InspectTrack {
    parent: Option<String>,
    members: Vec<String>,
    gaps: Vec<i64>,
    pos: i64,
}

fn round_i64(value: f64) -> i64 {
    if !value.is_finite() {
        0
    } else {
        value.round() as i64
    }
}

fn round_rect(rect: Rect) -> InspectRect {
    InspectRect {
        x1: round_i64(rect.x1),
        y1: round_i64(rect.y1),
        x2: round_i64(rect.x2),
        y2: round_i64(rect.y2),
    }
}

fn round_point(point: Point) -> [i64; 2] {
    [round_i64(point.x), round_i64(point.y)]
}

fn round_components(scene: &Scene) -> Vec<InspectComponent> {
    let siblings = sibling_index(scene);
    scene
        .components
        .iter()
        .map(|component| {
            let peers = siblings.get(&component.parent.clone().unwrap_or_default());
            round_component(component, peers.map(Vec::as_slice).unwrap_or(&[]))
        })
        .collect()
}

fn round_component(component: &Component, peers: &[&Component]) -> InspectComponent {
    let center = component.rect.center();
    let neighbors = if component.kind == ComponentKind::Text {
        None
    } else {
        let found = spatial::neighbors(component, peers);
        Some(InspectNeighbors {
            left: found.left.map(round_neighbor),
            right: found.right.map(round_neighbor),
            top: found.top.map(round_neighbor),
            bottom: found.bottom.map(round_neighbor),
        })
    };
    InspectComponent {
        id: component.id.clone(),
        kind: component.kind,
        type_name: component.type_name.clone(),
        label: component.label.clone(),
        rect: round_rect(component.rect),
        center: InspectPoint {
            x: round_i64(center.x),
            y: round_i64(center.y),
        },
        parent: component.parent.clone(),
        neighbors,
    }
}

fn round_neighbor(neighbor: spatial::Neighbor) -> InspectNeighbor {
    InspectNeighbor {
        id: neighbor.id,
        gap: round_i64(neighbor.gap),
    }
}

fn sibling_index(scene: &Scene) -> HashMap<String, Vec<&Component>> {
    let mut groups: HashMap<String, Vec<&Component>> = HashMap::new();
    for component in scene.nodes() {
        groups
            .entry(component.parent.clone().unwrap_or_default())
            .or_default()
            .push(component);
    }
    groups
}

fn round_attach(attach: &Attach) -> InspectAttach {
    match attach {
        Attach::Bound { target, element } => InspectAttach::Bound {
            target: target.clone(),
            element: element.clone(),
        },
        Attach::Near { target, distance } => InspectAttach::Near {
            target: target.clone(),
            distance: round_i64(*distance),
        },
        Attach::Free => InspectAttach::Free,
    }
}

enum Axis {
    Row,
    Column,
}

fn track_json(scene: &Scene, axis: Axis) -> Vec<InspectTrack> {
    let groups = sibling_groups(scene);
    let mut tracks = Vec::new();
    for components in groups {
        let mut found = match axis {
            Axis::Row => spatial::rows(&components),
            Axis::Column => spatial::columns(&components),
        };
        found.sort_by(|a, b| a.pos.total_cmp(&b.pos));
        tracks.extend(found.into_iter().map(round_track));
    }
    tracks
}

fn round_track(track: Track) -> InspectTrack {
    InspectTrack {
        parent: track.parent,
        members: track.members,
        gaps: track.gaps.into_iter().map(round_i64).collect(),
        pos: round_i64(track.pos),
    }
}

fn sibling_groups(scene: &Scene) -> Vec<Vec<&Component>> {
    let mut groups: HashMap<String, Vec<&Component>> = HashMap::new();
    for component in scene.nodes() {
        groups
            .entry(component.parent.clone().unwrap_or_default())
            .or_default()
            .push(component);
    }
    let mut containers: Vec<_> = scene.containers.iter().collect();
    containers.sort_by_key(|container| container.z);
    let mut ordered = Vec::new();
    if let Some(top) = groups.get("") {
        ordered.push(top.clone());
    }
    for container in containers {
        if let Some(children) = groups.get(&container.id) {
            ordered.push(children.clone());
        }
    }
    ordered
}

pub fn format_lint_text(issues: &[LintIssue]) -> String {
    let mut out = String::new();
    let summary = count_severities(issues);
    for issue in issues {
        let sev = match issue.severity {
            Severity::Error => "ERROR",
            Severity::Warn => "WARN ",
            Severity::Info => "INFO ",
        };
        out.push_str(sev);
        out.push(' ');
        out.push_str(issue.rule);
        out.push(' ');
        out.push_str(&issue.message);
        out.push('\n');
        if let Some(fix) = &issue.fix {
            out.push_str("      fix: ");
            out.push_str(fix);
            out.push('\n');
        }
        if let Some(command) = &issue.command {
            out.push_str("      run: ");
            out.push_str(command);
            out.push('\n');
        }
    }
    out.push_str(&format!(
        "errors: {}, warnings: {}, info: {}\n",
        summary.errors, summary.warnings, summary.info
    ));
    out
}

pub fn format_lint_json(file: &str, issues: &[LintIssue]) -> String {
    let report = LintJson {
        file,
        summary: count_severities(issues),
        issues,
    };
    let mut s = serde_json::to_string_pretty(&report).expect("lint json");
    s.push('\n');
    s
}

#[derive(serde::Serialize)]
struct LintJson<'a> {
    file: &'a str,
    summary: LintSummary,
    issues: &'a [LintIssue],
}

#[derive(serde::Serialize)]
struct LintSummary {
    errors: usize,
    warnings: usize,
    info: usize,
}

fn count_severities(issues: &[LintIssue]) -> LintSummary {
    let mut summary = LintSummary {
        errors: 0,
        warnings: 0,
        info: 0,
    };
    for issue in issues {
        match issue.severity {
            Severity::Error => summary.errors += 1,
            Severity::Warn => summary.warnings += 1,
            Severity::Info => summary.info += 1,
        }
    }
    summary
}
