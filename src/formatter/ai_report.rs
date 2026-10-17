use std::collections::HashMap;

use crate::formatter::ascii_map::{self, render};
use crate::geometry::{fmt_point, fmt_px, fmt_rect};
use crate::linter::{inline_text, LintIssue, Severity};
use crate::scene_graph::{Attach, Component, ComponentKind, Connector, Container, Scene};
use crate::spatial::{self, Track};

pub fn format_inspect(file: &str, scene: &Scene, issues: &[LintIssue]) -> String {
    let mut out = String::new();
    out.push_str(&format!("# excal inspect {file}\n"));
    out.push_str(&summary_line(scene, issues));
    out.push('\n');

    let sections = [
        ("Containers", container_lines(scene)),
        ("Components", component_lines(scene)),
        ("Rows", track_lines(scene, Axis::Row)),
        ("Columns", track_lines(scene, Axis::Column)),
        ("Connections", connection_lines(scene)),
        ("Minimap", minimap_lines(scene)),
        ("Issues", issue_lines(issues)),
    ];
    for (index, (title, lines)) in sections.iter().enumerate() {
        section(&mut out, title, lines, index + 1 == sections.len());
    }
    out
}

fn summary_line(scene: &Scene, issues: &[LintIssue]) -> String {
    let (errors, warnings, info) = count_issues(issues);
    let counts = format!(
        "components: {}, containers: {}, connectors: {} | errors: {errors}, warnings: {warnings}, info: {info}",
        scene.components.len(),
        scene.containers.len(),
        scene.connectors.len()
    );
    match ascii_map::canvas_rect(scene) {
        Some(rect) => format!("canvas {} | {counts}\n", fmt_rect(&rect)),
        None => format!("canvas (empty) | {counts}\n"),
    }
}

fn count_issues(issues: &[LintIssue]) -> (usize, usize, usize) {
    let mut errors = 0;
    let mut warnings = 0;
    let mut info = 0;
    for issue in issues {
        match issue.severity {
            Severity::Error => errors += 1,
            Severity::Warn => warnings += 1,
            Severity::Info => info += 1,
        }
    }
    (errors, warnings, info)
}

