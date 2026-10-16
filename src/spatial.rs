use crate::geometry::Rect;
use crate::scene_graph::{Component, Scene};

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

pub fn rows(siblings: &[&Component]) -> Vec<Track> {
    cluster(siblings, Axis::Row)
}

pub fn columns(siblings: &[&Component]) -> Vec<Track> {
    cluster(siblings, Axis::Column)
}

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
                parent: parent.clone(),
                members: members.iter().map(|c| c.id.clone()).collect(),
                gaps,
                pos,
            }
        })
        .collect()
}

fn member_range(c: &Component, axis: &Axis) -> (f64, f64) {
    match axis {
        Axis::Row => (c.rect.y1, c.rect.y2),
        Axis::Column => (c.rect.x1, c.rect.x2),
    }
}

fn track_range(members: &[&Component], axis: &Axis) -> (f64, f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    let mut min_h = f64::INFINITY;
    for c in members {
        let (a, b) = member_range(c, axis);
        lo = lo.min(a);
        hi = hi.max(b);
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
}
