use std::collections::HashMap;

use crate::geometry::Rect;
use crate::linter::NEAR_MISS_MAX;
use crate::scene_graph::{Component, ComponentKind, Scene};

const ALIGN_PX: f64 = 0.5;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Neighbor {
    pub id: String,
    pub gap: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Neighbors {
    pub left: Option<Neighbor>,
    pub right: Option<Neighbor>,
    pub top: Option<Neighbor>,
    pub bottom: Option<Neighbor>,
}

pub fn neighbors(c: &Component, siblings: &[&Component]) -> Neighbors {
    Neighbors {
        left: nearest(c, siblings, Direction::Left),
        right: nearest(c, siblings, Direction::Right),
        top: nearest(c, siblings, Direction::Top),
        bottom: nearest(c, siblings, Direction::Bottom),
    }
}

enum Direction {
    Left,
    Right,
    Top,
    Bottom,
}

fn nearest(c: &Component, siblings: &[&Component], dir: Direction) -> Option<Neighbor> {
    let mut best: Option<(&Component, f64, f64)> = None;
    for o in siblings {
        if o.id == c.id {
            continue;
        }
        let Some(gap) = directional_gap(c, o, &dir) else {
            continue;
        };
        let offset = match dir {
            Direction::Left | Direction::Right => (o.rect.center().y - c.rect.center().y).abs(),
            Direction::Top | Direction::Bottom => (o.rect.center().x - c.rect.center().x).abs(),
        };
        let better = match best {
            None => true,
            Some((best_o, best_gap, best_offset)) => gap
                .total_cmp(&best_gap)
                .then(offset.total_cmp(&best_offset))
                .then(o.z.cmp(&best_o.z))
                .is_lt(),
        };
        if better {
            best = Some((o, gap, offset));
        }
    }
    best.map(|(o, gap, _)| Neighbor {
        id: o.id.clone(),
        gap,
    })
}

fn directional_gap(c: &Component, o: &Component, dir: &Direction) -> Option<f64> {
    match dir {
        Direction::Right => {
            let overlap = o.rect.y2.min(c.rect.y2) - o.rect.y1.max(c.rect.y1);
            if overlap > 0.0 && o.rect.center().x > c.rect.center().x {
                Some(o.rect.x1 - c.rect.x2)
            } else {
                None
            }
        }
        Direction::Left => {
            let overlap = o.rect.y2.min(c.rect.y2) - o.rect.y1.max(c.rect.y1);
            if overlap > 0.0 && o.rect.center().x < c.rect.center().x {
                Some(c.rect.x1 - o.rect.x2)
            } else {
                None
            }
        }
        Direction::Bottom => {
            let overlap = o.rect.x2.min(c.rect.x2) - o.rect.x1.max(c.rect.x1);
            if overlap > 0.0 && o.rect.center().y > c.rect.center().y {
                Some(o.rect.y1 - c.rect.y2)
            } else {
                None
            }
        }
        Direction::Top => {
            let overlap = o.rect.x2.min(c.rect.x2) - o.rect.x1.max(c.rect.x1);
            if overlap > 0.0 && o.rect.center().y < c.rect.center().y {
                Some(c.rect.y1 - o.rect.y2)
            } else {
                None
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Track {
    pub parent: Option<String>,
    pub members: Vec<String>,
    pub gaps: Vec<f64>,
    pub pos: f64,
}

pub fn sibling_container_rects(scene: &Scene, parent: Option<&str>) -> Vec<Rect> {
    scene
        .containers
        .iter()
        .filter(|c| c.parent.as_deref() == parent)
        .map(|c| c.rect)
        .collect()
}

pub fn rows(siblings: &[&Component]) -> Vec<Track> {
    rows_with_obstacles(siblings, &[])
}

pub fn columns(siblings: &[&Component]) -> Vec<Track> {
    columns_with_obstacles(siblings, &[])
}

pub fn rows_with_obstacles(siblings: &[&Component], obstacles: &[Rect]) -> Vec<Track> {
    tracks_with_obstacles(siblings, obstacles, Axis::Row)
}

pub fn columns_with_obstacles(siblings: &[&Component], obstacles: &[Rect]) -> Vec<Track> {
    tracks_with_obstacles(siblings, obstacles, Axis::Column)
}

fn tracks_with_obstacles(siblings: &[&Component], obstacles: &[Rect], axis: Axis) -> Vec<Track> {
    let raw_rows = cluster(siblings, Axis::Row);
    let raw_cols = cluster(siblings, Axis::Column);
    let (tracks, orthogonal) = match axis {
        Axis::Row => (raw_rows, raw_cols),
        Axis::Column => (raw_cols, raw_rows),
    };
    let tracks = split_tracks(siblings, tracks, &orthogonal, axis);
    if obstacles.is_empty() {
        tracks
    } else {
        split_tracks_by_obstacles(siblings, tracks, obstacles, axis)
    }
}

#[derive(Clone, Copy)]
enum Axis {
    Row,
    Column,
}

fn cluster(siblings: &[&Component], axis: Axis) -> Vec<Track> {
    if siblings.is_empty() {
        return Vec::new();
    }
    let parent = siblings.first().and_then(|c| c.parent.clone());
    let mut ordered: Vec<&Component> = siblings.to_vec();
    ordered.sort_by(|a, b| match axis {
        Axis::Row => a
            .rect
            .center()
            .y
            .total_cmp(&b.rect.center().y)
            .then(a.rect.x1.total_cmp(&b.rect.x1)),
        Axis::Column => a
            .rect
            .center()
            .x
            .total_cmp(&b.rect.center().x)
            .then(a.rect.y1.total_cmp(&b.rect.y1)),
    });

    let mut groups: Vec<Vec<&Component>> = Vec::new();
    for n in ordered {
        if let Some(current) = groups.last_mut() {
            let (lo, hi, min_h) = track_range(current, &axis);
            let (n_lo, n_hi) = member_range(n, &axis);
            let overlap = n_hi.min(hi) - n_lo.max(lo);
            let n_size = n_hi - n_lo;
            if overlap >= 0.5 * n_size.min(min_h) {
                current.push(n);
                continue;
            }
        }
        groups.push(vec![n]);
    }

    groups
        .into_iter()
        .filter(|members| members.len() >= 2)
        .map(|mut members| {
            members.sort_by(|a, b| match axis {
                Axis::Row => a.rect.x1.total_cmp(&b.rect.x1),
                Axis::Column => a.rect.y1.total_cmp(&b.rect.y1),
            });
            track_from_members(&members, parent.clone(), axis)
        })
        .collect()
}

fn track_from_members(members: &[&Component], parent: Option<String>, axis: Axis) -> Track {
    let gaps: Vec<f64> = members
        .windows(2)
        .map(|pair| match axis {
            Axis::Row => pair[1].rect.x1 - pair[0].rect.x2,
            Axis::Column => pair[1].rect.y1 - pair[0].rect.y2,
        })
        .collect();
    let pos = members
        .iter()
        .map(|c| match axis {
            Axis::Row => c.rect.center().y,
            Axis::Column => c.rect.center().x,
        })
        .sum::<f64>()
        / members.len() as f64;
    Track {
        parent,
        members: members.iter().map(|c| c.id.clone()).collect(),
        gaps,
        pos,
    }
}

fn split_tracks(
    siblings: &[&Component],
    tracks: Vec<Track>,
    orthogonal: &[Track],
    axis: Axis,
) -> Vec<Track> {
    let lookup: HashMap<&str, &Component> = siblings.iter().map(|c| (c.id.as_str(), *c)).collect();
    tracks
        .into_iter()
        .flat_map(|track| split_one(track, orthogonal, &lookup, axis))
        .collect()
}

fn split_tracks_by_obstacles(
    siblings: &[&Component],
    tracks: Vec<Track>,
    obstacles: &[Rect],
    axis: Axis,
) -> Vec<Track> {
    let lookup: HashMap<&str, &Component> = siblings.iter().map(|c| (c.id.as_str(), *c)).collect();
    tracks
        .into_iter()
        .flat_map(|track| split_one_by_obstacles(track, obstacles, &lookup, axis))
        .collect()
}

fn obstacle_between(a: &Component, b: &Component, obstacle: &Rect, axis: Axis) -> bool {
    match axis {
        Axis::Row => {
            obstacle.x1 < b.rect.x1
                && obstacle.x2 > a.rect.x2
                && obstacle.y1 < a.rect.y2.min(b.rect.y2)
                && obstacle.y2 > a.rect.y1.max(b.rect.y1)
        }
        Axis::Column => {
            obstacle.y1 < b.rect.y1
                && obstacle.y2 > a.rect.y2
                && obstacle.x1 < a.rect.x2.min(b.rect.x2)
                && obstacle.x2 > a.rect.x1.max(b.rect.x1)
        }
    }
}

fn split_one_by_obstacles(
    track: Track,
    obstacles: &[Rect],
    lookup: &HashMap<&str, &Component>,
    axis: Axis,
) -> Vec<Track> {
    if track.members.len() < 2 {
        return Vec::new();
    }
    let members: Vec<&Component> = track.members.iter().map(|id| lookup[id.as_str()]).collect();
    let mut cut_after = vec![false; members.len().saturating_sub(1)];
    for i in 0..members.len().saturating_sub(1) {
        if obstacles
            .iter()
            .any(|obstacle| obstacle_between(members[i], members[i + 1], obstacle, axis))
        {
            cut_after[i] = true;
        }
    }
    if !cut_after.iter().any(|&cut| cut) {
        return vec![track];
    }
    let mut out = Vec::new();
    let mut start = 0;
    for (i, cut) in cut_after.iter().enumerate() {
        if *cut {
            if i + 1 - start >= 2 {
                out.push(track_from_members(
                    &members[start..=i],
                    track.parent.clone(),
                    axis,
                ));
            }
            start = i + 1;
        }
    }
    if members.len() - start >= 2 {
        out.push(track_from_members(
            &members[start..],
            track.parent.clone(),
            axis,
        ));
    }
    out
}

fn split_one(
    track: Track,
    orthogonal: &[Track],
    lookup: &HashMap<&str, &Component>,
    axis: Axis,
) -> Vec<Track> {
    if track.members.len() < 3 {
        return vec![track];
    }
    let mut remove = vec![false; track.members.len()];
    for (k, flag) in remove.iter_mut().enumerate() {
        if orthogonally_locked(&track.members[k], orthogonal, lookup, axis)
            && every_adjacent_misaligned(&track.members, k, lookup, axis)
        {
            *flag = true;
        }
    }
    if !remove.iter().any(|&r| r) {
        return vec![track];
    }
    let mut out = Vec::new();
    let mut start = 0;
    while start < track.members.len() {
        if remove[start] {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < track.members.len() && !remove[end] {
            end += 1;
        }
        if end - start >= 2 {
            let slice: Vec<&Component> = track.members[start..end]
                .iter()
                .map(|id| lookup[id.as_str()])
                .collect();
            out.push(track_from_members(&slice, track.parent.clone(), axis));
        }
        start = end;
    }
    out
}

fn orthogonally_locked(
    id: &str,
    orthogonal: &[Track],
    lookup: &HashMap<&str, &Component>,
    axis: Axis,
) -> bool {
    let Some(n) = lookup.get(id) else {
        return false;
    };
    for other in orthogonal {
        if other.members.len() < 2 {
            continue;
        }
        let Some(idx) = other.members.iter().position(|m| m == id) else {
            continue;
        };
        let prev = idx
            .checked_sub(1)
            .and_then(|i| lookup.get(other.members[i].as_str()).copied());
        let next = other
            .members
            .get(idx + 1)
            .and_then(|m| lookup.get(m.as_str()).copied());
        if [prev, next]
            .into_iter()
            .flatten()
            .any(|adj| lock_aligned(n, adj, axis))
        {
            return true;
        }
    }
    false
}

fn every_adjacent_misaligned(
    members: &[String],
    k: usize,
    lookup: &HashMap<&str, &Component>,
    axis: Axis,
) -> bool {
    let Some(n) = lookup.get(members[k].as_str()) else {
        return false;
    };
    let mut saw_neighbor = false;
    if k > 0 {
        saw_neighbor = true;
        let adj = lookup[members[k - 1].as_str()];
        if !fully_misaligned(n, adj, axis) {
            return false;
        }
    }
    if k + 1 < members.len() {
        saw_neighbor = true;
        let adj = lookup[members[k + 1].as_str()];
        if !fully_misaligned(n, adj, axis) {
            return false;
        }
    }
    saw_neighbor
}

fn lock_aligned(n: &Component, adj: &Component, axis: Axis) -> bool {
    match axis {
        Axis::Row => {
            (n.rect.center().x - adj.rect.center().x).abs() < ALIGN_PX
                || (n.rect.x1 - adj.rect.x1).abs() < ALIGN_PX
                || (n.rect.x2 - adj.rect.x2).abs() < ALIGN_PX
        }
        Axis::Column => {
            (n.rect.center().y - adj.rect.center().y).abs() < ALIGN_PX
                || (n.rect.y1 - adj.rect.y1).abs() < ALIGN_PX
                || (n.rect.y2 - adj.rect.y2).abs() < ALIGN_PX
        }
    }
}

fn fully_misaligned(n: &Component, adj: &Component, axis: Axis) -> bool {
    let limit = f64::from(NEAR_MISS_MAX);
    match axis {
        Axis::Row => {
            (n.rect.center().y - adj.rect.center().y).abs() > limit
                && (n.rect.y1 - adj.rect.y1).abs() > limit
                && (n.rect.y2 - adj.rect.y2).abs() > limit
        }
        Axis::Column => {
            (n.rect.center().x - adj.rect.center().x).abs() > limit
                && (n.rect.x1 - adj.rect.x1).abs() > limit
                && (n.rect.x2 - adj.rect.x2).abs() > limit
        }
    }
}

fn member_range(c: &Component, axis: &Axis) -> (f64, f64) {
    match axis {
        Axis::Row => (c.rect.y1, c.rect.y2),
        Axis::Column => (c.rect.x1, c.rect.x2),
    }
}

fn track_range(members: &[&Component], axis: &Axis) -> (f64, f64, f64) {
    let mut lo = f64::NEG_INFINITY;
    let mut hi = f64::INFINITY;
    let mut min_h = f64::INFINITY;
    for c in members {
        let (a, b) = member_range(c, axis);
        lo = lo.max(a);
        hi = hi.min(b);
        min_h = min_h.min(b - a);
    }
    (lo, hi, min_h)
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Padding {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

pub fn children(scene: &Scene, container_id: &str) -> Vec<(String, Rect)> {
    let mut components: Vec<(usize, String, Rect)> = scene
        .components
        .iter()
        .filter(|c| c.parent.as_deref() == Some(container_id))
        .map(|c| (c.z, c.id.clone(), c.rect))
        .collect();
    components.sort_by_key(|(z, _, _)| *z);
    let mut containers: Vec<(usize, String, Rect)> = scene
        .containers
        .iter()
        .filter(|c| c.parent.as_deref() == Some(container_id))
        .map(|c| (c.z, c.id.clone(), c.rect))
        .collect();
    containers.sort_by_key(|(z, _, _)| *z);
    components
        .into_iter()
        .chain(containers)
        .map(|(_, id, rect)| (id, rect))
        .collect()
}

pub fn ripple_targets(scene: &Scene, id: &str, dx: f64, dy: f64) -> Vec<String> {
    if dx.abs() <= 1e-9 && dy.abs() <= 1e-9 {
        return Vec::new();
    }
    let (anchor_rect, parent) = if let Some(component) = scene.component(id) {
        if component.kind == ComponentKind::Text {
            return Vec::new();
        }
        (component.rect, component.parent.clone())
    } else if let Some(container) = scene.container(id) {
        (container.rect, container.parent.clone())
    } else {
        return Vec::new();
    };
    let horizontal = dx.abs() >= dy.abs();
    let mut hits: Vec<(usize, String)> = Vec::new();
    for component in scene.nodes() {
        if component.id == id || component.parent != parent {
            continue;
        }
        if component.kind != ComponentKind::Shape && component.kind != ComponentKind::Group {
            continue;
        }
        if downstream(&component.rect, &anchor_rect, dx, dy, horizontal) {
            hits.push((component.z, component.id.clone()));
        }
    }
    for container in &scene.containers {
        if container.id == id || container.parent != parent {
            continue;
        }
        if downstream(&container.rect, &anchor_rect, dx, dy, horizontal) {
            hits.push((container.z, container.id.clone()));
        }
    }
    hits.sort_by_key(|(z, _)| *z);
    hits.into_iter().map(|(_, id)| id).collect()
}

fn downstream(rect: &Rect, anchor: &Rect, dx: f64, dy: f64, horizontal: bool) -> bool {
    if horizontal {
        let overlaps = rect.y1 < anchor.y2 && anchor.y1 < rect.y2;
        let center = rect.center().x;
        let anchor_center = anchor.center().x;
        overlaps
            && if dx > 0.0 {
                center > anchor_center + 0.5
            } else {
                center < anchor_center - 0.5
            }
    } else {
        let overlaps = rect.x1 < anchor.x2 && anchor.x1 < rect.x2;
        let center = rect.center().y;
        let anchor_center = anchor.center().y;
        overlaps
            && if dy > 0.0 {
                center > anchor_center + 0.5
            } else {
                center < anchor_center - 0.5
            }
    }
}

pub fn padding(scene: &Scene, container_id: &str) -> Option<Padding> {
    let container = scene.container(container_id)?;
    let kids = children(scene, container_id);
    let mut iter = kids.into_iter();
    let (_, first) = iter.next()?;
    let union = iter.fold(first, |acc, (_, rect)| acc.union(&rect));
    Some(Padding {
        top: union.y1 - container.rect.y1,
        right: container.rect.x2 - union.x2,
        bottom: container.rect.y2 - union.y2,
        left: union.x1 - container.rect.x1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene_graph::Scene;
    use crate::schema::Document;

    fn load(name: &str) -> Scene {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(format!("{name}.excalidraw"));
        let src = std::fs::read_to_string(path).expect("fixture");
        let doc = Document::parse(&src).expect("parse fixture");
        Scene::build(&doc)
    }

    fn approx_eq(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    fn sibling_refs(scene: &Scene) -> Vec<&Component> {
        scene.components.iter().collect()
    }

    fn shape(id: &str, rect: (f64, f64, f64, f64), z: usize) -> Component {
        Component {
            id: id.to_string(),
            kind: crate::scene_graph::ComponentKind::Shape,
            type_name: "rectangle".to_string(),
            rect: Rect {
                x1: rect.0,
                y1: rect.1,
                x2: rect.2,
                y2: rect.3,
            },
            label: None,
            label_element: None,
            label_size: None,
            elements: vec![id.to_string()],
            parent: None,
            frame_id: None,
            z,
        }
    }

    fn shape_xywh(id: &str, x: f64, y: f64, w: f64, h: f64, z: usize) -> Component {
        let rect = Rect::from_xywh(x, y, w, h);
        shape(id, (rect.x1, rect.y1, rect.x2, rect.y2), z)
    }

    #[test]
    fn basic_flow_api_neighbors() {
        let scene = load("basic_flow");
        let api = scene.component("api").expect("api");
        let siblings = sibling_refs(&scene);
        let n = neighbors(api, &siblings);

        let left = n.left.expect("left");
        assert_eq!(left.id, "client");
        approx_eq(left.gap, 100.0);

        let right = n.right.expect("right");
        assert_eq!(right.id, "svc");
        approx_eq(right.gap, 100.0);

        assert!(n.top.is_none());
        assert!(n.bottom.is_none());
    }

    #[test]
    fn basic_flow_top_level_one_row_no_columns() {
        let scene = load("basic_flow");
        let siblings = sibling_refs(&scene);
        let rs = rows(&siblings);
        assert_eq!(rs.len(), 1);
        assert!(rs[0].parent.is_none());
        assert_eq!(rs[0].members, ["client", "api", "svc"]);
        assert_eq!(rs[0].gaps, [100.0, 100.0]);
        approx_eq(rs[0].pos, 140.0);

        let cs = columns(&siblings);
        assert!(cs.is_empty());
    }

    #[test]
    fn text_layout_one_row_and_one_column() {
        let scene = load("text_layout");
        assert_eq!(scene.components.len(), 4);
        let siblings = sibling_refs(&scene);

        let rs = rows(&siblings);
        assert_eq!(rs.len(), 1);
        assert_eq!(rs[0].members, ["n1", "n2", "n3"]);
        assert_eq!(rs[0].gaps, [60.0, 20.0]);
        approx_eq(rs[0].pos, 425.0 / 3.0);

        let cs = columns(&siblings);
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].members, ["n1", "n4"]);
        assert_eq!(cs[0].gaps, [80.0]);
        approx_eq(cs[0].pos, 80.0);
    }

    #[test]
    fn singleton_siblings_produce_no_tracks() {
        let scene = load("basic_flow");
        let client = scene.component("client").expect("client");
        let siblings = [client];
        assert!(rows(&siblings).is_empty());
        assert!(columns(&siblings).is_empty());
    }

    #[test]
    fn overlap_row_has_negative_gap() {
        let scene = load("overlap");
        let siblings = sibling_refs(&scene);
        let rs = rows(&siblings);
        assert_eq!(rs.len(), 1);
        assert_eq!(rs[0].members, ["api", "auth"]);
        assert_eq!(rs[0].gaps, [-24.0]);

        let api = scene.component("api").expect("api");
        let n = neighbors(api, &siblings);
        let right = n.right.expect("right");
        assert_eq!(right.id, "auth");
        approx_eq(right.gap, -24.0);
        let bottom = n.bottom.expect("bottom");
        assert_eq!(bottom.id, "auth");
        approx_eq(bottom.gap, -60.0);
    }

    fn assert_child(child: &(String, Rect), id: &str, rect: (f64, f64, f64, f64)) {
        assert_eq!(child.0, id);
        approx_eq(child.1.x1, rect.0);
        approx_eq(child.1.y1, rect.1);
        approx_eq(child.1.x2, rect.2);
        approx_eq(child.1.y2, rect.3);
    }

    #[test]
    fn swimlanes_padding_and_children() {
        let scene = load("swimlanes");
        let lane_fe = children(&scene, "lane_fe");
        assert_eq!(lane_fe.len(), 2);
        assert_child(&lane_fe[0], "web", (40.0, 110.0, 180.0, 190.0));
        assert_child(&lane_fe[1], "mobile", (220.0, 110.0, 360.0, 190.0));

        let lane_be = children(&scene, "lane_be");
        assert_eq!(
            lane_be
                .iter()
                .map(|(id, _)| id.as_str())
                .collect::<Vec<_>>(),
            ["api", "db"]
        );

        let pad = padding(&scene, "lane_be").expect("lane_be padding");
        approx_eq(pad.top, 110.0);
        approx_eq(pad.right, -20.0);
        approx_eq(pad.bottom, 110.0);
        approx_eq(pad.left, 40.0);
    }

    #[test]
    fn padding_none_for_empty_or_unknown_container() {
        let scene = load("basic_flow");
        assert!(padding(&scene, "missing").is_none());
        assert!(children(&scene, "missing").is_empty());
        assert!(padding(&scene, "client").is_none());
    }

    #[test]
    fn icon_group_frame_padding_and_children() {
        let scene = load("icon_group");
        let frame_id = "Fr8Mw2Nq5Xs1Lp4Kv7RbZ";
        let kids = children(&scene, frame_id);
        assert_eq!(
            kids.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
            [
                "group:kq3Xv9TzL2mPw8RbN5cYd",
                "Mx9Cv2Bn5Lk8Jh1Gf4DsA",
                "Ab1Cd2Ef3Gh4Ij5Kl6MnO"
            ]
        );
        assert_child(
            &kids[0],
            "group:kq3Xv9TzL2mPw8RbN5cYd",
            (58.4, 80.0, 141.6, 209.0),
        );
        assert_child(
            &kids[1],
            "Mx9Cv2Bn5Lk8Jh1Gf4DsA",
            (300.0, 90.0, 460.0, 170.0),
        );
        assert_child(
            &kids[2],
            "Ab1Cd2Ef3Gh4Ij5Kl6MnO",
            (200.0, 200.0, 240.0, 240.0),
        );

        let pad = padding(&scene, frame_id).expect("frame padding");
        approx_eq(pad.top, 80.0);
        approx_eq(pad.right, 60.0);
        approx_eq(pad.bottom, 20.0);
        approx_eq(pad.left, 58.4);
    }

    #[test]
    fn neighbor_tie_break_prefers_smaller_center_offset_then_z() {
        let c = shape("c", (0.0, 40.0, 40.0, 80.0), 0);
        let closer = shape("closer", (80.0, 45.0, 120.0, 85.0), 5);
        let farther = shape("farther", (80.0, 20.0, 120.0, 70.0), 1);
        let siblings = [&c, &closer, &farther];
        let n = neighbors(&c, &siblings);
        let right = n.right.expect("right");
        assert_eq!(right.id, "closer");
        approx_eq(right.gap, 40.0);

        let low_z = shape("low", (80.0, 40.0, 120.0, 80.0), 1);
        let high_z = shape("high", (80.0, 40.0, 120.0, 80.0), 3);
        let siblings = [&c, &high_z, &low_z];
        let n = neighbors(&c, &siblings);
        let right = n.right.expect("right");
        assert_eq!(right.id, "low");
        approx_eq(right.gap, 40.0);
    }

    #[test]
    fn tall_sidebar_does_not_bridge_rows() {
        // B10: bus (0,60,80,280), r1_a (140,60,140,80), r1_b (500,60,140,80),
        // r2_a (310,260,140,80), r2_b (500,260,140,80). Running intersection
        // keeps the tall bus from pulling the lower pair into the top row.
        let bus = shape_xywh("bus", 0.0, 60.0, 80.0, 280.0, 0);
        let r1_a = shape_xywh("r1_a", 140.0, 60.0, 140.0, 80.0, 1);
        let r1_b = shape_xywh("r1_b", 500.0, 60.0, 140.0, 80.0, 2);
        let r2_a = shape_xywh("r2_a", 310.0, 260.0, 140.0, 80.0, 3);
        let r2_b = shape_xywh("r2_b", 500.0, 260.0, 140.0, 80.0, 4);
        let siblings = [&bus, &r1_a, &r1_b, &r2_a, &r2_b];

        let rs = rows(&siblings);
        assert_eq!(rs.len(), 2);
        assert!(rs[0].parent.is_none());
        assert_eq!(rs[0].members, ["bus", "r1_a", "r1_b"]);
        assert_eq!(rs[0].gaps, [60.0, 220.0]);
        approx_eq(rs[0].pos, 400.0 / 3.0);

        assert!(rs[1].parent.is_none());
        assert_eq!(rs[1].members, ["r2_a", "r2_b"]);
        assert_eq!(rs[1].gaps, [50.0]);
        approx_eq(rs[1].pos, 300.0);
    }

    #[test]
    fn scatter_gather_shard_column_preserved() {
        // B14: a scatter-gather row with a vertical shard stack that must stay
        // a column, not get absorbed into the horizontal row.
        let q_gw = shape_xywh("q_gw", 40.0, 370.0, 175.0, 75.0, 0);
        let q_parse = shape_xywh("q_parse", 275.0, 370.0, 175.0, 75.0, 1);
        let blender = shape_xywh("blender", 510.0, 370.0, 175.0, 75.0, 2);
        let leaf1 = shape_xywh("leaf1", 745.0, 345.0, 175.0, 65.0, 3);
        let leaf2 = shape_xywh("leaf2", 745.0, 430.0, 175.0, 65.0, 4);
        let leaf3 = shape_xywh("leaf3", 745.0, 515.0, 175.0, 65.0, 5);
        let ranker = shape_xywh("ranker", 970.0, 370.0, 175.0, 80.0, 6);
        let siblings = [&q_gw, &q_parse, &blender, &leaf1, &leaf2, &leaf3, &ranker];

        let cs = columns(&siblings);
        assert!(
            cs.iter().any(|t| t.members == ["leaf1", "leaf2", "leaf3"]),
            "expected shard column [leaf1, leaf2, leaf3], got {cs:?}"
        );

        let rs = rows(&siblings);
        assert_eq!(rs.len(), 1);
        assert_eq!(rs[0].members, ["q_gw", "q_parse", "blender"]);
        assert_eq!(rs[0].gaps, [60.0, 60.0]);
        approx_eq(rs[0].pos, 407.5);
        assert!(!rs[0].members.iter().any(|id| id == "leaf1"));
        assert!(!rs[0].members.iter().any(|id| id == "leaf2"));
        assert!(!rs[0].members.iter().any(|id| id == "leaf3"));
        assert!(!rs.iter().any(|t| t.members.iter().any(|id| id == "ranker")));
    }

    #[test]
    fn tall_node_without_column_lock_does_not_union_bridge_rows() {
        // Intersection regression independent of Step 4: tall is NOT column-
        // aligned with any other node, so orthogonal split cannot remove it.
        // Geometry (xywh):
        //   tall  (0,   0,  80, 300)  y=[0,300]   cy=150  cx=40
        //   top_a (140, 0, 100,  80)  y=[0,80]    cy=40   cx=190
        //   top_b (300, 0, 100,  80)  y=[0,80]    cy=40   cx=350
        //   bot_a (140,220,100,  80)  y=[220,300] cy=260  cx=190
        //   bot_b (300,220,100,  80)  y=[220,300] cy=260  cx=350
        // Processing order by (center.y, x1): top_a, top_b, tall, bot_a, bot_b.
        // Union [lo,hi] after {top_a, top_b, tall} is [0,300]; bot_a overlap
        // with that union is 80 >= 0.5*min(80,80)=40, so union would swallow
        // both bottom nodes into one five-member row.
        // Intersection after {top_a, top_b, tall} is [0,80]; bot_a overlap
        // with [0,80] is 0, so the bottom pair must form a separate row.
        let tall = shape_xywh("tall", 0.0, 0.0, 80.0, 300.0, 0);
        let top_a = shape_xywh("top_a", 140.0, 0.0, 100.0, 80.0, 1);
        let top_b = shape_xywh("top_b", 300.0, 0.0, 100.0, 80.0, 2);
        let bot_a = shape_xywh("bot_a", 140.0, 220.0, 100.0, 80.0, 3);
        let bot_b = shape_xywh("bot_b", 300.0, 220.0, 100.0, 80.0, 4);
        let siblings = [&tall, &top_a, &top_b, &bot_a, &bot_b];

        let rs = rows(&siblings);
        assert_eq!(rs.len(), 2);
        assert_eq!(rs[0].members, ["tall", "top_a", "top_b"]);
        // gaps: top_a.x1 - tall.x2 = 140-80 = 60; top_b.x1 - top_a.x2 = 300-240 = 60
        assert_eq!(rs[0].gaps, [60.0, 60.0]);
        // pos = mean center.y = (150+40+40)/3 = 230/3
        approx_eq(rs[0].pos, 230.0 / 3.0);

        assert_eq!(rs[1].members, ["bot_a", "bot_b"]);
        // gaps: bot_b.x1 - bot_a.x2 = 300-240 = 60
        assert_eq!(rs[1].gaps, [60.0]);
        approx_eq(rs[1].pos, 260.0);
    }

    #[test]
    fn b21_obstacle_splits_row_across_nested_sub_zone() {
        // Fixture F21 serving pipeline: five siblings on one row, with a nested
        // sub-zone sitting in the 420px gap between blender and l1_ranker.
        let search_ui = shape_xywh("search_ui", 40.0, 220.0, 160.0, 80.0, 2);
        let q_rewrite = shape_xywh("q_rewrite", 260.0, 220.0, 160.0, 80.0, 3);
        let blender = shape_xywh("blender", 510.0, 220.0, 160.0, 80.0, 4);
        let l1_ranker = shape_xywh("l1_ranker", 1090.0, 220.0, 160.0, 80.0, 10);
        let l2_reranker = shape_xywh("l2_reranker", 1310.0, 220.0, 160.0, 80.0, 11);
        let siblings = [&search_ui, &q_rewrite, &blender, &l1_ranker, &l2_reranker];

        let unsplit = rows(&siblings);
        assert_eq!(unsplit.len(), 1);
        assert_eq!(
            unsplit[0].members,
            [
                "search_ui",
                "q_rewrite",
                "blender",
                "l1_ranker",
                "l2_reranker"
            ]
        );
        assert_eq!(unsplit[0].gaps, [60.0, 90.0, 420.0, 60.0]);
        approx_eq(unsplit[0].pos, 260.0);

        let obstacle = Rect::from_xywh(730.0, 60.0, 300.0, 420.0);
        let split = rows_with_obstacles(&siblings, &[obstacle]);
        assert_eq!(split.len(), 2);
        assert_eq!(split[0].members, ["search_ui", "q_rewrite", "blender"]);
        assert_eq!(split[0].gaps, [60.0, 90.0]);
        approx_eq(split[0].pos, 260.0);
        assert_eq!(split[1].members, ["l1_ranker", "l2_reranker"]);
        assert_eq!(split[1].gaps, [60.0]);
        approx_eq(split[1].pos, 260.0);

        // An obstacle above the shared row band must not split the track.
        let above = Rect::from_xywh(730.0, 0.0, 300.0, 180.0);
        let kept = rows_with_obstacles(&siblings, &[above]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].members, unsplit[0].members);
        assert_eq!(kept[0].gaps, unsplit[0].gaps);
    }

    #[test]
    fn b21_obstacle_splits_vertical_column() {
        let n1 = shape_xywh("n1", 100.0, 0.0, 160.0, 80.0, 0);
        let n2 = shape_xywh("n2", 100.0, 120.0, 160.0, 80.0, 1);
        let n3 = shape_xywh("n3", 100.0, 440.0, 160.0, 80.0, 2);
        let n4 = shape_xywh("n4", 100.0, 560.0, 160.0, 80.0, 3);
        let siblings = [&n1, &n2, &n3, &n4];

        let unsplit = columns(&siblings);
        assert_eq!(unsplit.len(), 1);
        assert_eq!(unsplit[0].members, ["n1", "n2", "n3", "n4"]);
        assert_eq!(unsplit[0].gaps, [40.0, 240.0, 40.0]);

        let obstacle = Rect::from_xywh(60.0, 240.0, 240.0, 160.0);
        let split = columns_with_obstacles(&siblings, &[obstacle]);
        assert_eq!(split.len(), 2);
        assert_eq!(split[0].members, ["n1", "n2"]);
        assert_eq!(split[0].gaps, [40.0]);
        approx_eq(split[0].pos, 180.0);
        assert_eq!(split[1].members, ["n3", "n4"]);
        assert_eq!(split[1].gaps, [40.0]);
        approx_eq(split[1].pos, 180.0);
    }
}
