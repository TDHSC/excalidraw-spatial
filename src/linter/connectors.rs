use crate::geometry::{ceil_px, clip_segment, fmt_point, fmt_px, Point};
use crate::linter::{disp, label_str, LintIssue, Severity, NEAR_DIST};
use crate::scene_graph::{Attach, Connector, Scene};

pub fn arrow_crossing_node(scene: &Scene) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    for arrow in &scene.connectors {
        if arrow.kind != "arrow" {
            continue;
        }
        let s = attach_target(&arrow.start);
        let e = attach_target(&arrow.end);
        let s_str = attach_str(&arrow.start);
        let e_str = attach_str(&arrow.end);
        let Some(&first) = arrow.points.first() else {
            continue;
        };
        let Some(&last) = arrow.points.last() else {
            continue;
        };
        if arrow.points.len() < 2 {
            continue;
        }
        for n in scene.nodes() {
            let is_endpoint = Some(n.id.as_str()) == s
                || Some(n.id.as_str()) == e
                || n.rect.contains_point(first)
                || n.rect.contains_point(last);
            if arrow.points.len() < 3 && is_endpoint {
                continue;
            }
            let inflated = n.rect.inflate(-2.0);
            let len = arrow.points.len();
            let (seg_start, seg_end) = if is_endpoint && len >= 3 {
                let mut lead = 0;
                while lead < len - 1 && inflated.contains_point(arrow.points[lead]) {
                    lead += 1;
                }
                if n.rect.contains_point(first) {
                    lead = lead.max(1);
                }
                let mut trail = len - 1;
                while trail > 0 && inflated.contains_point(arrow.points[trail]) {
                    trail -= 1;
                }
                if n.rect.contains_point(last) {
                    trail = trail.min(len - 2);
                }
                (lead, trail)
            } else {
                (0, len - 1)
            };
            for i in seg_start..seg_end {
                let p0 = arrow.points[i];
                let p1 = arrow.points[i + 1];
                if clip_segment(p0, p1, &inflated).is_none() {
                    continue;
                }
                let dx = (p1.x - p0.x).abs();
                let dy = (p1.y - p0.y).abs();
                let fix = if dx >= dy {
                    format!(
                        "reroute around {}: e.g. via y={} (above) or y={} (below)",
                        n.id,
                        fmt_px(n.rect.y1 - 40.0),
                        fmt_px(n.rect.y2 + 40.0)
                    )
                } else {
                    format!(
                        "reroute around {}: e.g. via x={} (left) or x={} (right)",
                        n.id,
                        fmt_px(n.rect.x1 - 40.0),
                        fmt_px(n.rect.x2 + 40.0)
                    )
                };
                issues.push(LintIssue {
                    rule: "ARROW_CROSSING_NODE",
                    severity: Severity::Error,
                    targets: vec![arrow.id.clone(), n.id.clone()],
                    message: format!(
                        "arrow {} ({s_str} -> {e_str}) crosses {} at segment {}->{}",
                        arrow.id,
                        disp(&n.id, n.label.as_deref()),
                        fmt_point(p0),
                        fmt_point(p1)
                    ),
                    fix: Some(fix),
                    command: None,
                });
                break;
            }
        }
    }
    issues
}

