use std::collections::{HashMap, HashSet};

use crate::geometry::{ceil_px, Rect};
use crate::linter::{text_factor, MIN_GAP, TEXT_PAD_X, TEXT_PAD_Y, TEXT_SHAPES};
use crate::scene_graph::{zone_admits, Component, ComponentKind, ContainerKind, Scene, Target};
use crate::{Axis, Direction, PlaceAlign};

use super::Change;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RetainPolicy {
    Arrange,
    Position,
}

pub fn move_target(scene: &Scene, id: &str, dx: f64, dy: f64) -> anyhow::Result<Change> {
    let resolved = match scene.resolve(id)? {
        Target::Component(id) | Target::Container(id) | Target::Connector(id) => id,
    };
    let mut change = Change {
        moves: vec![(resolved, dx, dy)],
        resizes: vec![],
    };
    retain_zone_membership(scene, &mut change, RetainPolicy::Position);
    Ok(change)
}

pub fn align(scene: &Scene, ids: &[String], axis: Axis) -> anyhow::Result<Change> {
    if ids.len() < 2 {
        anyhow::bail!("align needs at least 2 ids");
    }
    let resolved = resolve_boxes(scene, ids)?;
    let anchor = resolved[0].1;
    let mut moves = Vec::new();
    for (id, rect) in resolved.into_iter().skip(1) {
        let (dx, dy) = match axis {
            Axis::Left => (anchor.x1 - rect.x1, 0.0),
            Axis::Center => (anchor.center().x - rect.center().x, 0.0),
            Axis::Right => (anchor.x2 - rect.x2, 0.0),
            Axis::Top => (0.0, anchor.y1 - rect.y1),
            Axis::Middle => (0.0, anchor.center().y - rect.center().y),
            Axis::Bottom => (0.0, anchor.y2 - rect.y2),
        };
        moves.push((id, dx, dy));
    }
    let mut change = Change {
        moves,
        resizes: vec![],
    };
    retain_zone_membership(scene, &mut change, RetainPolicy::Arrange);
    Ok(change)
}

fn resolve_boxes(scene: &Scene, ids: &[String]) -> anyhow::Result<Vec<(String, Rect)>> {
    let mut resolved = Vec::with_capacity(ids.len());
    for id in ids {
        let resolved_id = match scene.resolve(id)? {
            Target::Component(id) | Target::Container(id) => id,
            Target::Connector(_) => {
                anyhow::bail!("target must be a component or container")
            }
        };
        let rect = scene
            .rect_of(&resolved_id)
            .ok_or_else(|| anyhow::anyhow!("unknown id \"{resolved_id}\""))?;
        resolved.push((resolved_id, rect));
    }
    Ok(resolved)
}

pub fn distribute(
    scene: &Scene,
    ids: &[String],
    direction: Direction,
    gap: Option<f64>,
) -> anyhow::Result<Change> {
    if ids.len() < 2 {
        anyhow::bail!("distribute needs at least 2 ids");
    }
    let mut items = resolve_boxes(scene, ids)?;
    let horizontal = matches!(direction, Direction::Horizontal);
    // Stable sort by center then leading edge so fit-text widening cannot invert order.
    items.sort_by(|a, b| {
        let key = |rect: Rect| {
            if horizontal {
                (rect.center().x, rect.x1)
            } else {
                (rect.center().y, rect.y1)
            }
        };
        let (ac, al) = key(a.1);
        let (bc, bl) = key(b.1);
        ac.total_cmp(&bc).then(al.total_cmp(&bl))
    });

    let explicit = gap.is_some();
    let gap = match gap {
        Some(gap) => gap,
        None => {
            if items.len() < 3 {
                anyhow::bail!("distribute without --gap needs at least 3 ids");
            }
            let first = items[0].1;
            let last = items[items.len() - 1].1;
            let middle: f64 = items[1..items.len() - 1]
                .iter()
                .map(|(_, rect)| {
                    if horizontal {
                        rect.width()
                    } else {
                        rect.height()
                    }
                })
                .sum();
            let span = if horizontal {
                last.x1 - first.x2
            } else {
                last.y1 - first.y2
            };
            let computed = (span - middle) / (items.len() - 1) as f64;
            if computed < 0.0 {
                anyhow::bail!("not enough space to distribute; pass --gap");
            }
            computed
        }
    };

    // First never moves. Without --gap the last stays fixed too, so floating-point
    // rounding never writes a move for either end.
    let end = if explicit {
        items.len()
    } else {
        items.len() - 1
    };
    let mut prev = items[0].1;
    let mut moves = Vec::new();
    for (id, rect) in items.into_iter().take(end).skip(1) {
        let (dx, dy) = if horizontal {
            (prev.x2 + gap - rect.x1, 0.0)
        } else {
            (0.0, prev.y2 + gap - rect.y1)
        };
        moves.push((id, dx, dy));
        prev = rect.translate(dx, dy);
    }
    let mut change = Change {
        moves,
        resizes: vec![],
    };
    retain_zone_membership(scene, &mut change, RetainPolicy::Arrange);
    Ok(change)
}

