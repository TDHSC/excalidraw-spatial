use crate::geometry::fmt_px;
use crate::linter::{
    disp, LintIssue, Severity, NEAR_MISS_MAX, TEXT_PAD_X, TEXT_PAD_Y, TEXT_SHAPES, TIGHT_GAP,
    UNEVEN_DIFF,
};
use crate::scene_graph::{Component, ComponentKind, Scene};
use crate::spatial::{self, Track};

pub fn text_overflow(scene: &Scene, file: &str) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    for c in &scene.components {
        if c.kind != ComponentKind::Shape || !TEXT_SHAPES.contains(&c.type_name.as_str()) {
            continue;
        }
        let Some(t) = c.label_size.as_ref() else {
            continue;
        };
        let f = text_factor(&c.type_name);
        let w = c.rect.width();
        let h = c.rect.height();
        if t.width <= w * f - f64::from(TEXT_PAD_X) && t.height <= h * f - f64::from(TEXT_PAD_Y) {
            continue;
        }
        let suffix = if t.estimated { " (estimated)" } else { "" };
        issues.push(LintIssue {
            rule: "TEXT_OVERFLOW",
            severity: Severity::Warn,
            targets: vec![c.id.clone()],
            message: format!(
                "label of {} needs {}x{}px{suffix} but the {} is {}x{}px",
                disp(&c.id, c.label.as_deref()),
                fmt_px(t.width),
                fmt_px(t.height),
                c.type_name,
                fmt_px(w),
                fmt_px(h)
            ),
            fix: Some(format!("grow {} to fit its label", c.id)),
            command: Some(format!("excal layout fit-text {file} --id {}", c.id)),
        });
    }
    issues
}

pub fn near_miss_alignment(scene: &Scene, file: &str) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    for siblings in sibling_groups(scene) {
        for track in spatial::rows(&siblings) {
            emit_near_miss(scene, file, &track, true, &mut issues);
        }
        for track in spatial::columns(&siblings) {
            emit_near_miss(scene, file, &track, false, &mut issues);
        }
    }
    issues
}

pub fn uneven_spacing(scene: &Scene, file: &str) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    for siblings in sibling_groups(scene) {
        emit_uneven(file, &spatial::rows(&siblings), "horizontal", &mut issues);
        emit_uneven(file, &spatial::columns(&siblings), "vertical", &mut issues);
    }
    issues
}

fn text_factor(type_name: &str) -> f64 {
    match type_name {
        "ellipse" => 0.707,
        "diamond" => 0.5,
        _ => 1.0,
    }
}

fn sibling_groups(scene: &Scene) -> Vec<Vec<&Component>> {
    let nodes = scene.nodes();
    let mut groups = Vec::new();
    let top: Vec<&Component> = nodes
        .iter()
        .copied()
        .filter(|c| c.parent.is_none())
        .collect();
    if !top.is_empty() {
        groups.push(top);
    }
    for container in &scene.containers {
        let kids: Vec<&Component> = nodes
            .iter()
            .copied()
            .filter(|c| c.parent.as_deref() == Some(container.id.as_str()))
            .collect();
        if !kids.is_empty() {
            groups.push(kids);
        }
    }
    groups
}

fn emit_near_miss(
    scene: &Scene,
    file: &str,
    track: &Track,
    row: bool,
    issues: &mut Vec<LintIssue>,
) {
    for pair in track.members.windows(2) {
        let Some(a) = scene.component(&pair[0]) else {
            continue;
        };
        let Some(b) = scene.component(&pair[1]) else {
            continue;
        };
        let ds: [(&str, f64); 3] = if row {
            [
                ("middle", (a.rect.center().y - b.rect.center().y).abs()),
                ("top", (a.rect.y1 - b.rect.y1).abs()),
                ("bottom", (a.rect.y2 - b.rect.y2).abs()),
            ]
        } else {
            [
                ("center", (a.rect.center().x - b.rect.center().x).abs()),
                ("left", (a.rect.x1 - b.rect.x1).abs()),
                ("right", (a.rect.x2 - b.rect.x2).abs()),
            ]
        };
        if ds.iter().any(|(_, d)| *d < 0.5) {
            continue;
        }
        let (axis, d) = ds
            .into_iter()
            .min_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(std::cmp::Ordering::Equal))
            .expect("three deltas");
        if !(1.0 <= d && d <= f64::from(NEAR_MISS_MAX)) {
            continue;
        }
        issues.push(LintIssue {
            rule: "NEAR_MISS_ALIGNMENT",
            severity: Severity::Warn,
            targets: vec![a.id.clone(), b.id.clone()],
            message: format!(
                "{} and {} are misaligned by {}px ({axis})",
                disp(&a.id, a.label.as_deref()),
                disp(&b.id, b.label.as_deref()),
                fmt_px(d)
            ),
            fix: Some(format!("align {} to {} ({axis})", b.id, a.id)),
            command: Some(format!(
                "excal layout align {file} --ids {},{} --axis {axis}",
                a.id, b.id
            )),
        });
    }
}