pub fn arrow_label_collision(scene: &Scene, file: &str) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    for arrow in &scene.connectors {
        if arrow.kind != "arrow" {
            continue;
        }
        let Some(label_rect) = arrow.label_rect else {
            continue;
        };
        let s = attach_target(&arrow.start);
        let e = attach_target(&arrow.end);
        let label_text = arrow.label.as_deref().unwrap_or("");
        for x in &scene.components {
            if !label_rect.overlaps(&x.rect) {
                continue;
            }
            let (dx, dy) = label_rect.overlap_depth(&x.rect);
            let message = format!(
                "label \"{}\" of arrow {} overlaps {} by {}x{}px",
                label_str(label_text),
                arrow.id,
                disp(&x.id, x.label.as_deref()),
                fmt_px(dx),
                fmt_px(dy)
            );
            let end_not_free = !matches!(arrow.end, Attach::Free);
            let (fix, command) =
                if (Some(x.id.as_str()) == s || Some(x.id.as_str()) == e) && end_not_free {
                    if let (Some(&sp), Some(&ep), Some(end_target)) =
                        (arrow.points.first(), arrow.points.last(), e)
                    {
                        let s_rect = s.and_then(|id| scene.rect_of(id));
                        let e_rect = e.and_then(|id| scene.rect_of(id));
                        let (start_pt, end_pt) = match (s_rect, e_rect) {
                            (Some(sr), Some(er)) => {
                                let sc = sr.center();
                                let ec = er.center();
                                if sc.x == ec.x && sc.y == ec.y {
                                    (sp, ep)
                                } else {
                                    (sc, ec)
                                }
                            }
                            _ => (sp, ep),
                        };
                        lengthen_fix(file, end_target, start_pt, end_pt, dx, dy)
                    } else {
                        (format!("move the label or {}", x.id), None)
                    }
                } else {
                    (format!("move the label or {}", x.id), None)
                };
            issues.push(LintIssue {
                rule: "ARROW_LABEL_COLLISION",
                severity: Severity::Error,
                targets: vec![arrow.id.clone(), x.id.clone()],
                message,
                fix: Some(fix),
                command,
            });
        }
        for other in &scene.connectors {
            if other.kind != "arrow" || other.id == arrow.id || other.z <= arrow.z {
                continue;
            }
            let Some(other_rect) = other.label_rect else {
                continue;
            };
            if !label_rect.overlaps(&other_rect) {
                continue;
            }
            let (dx, dy) = label_rect.overlap_depth(&other_rect);
            issues.push(LintIssue {
                rule: "ARROW_LABEL_COLLISION",
                severity: Severity::Error,
                targets: vec![arrow.id.clone(), other.id.clone()],
                message: format!(
                    "label of arrow {} overlaps label of arrow {} by {}x{}px",
                    arrow.id,
                    other.id,
                    fmt_px(dx),
                    fmt_px(dy)
                ),
                fix: Some("move one of the labels".to_string()),
                command: None,
            });
        }
    }
    issues
}

pub fn dangling_arrow(scene: &Scene) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    for arrow in &scene.connectors {
        if arrow.kind != "arrow" {
            continue;
        }
        check_end(scene, arrow, true, &mut issues);
        check_end(scene, arrow, false, &mut issues);
    }
    issues
}

fn attach_target(attach: &Attach) -> Option<&str> {
    match attach {
        Attach::Bound { target, .. } | Attach::Near { target, .. } => Some(target.as_str()),
        Attach::Free => None,
    }
}

fn attach_str(attach: &Attach) -> String {
    match attach {
        Attach::Bound { target, .. } => target.clone(),
        Attach::Near { target, distance } => format!("near:{target}({}px)", fmt_px(*distance)),
        Attach::Free => "free".to_string(),
    }
}

fn lengthen_fix(
    file: &str,
    end_target: &str,
    start: Point,
    end: Point,
    dx: f64,
    dy: f64,
) -> (String, Option<String>) {
    let horizontal = (end.x - start.x).abs() >= (end.y - start.y).abs();
    let depth = if horizontal { dx } else { dy };
    let mut n = ceil_px(2.0 * (depth + 10.0)) as i64;
    if (horizontal && end.x < start.x) || (!horizontal && end.y < start.y) {
        n = -n;
    }
    let (axis, flag) = if horizontal {
        ("x", "--dx")
    } else {
        ("y", "--dy")
    };
    (
        format!("lengthen the arrow: move {end_target} by {n}px along {axis}"),
        Some(format!(
            "excal layout move {file} --id {end_target} {flag} {n}"
        )),
    )
}