#[allow(clippy::too_many_arguments)]
pub fn place(
    scene: &Scene,
    id: &str,
    right_of: Option<&str>,
    left_of: Option<&str>,
    below: Option<&str>,
    above: Option<&str>,
    gap: Option<f64>,
    align: PlaceAlign,
) -> anyhow::Result<Change> {
    let gap = gap.unwrap_or(40.0);
    let (moved_id, moved) = resolve_box(scene, id)?;
    let (dx, dy) = if let Some(anchor_id) = right_of {
        let (_, anchor) = resolve_box(scene, anchor_id)?;
        (
            anchor.x2 + gap - moved.x1,
            cross_delta(anchor, moved, false, &align),
        )
    } else if let Some(anchor_id) = left_of {
        let (_, anchor) = resolve_box(scene, anchor_id)?;
        (
            anchor.x1 - gap - moved.x2,
            cross_delta(anchor, moved, false, &align),
        )
    } else if let Some(anchor_id) = below {
        let (_, anchor) = resolve_box(scene, anchor_id)?;
        (
            cross_delta(anchor, moved, true, &align),
            anchor.y2 + gap - moved.y1,
        )
    } else if let Some(anchor_id) = above {
        let (_, anchor) = resolve_box(scene, anchor_id)?;
        (
            cross_delta(anchor, moved, true, &align),
            anchor.y1 - gap - moved.y2,
        )
    } else {
        anyhow::bail!("place needs exactly one of --right-of, --left-of, --below, --above");
    };
    let mut change = Change {
        moves: vec![(moved_id, dx, dy)],
        resizes: vec![],
    };
    retain_zone_membership(scene, &mut change, RetainPolicy::Position);
    Ok(change)
}

fn cross_delta(anchor: Rect, moved: Rect, horizontal: bool, align: &PlaceAlign) -> f64 {
    let (anchor_start, anchor_mid, anchor_end, moved_start, moved_mid, moved_end) = if horizontal {
        (
            anchor.x1,
            anchor.center().x,
            anchor.x2,
            moved.x1,
            moved.center().x,
            moved.x2,
        )
    } else {
        (
            anchor.y1,
            anchor.center().y,
            anchor.y2,
            moved.y1,
            moved.center().y,
            moved.y2,
        )
    };
    match align {
        PlaceAlign::Start => anchor_start - moved_start,
        PlaceAlign::Center => anchor_mid - moved_mid,
        PlaceAlign::End => anchor_end - moved_end,
    }
}