fn section(out: &mut String, title: &str, lines: &[String], last: bool) {
    out.push_str("## ");
    out.push_str(title);
    out.push('\n');
    if lines.is_empty() {
        out.push_str("(none)\n");
    } else {
        for line in lines {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !last {
        out.push('\n');
    }
}

fn container_lines(scene: &Scene) -> Vec<String> {
    scene
        .containers
        .iter()
        .map(|container| format_container(scene, container))
        .collect()
}

fn format_container(scene: &Scene, container: &Container) -> String {
    let kind = match container.kind {
        crate::scene_graph::ContainerKind::Zone => "zone",
        crate::scene_graph::ContainerKind::Frame => "frame",
    };
    let name = container
        .name
        .as_deref()
        .map(inline_text)
        .filter(|name| !name.is_empty())
        .map(|name| format!(" \"{name}\""))
        .unwrap_or_default();
    let kids = spatial::children(scene, &container.id);
    let padding = if kids.is_empty() {
        "padding -".to_string()
    } else {
        let pad = spatial::padding(scene, &container.id).expect("children imply padding");
        format!(
            "padding T{} R{} B{} L{}",
            fmt_px(pad.top),
            fmt_px(pad.right),
            fmt_px(pad.bottom),
            fmt_px(pad.left)
        )
    };
    let child_ids = kids
        .iter()
        .map(|(id, _)| id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "- {} {kind}{name} {} parent={} {padding} children=[{child_ids}]",
        container.id,
        fmt_rect(&container.rect),
        parent_id(container.parent.as_deref())
    )
}

fn component_lines(scene: &Scene) -> Vec<String> {
    let siblings = sibling_index(scene);
    scene
        .components
        .iter()
        .map(|component| {
            format_component(
                component,
                siblings.get(&component.parent.clone().unwrap_or_default()),
            )
        })
        .collect()
}

fn format_component(component: &Component, siblings: Option<&Vec<&Component>>) -> String {
    let label = component
        .label
        .as_deref()
        .map(inline_text)
        .filter(|label| !label.is_empty())
        .map(|label| format!(" \"{label}\""))
        .unwrap_or_default();
    let center = component.rect.center();
    let mut line = format!(
        "- {}{label} {} {} c({},{}) parent={}",
        component.id,
        component.type_name,
        fmt_rect(&component.rect),
        fmt_px(center.x),
        fmt_px(center.y),
        parent_id(component.parent.as_deref())
    );
    if component.kind != ComponentKind::Text {
        let empty = Vec::new();
        let peers = siblings.unwrap_or(&empty);
        let neighbors = spatial::neighbors(component, peers);
        line.push_str(" L=");
        line.push_str(&neighbor_token(neighbors.left.as_ref()));
        line.push_str(" R=");
        line.push_str(&neighbor_token(neighbors.right.as_ref()));
        line.push_str(" T=");
        line.push_str(&neighbor_token(neighbors.top.as_ref()));
        line.push_str(" B=");
        line.push_str(&neighbor_token(neighbors.bottom.as_ref()));
    }
    line
}

fn neighbor_token(neighbor: Option<&spatial::Neighbor>) -> String {
    match neighbor {
        Some(neighbor) => format!("{}({})", neighbor.id, fmt_px(neighbor.gap)),
        None => "-".to_string(),
    }
}

enum Axis {
    Row,
    Column,
}

fn track_lines(scene: &Scene, axis: Axis) -> Vec<String> {
    let groups = sibling_groups(scene);
    let mut lines = Vec::new();
    for components in groups {
        let parent = components.first().and_then(|c| c.parent.as_deref());
        let obstacles = spatial::sibling_container_rects(scene, parent);
        let mut tracks = match axis {
            Axis::Row => spatial::rows_with_obstacles(&components, &obstacles),
            Axis::Column => spatial::columns_with_obstacles(&components, &obstacles),
        };
        tracks.sort_by(|a, b| a.pos.total_cmp(&b.pos));
        for track in tracks {
            lines.push(format_track(&track, &axis));
        }
    }
    lines
}

fn format_track(track: &Track, axis: &Axis) -> String {
    let pos_name = match axis {
        Axis::Row => "y",
        Axis::Column => "x",
    };
    let parent = track.parent.as_deref().unwrap_or("top");
    let mut body = track.members[0].clone();
    for (gap, member) in track.gaps.iter().zip(track.members.iter().skip(1)) {
        body.push_str(" --");
        body.push_str(&fmt_px(*gap));
        body.push_str("-- ");
        body.push_str(member);
    }
    format!("- {parent} {pos_name}≈{}: {body}", fmt_px(track.pos))
}

fn connection_lines(scene: &Scene) -> Vec<String> {
    scene.connectors.iter().map(format_connector).collect()
}

fn format_connector(connector: &Connector) -> String {
    let label = connector
        .label
        .as_deref()
        .map(inline_text)
        .filter(|label| !label.is_empty())
        .map(|label| format!(" \"{label}\""))
        .unwrap_or_default();
    let kind = if connector.kind == "line" {
        "line"
    } else if connector.elbowed {
        "elbow"
    } else {
        "straight"
    };
    let points = connector
        .points
        .iter()
        .map(|point| fmt_point(*point))
        .collect::<Vec<_>>()
        .join(" -> ");
    format!(
        "- {} {} -> {}{label} {kind} {points}",
        connector.id,
        attach_token(&connector.start),
        attach_token(&connector.end)
    )
}

fn attach_token(attach: &Attach) -> String {
    match attach {
        Attach::Bound { target, .. } => target.clone(),
        Attach::Near { target, distance } => format!("near:{target}({}px)", fmt_px(*distance)),
        Attach::Free => "free".to_string(),
    }
}

fn minimap_lines(scene: &Scene) -> Vec<String> {
    render(scene)
}

fn issue_lines(issues: &[LintIssue]) -> Vec<String> {
    let mut lines = Vec::new();
    for issue in issues {
        let severity = match issue.severity {
            Severity::Error => "ERROR",
            Severity::Warn => "WARN ",
            Severity::Info => "INFO ",
        };
        lines.push(format!("{severity} {} {}", issue.rule, issue.message));
        if let Some(fix) = &issue.fix {
            lines.push(format!("      fix: {fix}"));
        }
        if let Some(command) = &issue.command {
            lines.push(format!("      run: {command}"));
        }
    }
    lines
}

fn parent_id(parent: Option<&str>) -> &str {
    parent.unwrap_or("-")
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

fn sibling_groups(scene: &Scene) -> Vec<Vec<&Component>> {
    let groups = sibling_index(scene);
    let mut containers: Vec<&Container> = scene.containers.iter().collect();
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
