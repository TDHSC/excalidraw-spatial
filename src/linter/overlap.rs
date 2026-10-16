use crate::geometry::{ceil_px, fmt_px};
use crate::linter::{disp, LintIssue, Severity, CONTAINER_WARN_PADDING, MIN_GAP};
use crate::scene_graph::{Component, ComponentKind, Container, Scene};
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

    let containers = &scene.containers;
    for i in 0..containers.len() {
        for j in i + 1..containers.len() {
            let c1 = &containers[i];
            let c2 = &containers[j];
            if c1.parent != c2.parent {
                continue;
            }
            if c1.rect.contains_point(c2.rect.center()) || c2.rect.contains_point(c1.rect.center())
            {
                continue;
            }
            if !c1.rect.overlaps(&c2.rect) {
                continue;
            }
            let (dx, dy) = c1.rect.overlap_depth(&c2.rect);
            let (first, second, n, horizontal) = if dx <= dy {
                let (aa, bb) = if c1.rect.center().x <= c2.rect.center().x {
                    (c1, c2)
                } else {
                    (c2, c1)
                };
                let n = ceil_px(aa.rect.x2 + f64::from(MIN_GAP) - bb.rect.x1);
                (aa, bb, n, true)
            } else {
                let (aa, bb) = if c1.rect.center().y <= c2.rect.center().y {
                    (c1, c2)
                } else {
                    (c2, c1)
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
                    disp(&first.id, first.name.as_deref()),
                    disp(&second.id, second.name.as_deref()),
                    fmt_px(dx),
                    fmt_px(dy)
                ),
                fix: Some(fix),
                command: Some(command),
            });
        }
    }

    for container in containers {
        for n in scene.nodes() {
            if n.kind != ComponentKind::Shape && n.kind != ComponentKind::Group {
                continue;
            }
            if n.parent != container.parent {
                continue;
            }
            if container.rect.contains_point(n.rect.center())
                || n.rect.contains_point(container.rect.center())
            {
                continue;
            }
            if !container.rect.overlaps(&n.rect) {
                continue;
            }
            let (dx, dy) = container.rect.overlap_depth(&n.rect);
            let (dir, n_move, flag, signed) = push_node_away(container, n, dx, dy);
            let n_s = fmt_px(n_move);
            let signed_s = if signed {
                format!("-{n_s}")
            } else {
                n_s.clone()
            };
            issues.push(LintIssue {
                rule: "OVERLAP_COMPONENT",
                severity: Severity::Error,
                targets: vec![container.id.clone(), n.id.clone()],
                message: format!(
                    "{} overlaps {} by {}x{}px",
                    disp(&container.id, container.name.as_deref()),
                    disp(&n.id, n.label.as_deref()),
                    fmt_px(dx),
                    fmt_px(dy)
                ),
                fix: Some(format!(
                    "move {} {dir} by {n_s}px to leave a 40px gap",
                    n.id
                )),
                command: Some(format!(
                    "excal layout move {file} --id {} {flag} {signed_s}",
                    n.id
                )),
            });
        }
    }
    issues
}

fn push_node_away(
    container: &Container,
    n: &Component,
    dx: f64,
    dy: f64,
) -> (&'static str, f64, &'static str, bool) {
    if dx <= dy {
        if n.rect.center().x < container.rect.center().x {
            let amount = ceil_px(n.rect.x2 + 40.0 - container.rect.x1);
            ("left", amount, "--dx", true)
        } else {
            let amount = ceil_px(container.rect.x2 + 40.0 - n.rect.x1);
            ("right", amount, "--dx", false)
        }
    } else if n.rect.center().y < container.rect.center().y {
        let amount = ceil_px(n.rect.y2 + 40.0 - container.rect.y1);
        ("up", amount, "--dy", true)
    } else {
        let amount = ceil_px(container.rect.y2 + 40.0 - n.rect.y1);
        ("down", amount, "--dy", false)
    }
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