fn retain_zone_membership(scene: &Scene, change: &mut Change, policy: RetainPolicy) {
    let mut pinned = HashSet::new();
    for (id, _, _) in &change.moves {
        if scene.connector(id).is_none() {
            pinned.insert(id.clone());
        }
    }
    for (id, _) in &change.resizes {
        pinned.insert(id.clone());
    }

    let (expanded, _) = super::expand_moves(scene, &change.moves);
    let mut grown: HashMap<String, Rect> = scene
        .containers
        .iter()
        .filter(|container| container.kind == ContainerKind::Zone)
        .map(|container| (container.id.clone(), container.rect))
        .collect();

    for (id, dx, dy) in &expanded {
        if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
            continue;
        }
        let (rect, z, parent, k_is_zone) = if let Some(component) = scene.component(id) {
            (component.rect, component.z, component.parent.clone(), false)
        } else if let Some(container) = scene.container(id) {
            (
                container.rect,
                container.z,
                container.parent.clone(),
                container.kind == ContainerKind::Zone,
            )
        } else {
            continue;
        };
        let Some(parent_id) = parent else {
            continue;
        };
        let Some(parent_zone) = scene.container(&parent_id) else {
            continue;
        };
        if parent_zone.kind != ContainerKind::Zone {
            continue;
        }
        if pinned_or_ancestor(scene, &pinned, &parent_id) {
            continue;
        }
        let moved = rect.translate(*dx, *dy);
        if zone_admits(&parent_zone.rect, parent_zone.z, &moved, z, k_is_zone) {
            continue;
        }
        if policy == RetainPolicy::Position
            && (intentional_exit(
                scene,
                parent_zone.rect,
                &moved,
                z,
                k_is_zone,
                id,
                &parent_id,
            ))
        {
            continue;
        }
        grow_crossed(
            grown.get_mut(&parent_id).expect("zone seeded"),
            parent_zone.rect,
            moved,
            k_is_zone,
        );
    }

    let mut cascade: Vec<&crate::scene_graph::Container> = scene
        .containers
        .iter()
        .filter(|container| container.kind == ContainerKind::Zone)
        .collect();
    cascade.sort_by_key(|zone| std::cmp::Reverse(zone.z));
    for zone in cascade {
        let grown_p = grown[&zone.id];
        if grown_p == zone.rect {
            continue;
        }
        let Some(parent_id) = zone.parent.clone() else {
            continue;
        };
        let Some(parent_zone) = scene.container(&parent_id) else {
            continue;
        };
        if parent_zone.kind != ContainerKind::Zone {
            continue;
        }
        if pinned_or_ancestor(scene, &pinned, &parent_id) {
            continue;
        }
        let grown_q = grown[&parent_id];
        if zone_admits(&grown_q, parent_zone.z, &grown_p, zone.z, true) {
            continue;
        }
        grow_zone_edge(
            grown.get_mut(&parent_id).expect("parent seeded"),
            parent_zone.rect,
            grown_p,
        );
    }

    let mut resized: Vec<&crate::scene_graph::Container> = scene
        .containers
        .iter()
        .filter(|container| container.kind == ContainerKind::Zone)
        .filter(|container| grown[&container.id] != container.rect)
        .collect();
    resized.sort_by_key(|container| container.z);
    for zone in resized {
        change.resizes.push((zone.id.clone(), grown[&zone.id]));
    }
}

fn intentional_exit(
    scene: &Scene,
    parent_rect: Rect,
    moved: &Rect,
    z: usize,
    k_is_zone: bool,
    moved_id: &str,
    parent_id: &str,
) -> bool {
    if !parent_rect.overlaps(moved) {
        return true;
    }
    scene.containers.iter().any(|candidate| {
        candidate.kind == ContainerKind::Zone
            && candidate.id != moved_id
            && candidate.id != parent_id
            && !is_descendant(scene, moved_id, &candidate.id)
            && !is_ancestor(scene, parent_id, &candidate.id)
            && zone_admits(&candidate.rect, candidate.z, moved, z, k_is_zone)
    })
}

fn pinned_or_ancestor(scene: &Scene, pinned: &HashSet<String>, id: &str) -> bool {
    let mut current = Some(id.to_string());
    let mut seen = HashSet::new();
    while let Some(node) = current {
        if pinned.contains(&node) {
            return true;
        }
        if !seen.insert(node.clone()) {
            return false;
        }
        current = scene
            .container(&node)
            .and_then(|container| container.parent.clone());
    }
    false
}

fn is_ancestor(scene: &Scene, ancestor: &str, start: &str) -> bool {
    let mut current = Some(start.to_string());
    let mut seen = HashSet::new();
    while let Some(node) = current {
        if node == ancestor {
            return true;
        }
        if !seen.insert(node.clone()) {
            return false;
        }
        current = scene
            .container(&node)
            .and_then(|container| container.parent.clone())
            .or_else(|| {
                scene
                    .component(&node)
                    .and_then(|component| component.parent.clone())
            });
    }
    false
}