fn check_end(scene: &Scene, arrow: &Connector, is_start: bool, issues: &mut Vec<LintIssue>) {
    let attach = if is_start { &arrow.start } else { &arrow.end };
    let p = if is_start {
        arrow.points.first().copied()
    } else {
        arrow.points.last().copied()
    };
    let Some(p) = p else {
        return;
    };
    let which = if is_start { "start" } else { "end" };
    match attach {
        Attach::Free => {
            let mut nearest = None;
            for n in scene.nodes() {
                let d = n.rect.distance_to_point(p);
                if nearest.is_none_or(|(_, best)| d < best) {
                    nearest = Some((n, d));
                }
            }
            let (message, fix) = match nearest {
                Some((n, d)) => (
                    format!(
                        "arrow {} {which} point {} is not connected (nearest {} is {}px away)",
                        arrow.id,
                        fmt_point(p),
                        n.id,
                        fmt_px(d)
                    ),
                    Some(format!(
                        "extend the {which} point to {} on {} and bind it",
                        fmt_point(n.rect.closest_point(p)),
                        n.id
                    )),
                ),
                None => (
                    format!(
                        "arrow {} {which} point {} is not connected",
                        arrow.id,
                        fmt_point(p)
                    ),
                    None,
                ),
            };
            issues.push(LintIssue {
                rule: "DANGLING_ARROW",
                severity: Severity::Warn,
                targets: vec![arrow.id.clone()],
                message,
                fix,
                command: None,
            });
        }
        Attach::Near { target, distance } => {
            issues.push(LintIssue {
                rule: "DANGLING_ARROW",
                severity: Severity::Info,
                targets: vec![arrow.id.clone(), target.clone()],
                message: format!(
                    "arrow {} {which} point touches {target} ({}px) but is not bound; it will not follow {target} when moved in Excalidraw",
                    arrow.id,
                    fmt_px(*distance)
                ),
                fix: Some(format!("bind the {which} of {} to {target}", arrow.id)),
                command: None,
            });
        }
        Attach::Bound { target, .. } => {
            let Some(rect) = scene.rect_of(target) else {
                return;
            };
            let d = rect.distance_to_point(p);
            if d > f64::from(NEAR_DIST) {
                issues.push(LintIssue {
                    rule: "DANGLING_ARROW",
                    severity: Severity::Warn,
                    targets: vec![arrow.id.clone(), target.clone()],
                    message: format!(
                        "arrow {} {which} is bound to {target} but the endpoint is {}px away from it",
                        arrow.id,
                        fmt_px(d)
                    ),
                    fix: Some(format!(
                        "move the {which} point of {} onto the edge of {target}",
                        arrow.id
                    )),
                    command: None,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linter::Severity;
    use crate::scene_graph::{Attach, Scene};

    fn parse_scene(src: &str) -> Scene {
        let doc = crate::schema::Document::parse(src).expect("parse constructed document");
        Scene::build(&doc)
    }

    fn crossing(src: &str) -> Vec<LintIssue> {
        arrow_crossing_node(&parse_scene(src))
            .into_iter()
            .filter(|i| i.rule == "ARROW_CROSSING_NODE")
            .collect()
    }

    fn labels(src: &str) -> Vec<LintIssue> {
        arrow_label_collision(&parse_scene(src), "f.excalidraw")
            .into_iter()
            .filter(|i| i.rule == "ARROW_LABEL_COLLISION")
            .collect()
    }

    fn dangling(src: &str) -> Vec<LintIssue> {
        dangling_arrow(&parse_scene(src))
            .into_iter()
            .filter(|i| i.rule == "DANGLING_ARROW")
            .collect()
    }

    #[test]
    fn arrow_crossing_zone_border_is_not_reported() {
        let scene = parse_scene(
            r#"{
                "elements": [
                    {"id":"zone","type":"rectangle","x":0,"y":0,"width":400,"height":300},
                    {"id":"c1","type":"rectangle","x":50,"y":50,"width":80,"height":80},
                    {"id":"c2","type":"rectangle","x":250,"y":50,"width":80,"height":80},
                    {"id":"arr","type":"arrow","x":-50,"y":150,"width":250,"height":0,"points":[[0,0],[250,0]]}
                ]
            }"#,
        );
        assert!(
            !scene.connectors.is_empty(),
            "arrow must exist in the scene"
        );
        assert!(
            scene.containers.iter().any(|c| c.id == "zone"),
            "zone must be present as a container"
        );
        let issues: Vec<_> = arrow_crossing_node(&scene)
            .into_iter()
            .filter(|i| i.rule == "ARROW_CROSSING_NODE")
            .collect();
        assert!(
            issues.is_empty(),
            "crossing a zone border must not emit: {issues:?}"
        );
    }

    #[test]
    fn arrow_crossing_middle_node_reports_horizontal_reroute() {
        let issues = crossing(
            r#"{
                "elements": [
                    {"id":"a","type":"rectangle","x":0,"y":100,"width":140,"height":80},
                    {"id":"b","type":"rectangle","x":200,"y":100,"width":140,"height":80},
                    {"id":"m","type":"rectangle","x":160,"y":80,"width":80,"height":120},
                    {
                        "id":"arr",
                        "type":"arrow",
                        "x":140,
                        "y":140,
                        "width":260,
                        "height":0,
                        "points":[[0,0],[260,0]],
                        "startBinding":{"elementId":"a","mode":"orbit"},
                        "endBinding":{"elementId":"b","mode":"orbit"}
                    }
                ]
            }"#,
        );
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Error);
        assert_eq!(issues[0].targets, ["arr", "m"]);
        assert_eq!(
            issues[0].message,
            "arrow arr (a -> b) crosses m at segment (140,140)->(400,140)"
        );
        assert_eq!(
            issues[0].fix.as_deref(),
            Some("reroute around m: e.g. via y=40 (above) or y=240 (below)")
        );
        assert_eq!(issues[0].command, None);
    }

    #[test]
    fn arrow_skips_node_that_contains_an_endpoint() {
        let issues = crossing(
            r#"{
                "elements": [
                    {"id":"a","type":"rectangle","x":500,"y":0,"width":40,"height":40},
                    {"id":"b","type":"rectangle","x":600,"y":0,"width":40,"height":40},
                    {"id":"u","type":"rectangle","x":0,"y":100,"width":100,"height":80},
                    {
                        "id":"arr",
                        "type":"arrow",
                        "x":50,
                        "y":140,
                        "width":250,
                        "height":0,
                        "points":[[0,0],[250,0]],
                        "startBinding":{"elementId":"a","mode":"orbit"},
                        "endBinding":{"elementId":"b","mode":"orbit"}
                    }
                ]
            }"#,
        );
        assert!(
            issues
                .iter()
                .all(|i| i.targets.get(1).map(String::as_str) != Some("u")),
            "node containing an endpoint must be skipped: {issues:?}"
        );
        assert!(
            issues.is_empty(),
            "expected no crossing issues, got {issues:?}"
        );
    }

    #[test]
    fn vertical_crossing_suggests_left_or_right_reroute() {
        let issues = crossing(
            r#"{
                "elements": [
                    {"id":"a","type":"rectangle","x":100,"y":0,"width":80,"height":40},
                    {"id":"b","type":"rectangle","x":100,"y":300,"width":80,"height":40},
                    {"id":"m","type":"rectangle","x":80,"y":100,"width":120,"height":80},
                    {
                        "id":"arr",
                        "type":"arrow",
                        "x":140,
                        "y":40,
                        "width":0,
                        "height":260,
                        "points":[[0,0],[0,260]],
                        "startBinding":{"elementId":"a","mode":"orbit"},
                        "endBinding":{"elementId":"b","mode":"orbit"}
                    }
                ]
            }"#,
        );
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].targets, ["arr", "m"]);
        assert_eq!(
            issues[0].fix.as_deref(),
            Some("reroute around m: e.g. via x=40 (left) or x=240 (right)")
        );
        assert!(issues[0].message.contains("a -> b"));
    }

    #[test]
    fn crossing_message_formats_near_and_free_endpoints() {
        let scene = parse_scene(
            r#"{
                "elements": [
                    {"id":"a","type":"rectangle","x":0,"y":100,"width":140,"height":80},
                    {"id":"m","type":"rectangle","x":200,"y":80,"width":80,"height":120},
                    {
                        "id":"arr",
                        "type":"arrow",
                        "x":146,
                        "y":140,
                        "width":254,
                        "height":0,
                        "points":[[0,0],[254,0]]
                    }
                ]
            }"#,
        );
        match &scene.connectors[0].start {
            Attach::Near { target, distance } => {
                assert_eq!(target, "a");
                assert!((distance - 6.0).abs() < 1e-9, "distance {distance}");
            }
            other => panic!("expected Near start, got {other:?}"),
        }
        assert_eq!(scene.connectors[0].end, Attach::Free);
        let issues: Vec<_> = arrow_crossing_node(&scene)
            .into_iter()
            .filter(|i| i.rule == "ARROW_CROSSING_NODE")
            .collect();
        assert_eq!(issues.len(), 1);
        assert!(
            issues[0].message.contains("near:a(6px) -> free"),
            "message: {}",
            issues[0].message
        );
    }

    #[test]
    fn label_collision_with_non_endpoint_has_move_fix_and_no_command() {
        let issues = labels(
            r#"{
                "elements": [
                    {"id":"a","type":"rectangle","x":0,"y":180,"width":40,"height":40},
                    {"id":"b","type":"rectangle","x":200,"y":180,"width":40,"height":40},
                    {"id":"x","type":"rectangle","x":80,"y":80,"width":60,"height":40},
                    {
                        "id":"arr",
                        "type":"arrow",
                        "x":40,
                        "y":200,
                        "width":160,
                        "height":0,
                        "points":[[0,0],[160,0]],
                        "startBinding":{"elementId":"a","mode":"orbit"},
                        "endBinding":{"elementId":"b","mode":"orbit"}
                    },
                    {
                        "id":"arr-label",
                        "type":"text",
                        "x":80,
                        "y":90,
                        "width":50,
                        "height":20,
                        "text":"go",
                        "fontSize":20,
                        "containerId":"arr"
                    }
                ]
            }"#,
        );
        let hit: Vec<_> = issues
            .iter()
            .filter(|i| i.targets == ["arr", "x"])
            .collect();
        assert_eq!(hit.len(), 1, "{issues:?}");
        assert_eq!(hit[0].severity, Severity::Error);
        assert_eq!(
            hit[0].message,
            r#"label "go" of arrow arr overlaps x by 50x20px"#
        );
        assert_eq!(hit[0].fix.as_deref(), Some("move the label or x"));
        assert_eq!(hit[0].command, None);
    }

    #[test]
    fn label_collision_with_end_of_upward_arrow_uses_negative_dy() {
        let issues = labels(
            r#"{
                "elements": [
                    {"id":"s","type":"rectangle","x":40,"y":260,"width":120,"height":80},
                    {"id":"e","type":"rectangle","x":40,"y":0,"width":120,"height":100},
                    {
                        "id":"arr",
                        "type":"arrow",
                        "x":100,
                        "y":260,
                        "width":0,
                        "height":160,
                        "points":[[0,0],[0,-160]],
                        "startBinding":{"elementId":"s","mode":"orbit"},
                        "endBinding":{"elementId":"e","mode":"orbit"}
                    },
                    {
                        "id":"arr-label",
                        "type":"text",
                        "x":50,
                        "y":20,
                        "width":80,
                        "height":30,
                        "text":"go",
                        "fontSize":20,
                        "containerId":"arr"
                    }
                ]
            }"#,
        );
        let hit: Vec<_> = issues
            .iter()
            .filter(|i| i.targets == ["arr", "e"])
            .collect();
        assert_eq!(hit.len(), 1, "{issues:?}");
        assert_eq!(
            hit[0].fix.as_deref(),
            Some("lengthen the arrow: move e by -80px along y")
        );
        assert_eq!(
            hit[0].command.as_deref(),
            Some("excal layout move f.excalidraw --id e --dy -80")
        );
    }

    #[test]
    fn overlapping_arrow_labels_reported_once_from_lower_z() {
        let issues = labels(
            r#"{
                "elements": [
                    {"id":"a1","type":"arrow","x":0,"y":0,"width":100,"height":0,"points":[[0,0],[100,0]]},
                    {
                        "id":"a1-label",
                        "type":"text",
                        "x":20,
                        "y":-10,
                        "width":80,
                        "height":20,
                        "text":"one",
                        "fontSize":20,
                        "containerId":"a1"
                    },
                    {"id":"a2","type":"arrow","x":40,"y":5,"width":100,"height":0,"points":[[0,0],[100,0]]},
                    {
                        "id":"a2-label",
                        "type":"text",
                        "x":40,
                        "y":-5,
                        "width":80,
                        "height":20,
                        "text":"two",
                        "fontSize":20,
                        "containerId":"a2"
                    }
                ]
            }"#,
        );
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert_eq!(issues[0].targets, ["a1", "a2"]);
        assert_eq!(
            issues[0].message,
            "label of arrow a1 overlaps label of arrow a2 by 60x15px"
        );
        assert_eq!(issues[0].fix.as_deref(), Some("move one of the labels"));
        assert_eq!(issues[0].command, None);
    }

    #[test]
    fn dangling_free_with_no_nodes_has_no_fix() {
        let issues = dangling(
            r#"{
                "elements": [
                    {"id":"arr","type":"arrow","x":0,"y":0,"width":50,"height":0,"points":[[0,0],[50,0]]}
                ]
            }"#,
        );
        assert_eq!(issues.len(), 2, "{issues:?}");
        assert_eq!(issues[0].severity, Severity::Warn);
        assert_eq!(issues[0].targets, ["arr"]);
        assert_eq!(
            issues[0].message,
            "arrow arr start point (0,0) is not connected"
        );
        assert_eq!(issues[0].fix, None);
        assert_eq!(issues[0].command, None);
        assert_eq!(
            issues[1].message,
            "arrow arr end point (50,0) is not connected"
        );
        assert_eq!(issues[1].fix, None);
    }

    #[test]
    fn dangling_bound_reports_only_when_farther_than_near_dist() {
        let far = dangling(
            r#"{
                "elements": [
                    {"id":"a","type":"rectangle","x":0,"y":0,"width":100,"height":80},
                    {
                        "id":"arr",
                        "type":"arrow",
                        "x":100,
                        "y":40,
                        "width":80,
                        "height":0,
                        "points":[[0,0],[80,0]],
                        "startBinding":{"elementId":"a","mode":"orbit"},
                        "endBinding":{"elementId":"a","mode":"orbit"}
                    }
                ]
            }"#,
        );
        assert_eq!(far.len(), 1, "{far:?}");
        assert_eq!(far[0].severity, Severity::Warn);
        assert_eq!(far[0].targets, ["arr", "a"]);
        assert_eq!(
            far[0].message,
            "arrow arr end is bound to a but the endpoint is 80px away from it"
        );
        assert_eq!(
            far[0].fix.as_deref(),
            Some("move the end point of arr onto the edge of a")
        );
        assert_eq!(far[0].command, None);

        let close = dangling(
            r#"{
                "elements": [
                    {"id":"a","type":"rectangle","x":0,"y":0,"width":100,"height":80},
                    {
                        "id":"arr",
                        "type":"arrow",
                        "x":100,
                        "y":40,
                        "width":10,
                        "height":0,
                        "points":[[0,0],[10,0]],
                        "startBinding":{"elementId":"a","mode":"orbit"},
                        "endBinding":{"elementId":"a","mode":"orbit"}
                    }
                ]
            }"#,
        );
        assert!(close.is_empty(), "{close:?}");
    }

    #[test]
    fn line_crossing_a_node_is_not_an_arrow_crossing() {
        let issues = crossing(
            r#"{
                "elements": [
                    {"id":"m","type":"rectangle","x":160,"y":80,"width":80,"height":120},
                    {
                        "id":"ln",
                        "type":"line",
                        "x":140,
                        "y":140,
                        "width":260,
                        "height":0,
                        "points":[[0,0],[260,0]]
                    }
                ]
            }"#,
        );
        assert!(issues.is_empty(), "{issues:?}");
    }
}