fn emit_uneven(file: &str, tracks: &[Track], direction: &str, issues: &mut Vec<LintIssue>) {
    for track in tracks {
        if track.gaps.iter().any(|g| *g < 0.0) {
            continue;
        }
        let ids = track.members.join(",");
        let gs = track
            .gaps
            .iter()
            .map(|g| fmt_px(*g))
            .collect::<Vec<_>>()
            .join(", ");
        let max_g = track.gaps.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let min_g = track.gaps.iter().copied().fold(f64::INFINITY, f64::min);
        let (g, message, fix) =
            if track.members.len() >= 3 && max_g - min_g > f64::from(UNEVEN_DIFF) {
                let g = (max_g.round() as i64).max(40);
                (
                    g,
                    format!("{direction} sequence [{ids}] has uneven gaps [{gs}]px"),
                    format!("space them evenly with a {g}px gap"),
                )
            } else if track.gaps.iter().any(|gap| *gap < f64::from(TIGHT_GAP)) {
                (
                    40,
                    format!("{direction} sequence [{ids}] has tight gaps [{gs}]px (< 30px)"),
                    "space them with a 40px gap".to_string(),
                )
            } else {
                continue;
            };
        issues.push(LintIssue {
            rule: "UNEVEN_SPACING",
            severity: Severity::Info,
            targets: track.members.clone(),
            message,
            fix: Some(fix),
            command: Some(format!(
                "excal layout distribute {file} --ids {ids} --direction {direction} --gap {g}"
            )),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linter::Severity;
    use crate::scene_graph::Scene;
    fn parse_scene(src: &str) -> Scene {
        let doc = crate::schema::Document::parse(src).expect("parse constructed document");
        Scene::build(&doc)
    }

    fn labeled_shape(id: &str, type_name: &str, x: f64) -> String {
        format!(
            r#"{{
                "id": "{id}",
                "type": "{type_name}",
                "x": {x},
                "y": 0,
                "width": 160,
                "height": 80
            }},
            {{
                "id": "{id}-label",
                "type": "text",
                "x": {label_x},
                "y": 27.5,
                "width": 100,
                "height": 25,
                "text": "Label",
                "fontSize": 20,
                "containerId": "{id}"
            }}"#,
            label_x = x + 30.0
        )
    }

    fn two_boxes(dy: f64) -> Scene {
        parse_scene(&format!(
            r#"{{
                "elements": [
                    {{"id":"a","type":"rectangle","x":0,"y":0,"width":100,"height":80}},
                    {{"id":"b","type":"rectangle","x":200,"y":{dy},"width":100,"height":80}}
                ]
            }}"#
        ))
    }

    fn near_miss_issues(dy: f64) -> Vec<LintIssue> {
        near_miss_alignment(&two_boxes(dy), "f.excalidraw")
            .into_iter()
            .filter(|i| i.rule == "NEAR_MISS_ALIGNMENT")
            .collect()
    }

    #[test]
    fn ellipse_label_overflows_but_same_size_rectangle_does_not() {
        let scene = parse_scene(&format!(
            r#"{{
                "elements": [
                    {},
                    {}
                ]
            }}"#,
            labeled_shape("e", "ellipse", 0.0),
            labeled_shape("r", "rectangle", 400.0)
        ));
        let issues = text_overflow(&scene, "f.excalidraw");
        let overflows: Vec<_> = issues
            .iter()
            .filter(|i| i.rule == "TEXT_OVERFLOW")
            .collect();
        assert_eq!(overflows.len(), 1);
        assert_eq!(overflows[0].targets, ["e"]);
        assert_eq!(overflows[0].severity, Severity::Warn);
        assert_eq!(
            overflows[0].message,
            r#"label of e "Label" needs 100x25px but the ellipse is 160x80px"#
        );
        assert!(issues.iter().all(|i| i.targets != ["r"]));
    }

    #[test]
    fn near_miss_skips_sub_pixel_and_beyond_max_but_reports_three_px() {
        assert!(near_miss_issues(0.3).is_empty());
        assert!(near_miss_issues(9.0).is_empty());

        let issues = near_miss_issues(3.0);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].targets, ["a", "b"]);
        assert_eq!(issues[0].severity, Severity::Warn);
        assert_eq!(issues[0].message, "a and b are misaligned by 3px (middle)");
        assert_eq!(issues[0].fix.as_deref(), Some("align b to a (middle)"));
        assert_eq!(
            issues[0].command.as_deref(),
            Some("excal layout align f.excalidraw --ids a,b --axis middle")
        );
    }

    fn spacing_issues(src: &str) -> Vec<LintIssue> {
        uneven_spacing(&parse_scene(src), "f.excalidraw")
            .into_iter()
            .filter(|i| i.rule == "UNEVEN_SPACING")
            .collect()
    }

    #[test]
    fn uneven_spacing_reports_uneven_row_with_gap_60() {
        let issues = spacing_issues(
            r#"{
                "elements": [
                    {"id":"a","type":"rectangle","x":0,"y":0,"width":100,"height":80},
                    {"id":"b","type":"rectangle","x":160,"y":0,"width":100,"height":80},
                    {"id":"c","type":"rectangle","x":280,"y":0,"width":100,"height":80}
                ]
            }"#,
        );
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Info);
        assert_eq!(issues[0].targets, ["a", "b", "c"]);
        assert_eq!(
            issues[0].message,
            "horizontal sequence [a,b,c] has uneven gaps [60, 20]px"
        );
        assert_eq!(
            issues[0].fix.as_deref(),
            Some("space them evenly with a 60px gap")
        );
        assert_eq!(
            issues[0].command.as_deref(),
            Some(
                "excal layout distribute f.excalidraw --ids a,b,c --direction horizontal --gap 60"
            )
        );
    }

    #[test]
    fn uneven_spacing_skips_negative_gap_and_reports_tight_pair() {
        let negative = spacing_issues(
            r#"{
                "elements": [
                    {"id":"a","type":"rectangle","x":0,"y":0,"width":100,"height":80},
                    {"id":"b","type":"rectangle","x":80,"y":0,"width":100,"height":80},
                    {"id":"c","type":"rectangle","x":200,"y":0,"width":100,"height":80}
                ]
            }"#,
        );
        assert!(negative.is_empty());

        let tight = spacing_issues(
            r#"{
                "elements": [
                    {"id":"a","type":"rectangle","x":0,"y":0,"width":100,"height":80},
                    {"id":"b","type":"rectangle","x":110,"y":0,"width":100,"height":80}
                ]
            }"#,
        );
        assert_eq!(tight.len(), 1);
        assert_eq!(tight[0].targets, ["a", "b"]);
        assert_eq!(
            tight[0].message,
            "horizontal sequence [a,b] has tight gaps [10]px (< 30px)"
        );
        assert_eq!(tight[0].fix.as_deref(), Some("space them with a 40px gap"));
        assert_eq!(
            tight[0].command.as_deref(),
            Some("excal layout distribute f.excalidraw --ids a,b --direction horizontal --gap 40")
        );

        let even = spacing_issues(
            r#"{
                "elements": [
                    {"id":"a","type":"rectangle","x":0,"y":0,"width":100,"height":80},
                    {"id":"b","type":"rectangle","x":140,"y":0,"width":100,"height":80}
                ]
            }"#,
        );
        assert!(even.is_empty());
    }
}