fn is_descendant(scene: &Scene, ancestor: &str, id: &str) -> bool {
    id != ancestor && is_ancestor(scene, ancestor, id)
}

fn grow_crossed(grown: &mut Rect, original: Rect, moved: Rect, k_is_zone: bool) {
    let center = moved.center();
    if center.x < original.x1 || (k_is_zone && moved.x1 < original.x1) {
        grown.x1 = grown.x1.min(moved.x1 - f64::from(MIN_GAP));
    }
    if center.x > original.x2 || (k_is_zone && moved.x2 > original.x2) {
        grown.x2 = grown.x2.max(moved.x2 + f64::from(MIN_GAP));
    }
    if center.y < original.y1 || (k_is_zone && moved.y1 < original.y1) {
        grown.y1 = grown.y1.min(moved.y1 - f64::from(MIN_GAP));
    }
    if center.y > original.y2 || (k_is_zone && moved.y2 > original.y2) {
        grown.y2 = grown.y2.max(moved.y2 + f64::from(MIN_GAP));
    }
}

fn grow_zone_edge(grown_q: &mut Rect, q_rect: Rect, grown_p: Rect) {
    if grown_p.x1 < q_rect.x1 {
        grown_q.x1 = grown_q.x1.min(grown_p.x1 - f64::from(MIN_GAP));
    }
    if grown_p.x2 > q_rect.x2 {
        grown_q.x2 = grown_q.x2.max(grown_p.x2 + f64::from(MIN_GAP));
    }
    if grown_p.y1 < q_rect.y1 {
        grown_q.y1 = grown_q.y1.min(grown_p.y1 - f64::from(MIN_GAP));
    }
    if grown_p.y2 > q_rect.y2 {
        grown_q.y2 = grown_q.y2.max(grown_p.y2 + f64::from(MIN_GAP));
    }
}

fn resolve_box(scene: &Scene, id: &str) -> anyhow::Result<(String, Rect)> {
    let resolved = resolve_boxes(scene, &[id.to_string()])?;
    Ok(resolved.into_iter().next().expect("one id"))
}

pub fn fit_container(
    scene: &Scene,
    id: &str,
    padding: Option<f64>,
    expand_only: bool,
) -> anyhow::Result<Change> {
    let Target::Container(container_id) = scene.resolve(id)? else {
        anyhow::bail!("{id} is not a container");
    };
    let container = scene
        .container(&container_id)
        .ok_or_else(|| anyhow::anyhow!("unknown id \"{container_id}\""))?;
    let padding = padding.unwrap_or(40.0);
    let kids = crate::spatial::children(scene, &container_id);
    let mut iter = kids.into_iter();
    let Some((_, first)) = iter.next() else {
        anyhow::bail!("container {container_id} has no children");
    };
    let union = iter.fold(first, |acc, (_, rect)| acc.union(&rect));
    let mut new_rect = union.inflate(padding);
    if container.kind == ContainerKind::Zone && container.title_element.is_some() {
        new_rect.y1 -= container.title_height.unwrap_or(0.0) + 10.0;
    }
    if expand_only {
        new_rect = container.rect.union(&new_rect);
    }
    Ok(Change {
        moves: vec![],
        resizes: vec![(container_id, new_rect)],
    })
}

