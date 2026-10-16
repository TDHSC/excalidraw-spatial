use crate::geometry::{ceil_px, fmt_px};
use crate::linter::{disp, LintIssue, Severity, CONTAINER_WARN_PADDING, MIN_GAP};
use crate::scene_graph::Scene;
use crate::spatial::children;

pub fn overlap_component(scene: &Scene, file: &str) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let comps = &scene.components;
    for i in 0..comps.len() {
        for j in i + 1..comps.len() {
            let a = &comps[i];
            let b = &comps[j];
            if !a.rect.overlaps(&b.rect) {
                continue;
            }
            let (dx, dy) = a.rect.overlap_depth(&b.rect);
            let (first, second, n, horizontal) = if dx <= dy {
                let (aa, bb) = if a.rect.center().x <= b.rect.center().x {
                    (a, b)
                } else {
                    (b, a)
                };
                let n = ceil_px(aa.rect.x2 + f64::from(MIN_GAP) - bb.rect.x1);
                (aa, bb, n, true)
            } else {
                let (aa, bb) = if a.rect.center().y <= b.rect.center().y {
                    (a, b)
                } else {
                    (b, a)
                };
                let n = ceil_px(aa.rect.y2 + f64::from(MIN_GAP) - bb.rect.y1);
                (aa, bb, n, false)
            };
            let n_s = fmt_px(n);
            let (fix, command) = if horizontal {
                (
                    format!("move {} right by {n_s}px to leave a 40px gap", second.id),
                    format!("excal layout move {file} --id {} --dx {n_s}", second.id),
                )
            } else {
                (
                    format!("move {} down by {n_s}px to leave a 40px gap", second.id),
                    format!("excal layout move {file} --id {} --dy {n_s}", second.id),
                )
            };
            issues.push(LintIssue {
                rule: "OVERLAP_COMPONENT",
                severity: Severity::Error,
                targets: vec![first.id.clone(), second.id.clone()],
                message: format!(
                    "{} overlaps {} by {}x{}px",
                    disp(&first.id, first.label.as_deref()),
                    disp(&second.id, second.label.as_deref()),
                    fmt_px(dx),
                    fmt_px(dy)
                ),
                fix: Some(fix),
                command: Some(command),
            });
        }
    }
    issues
}

pub fn container_overflow(scene: &Scene, file: &str) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    for container in &scene.containers {
        for (kid_id, kid_rect) in children(scene, &container.id) {
            let top = kid_rect.y1 - container.rect.y1;
            let right = container.rect.x2 - kid_rect.x2;
            let bottom = container.rect.y2 - kid_rect.y2;
            let left = kid_rect.x1 - container.rect.x1;
            let sides = [
                ("top", top),
                ("right", right),
                ("bottom", bottom),
                ("left", left),
            ];
            let (side, v) = sides
                .iter()
                .copied()
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                .expect("four sides");
            if v >= f64::from(CONTAINER_WARN_PADDING) {
                continue;
            }
            let kid_disp = node_disp(scene, &kid_id);
            let container_disp = disp(&container.id, container.name.as_deref());
            let (severity, message, fix) = if v < 0.0 {
                (
                    Severity::Error,
                    format!(
                        "{kid_disp} extends {}px outside the {side} edge of {container_disp}",
                        fmt_px(-v)
                    ),
                    format!("grow {} or move {kid_id} inside it", container.id),
                )
            } else {
                (
                    Severity::Warn,
                    format!(
                        "{kid_disp} is only {}px from the {side} edge of {container_disp}",
                        fmt_px(v)
                    ),
                    format!("grow {} or move {kid_id} inward", container.id),
                )
            };
            issues.push(LintIssue {
                rule: "CONTAINER_OVERFLOW",
                severity,
                targets: vec![container.id.clone(), kid_id.clone()],
                message,
                fix: Some(fix),
                command: Some(format!(
                    "excal layout fit-container {file} --id {} --padding 40",
                    container.id
                )),
            });
        }
    }
    issues
}

fn node_disp(scene: &Scene, id: &str) -> String {
    if let Some(c) = scene.component(id) {
        disp(id, c.label.as_deref())
    } else if let Some(c) = scene.container(id) {
        disp(id, c.name.as_deref())
    } else {
        id.to_string()
    }
}