pub fn fit_text(
    scene: &Scene,
    id: Option<&str>,
    all: bool,
    padding_x: Option<f64>,
    padding_y: Option<f64>,
) -> anyhow::Result<Change> {
    let px = padding_x.unwrap_or(24.0);
    let py = padding_y.unwrap_or(16.0);
    let targets: Vec<&Component> = if let Some(id) = id {
        let Target::Component(component_id) = scene.resolve(id)? else {
            anyhow::bail!("{id} is not a text shape");
        };
        let component = scene
            .component(&component_id)
            .ok_or_else(|| anyhow::anyhow!("unknown id \"{component_id}\""))?;
        if !is_fit_text_eligible(component) {
            anyhow::bail!("{component_id} is not a text shape");
        }
        vec![component]
    } else if all {
        scene
            .components
            .iter()
            .filter(|component| is_fit_text_eligible(component))
            .collect()
    } else {
        anyhow::bail!("fit-text needs --id or --all");
    };

    let mut resizes = Vec::new();
    for component in targets {
        let Some(t) = component.label_size.as_ref() else {
            continue;
        };
        let f = text_factor(&component.type_name);
        let w = component.rect.width();
        let h = component.rect.height();
        if t.width <= w * f - f64::from(TEXT_PAD_X) && t.height <= h * f - f64::from(TEXT_PAD_Y) {
            continue;
        }
        let new_w = w.max(ceil_px((t.width + 2.0 * px) / f));
        let new_h = h.max(ceil_px((t.height + 2.0 * py) / f));
        let center = component.rect.center();
        let new_rect =
            Rect::from_xywh(center.x - new_w / 2.0, center.y - new_h / 2.0, new_w, new_h);
        resizes.push((component.id.clone(), new_rect));
    }
    Ok(Change {
        moves: vec![],
        resizes,
    })
}

fn is_fit_text_eligible(component: &Component) -> bool {
    component.kind == ComponentKind::Shape
        && TEXT_SHAPES.contains(&component.type_name.as_str())
        && component.label_element.is_some()
        && component.label_size.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene_graph::Scene;
    use crate::schema::Document;

    fn scene_of(src: &str) -> (Document, Scene) {
        let doc = Document::parse(src).expect("parse");
        let scene = Scene::build(&doc);
        (doc, scene)
    }

    fn two_boxes() -> (Document, Scene) {
        scene_of(
            r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "A", "x": 10, "y": 20, "width": 100, "height": 40},
    {"type": "rectangle", "id": "B", "x": 200, "y": 80, "width": 50, "height": 30}
  ]
}"#,
        )
    }

    fn moved(scene: &Scene, ids: &[&str], axis: Axis) -> (f64, f64) {
        let change = align(
            scene,
            &ids.iter().map(|id| (*id).to_string()).collect::<Vec<_>>(),
            axis,
        )
        .expect("align");
        assert_eq!(change.moves.len(), 1, "{:?}", change.moves);
        let (id, dx, dy) = &change.moves[0];
        assert_eq!(id, "B");
        (*dx, *dy)
    }

    #[test]
    fn align_left_matches_anchor_x1() {
        let (_doc, scene) = two_boxes();
        let (dx, dy) = moved(&scene, &["A", "B"], Axis::Left);
        assert_eq!((dx, dy), (10.0 - 200.0, 0.0));
    }

    #[test]
    fn align_center_matches_anchor_center_x() {
        let (_doc, scene) = two_boxes();
        let (dx, dy) = moved(&scene, &["A", "B"], Axis::Center);
        // A center.x = 60, B center.x = 225
        assert_eq!((dx, dy), (60.0 - 225.0, 0.0));
    }

    #[test]
    fn align_right_matches_anchor_x2() {
        let (_doc, scene) = two_boxes();
        let (dx, dy) = moved(&scene, &["A", "B"], Axis::Right);
        // A.x2 = 110, B.x2 = 250
        assert_eq!((dx, dy), (110.0 - 250.0, 0.0));
    }

    #[test]
    fn align_top_matches_anchor_y1() {
        let (_doc, scene) = two_boxes();
        let (dx, dy) = moved(&scene, &["A", "B"], Axis::Top);
        assert_eq!((dx, dy), (0.0, 20.0 - 80.0));
    }

    #[test]
    fn align_bottom_matches_anchor_y2() {
        let (_doc, scene) = two_boxes();
        let (dx, dy) = moved(&scene, &["A", "B"], Axis::Bottom);
        // A.y2 = 60, B.y2 = 110
        assert_eq!((dx, dy), (0.0, 60.0 - 110.0));
    }

    fn placed(
        scene: &Scene,
        left_of: Option<&str>,
        above: Option<&str>,
        align: PlaceAlign,
    ) -> (f64, f64) {
        let change =
            place(scene, "B", None, left_of, None, above, Some(40.0), align).expect("place");
        assert_eq!(change.moves.len(), 1);
        let (id, dx, dy) = &change.moves[0];
        assert_eq!(id, "B");
        (*dx, *dy)
    }

    #[test]
    fn place_left_of_sets_x2_and_centers_vertically() {
        let (_doc, scene) = two_boxes();
        let (dx, dy) = placed(&scene, Some("A"), None, PlaceAlign::Center);
        // B.x2 = A.x1 - 40 => 250 + dx = 10 - 40 => dx = -280
        // centers: A.cy = 40, B.cy = 95 => dy = -55
        assert_eq!((dx, dy), (-280.0, -55.0));
    }

    #[test]
    fn place_above_sets_y2_and_centers_horizontally() {
        let (_doc, scene) = two_boxes();
        let (dx, dy) = placed(&scene, None, Some("A"), PlaceAlign::Center);
        // B.y2 = A.y1 - 40 => 110 + dy = 20 - 40 => dy = -130
        // centers: A.cx = 60, B.cx = 225 => dx = -165
        assert_eq!((dx, dy), (-165.0, -130.0));
    }

    #[test]
    fn place_align_start_matches_leading_edge() {
        let (_doc, scene) = two_boxes();
        let (dx, dy) = placed(&scene, Some("A"), None, PlaceAlign::Start);
        // left-of, start aligns y1: dy = 20 - 80 = -60; dx still -280
        assert_eq!((dx, dy), (-280.0, -60.0));
    }

    fn three_boxes() -> (Document, Scene) {
        scene_of(
            r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "A", "x": 0, "y": 0, "width": 40, "height": 20},
    {"type": "rectangle", "id": "B", "x": 10, "y": 0, "width": 40, "height": 20},
    {"type": "rectangle", "id": "C", "x": 20, "y": 0, "width": 40, "height": 20}
  ]
}"#,
        )
    }

    #[test]
    fn distribute_overlapping_boxes_without_gap_needs_space() {
        let (_doc, scene) = three_boxes();
        let err = distribute(
            &scene,
            &["A".to_string(), "B".to_string(), "C".to_string()],
            Direction::Horizontal,
            None,
        )
        .expect_err("overlapping boxes have no free gap");
        assert_eq!(
            err.to_string(),
            "not enough space to distribute; pass --gap"
        );
    }

    #[test]
    fn place_align_end_matches_trailing_edge() {
        let (_doc, scene) = two_boxes();
        let (dx, dy) = placed(&scene, None, Some("A"), PlaceAlign::End);
        // above, end aligns x2: A.x2 = 110, B.x2 = 250 => dx = -140; dy still -130
        assert_eq!((dx, dy), (-140.0, -130.0));
    }

    #[test]
    fn distribute_horizontal_sorts_by_center_then_leading_edge() {
        // A: x=40,w=140 → cx=110, x1=40. Widened B: x=30,w=340 → cx=200, x1=30.
        // A leading-edge sort would put B first (x1=30) and move A; center order keeps A.
        let (_doc, scene) = scene_of(
            r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "A", "x": 40, "y": 100, "width": 140, "height": 80},
    {"type": "rectangle", "id": "B", "x": 30, "y": 100, "width": 340, "height": 80}
  ]
}"#,
        );
        let change = distribute(
            &scene,
            &["A".to_string(), "B".to_string()],
            Direction::Horizontal,
            Some(40.0),
        )
        .expect("distribute");
        let move_of = |id: &str| {
            change
                .moves
                .iter()
                .find(|(moved_id, _, _)| moved_id == id)
                .map(|(_, dx, dy)| (*dx, *dy))
        };
        assert!(
            move_of("A").is_none() || move_of("A") == Some((0.0, 0.0)),
            "A must stay put, got {:?}",
            change.moves
        );
        let (dx, dy) = move_of("B").expect("B should move");
        assert!(
            (dx - 190.0).abs() < 1e-6,
            "B.x becomes 220, dx=190, got {dx}"
        );
        assert!(
            (dy - 0.0).abs() < 1e-6,
            "B must not move vertically, dy={dy}"
        );
    }
}
