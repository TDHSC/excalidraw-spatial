pub mod ops;
pub mod sync_bindings;

use std::collections::{HashMap, HashSet};

use crate::geometry::Rect;
use crate::scene_graph::{ComponentKind, ContainerKind, Scene};
use crate::schema::Document;
use crate::text_metrics::text_size;
use crate::LayoutCommands;

use self::sync_bindings::{bound_text_xy, sync_arrows};

#[derive(Debug, Clone, Default)]
pub struct Change {
    pub moves: Vec<(String, f64, f64)>,
    pub resizes: Vec<(String, Rect)>,
}

impl Change {
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty() && self.resizes.is_empty()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ApplyReport {
    pub moved: usize,
    pub resized: usize,
    pub connectors: usize,
}

pub fn layout_change(scene: &Scene, cmd: &LayoutCommands) -> anyhow::Result<Change> {
    match cmd {
        LayoutCommands::Move {
            id, dx, dy, ripple, ..
        } => ops::move_target(scene, id, dx.unwrap_or(0.0), dy.unwrap_or(0.0), *ripple),
        LayoutCommands::Align { ids, axis, .. } => ops::align(scene, ids, axis.clone()),
        LayoutCommands::Distribute {
            ids,
            direction,
            gap,
            ..
        } => ops::distribute(scene, ids, direction.clone(), *gap),
        LayoutCommands::Place {
            id,
            right_of,
            left_of,
            below,
            above,
            gap,
            align: place_align,
            ..
        } => ops::place(
            scene,
            id,
            right_of.as_deref(),
            left_of.as_deref(),
            below.as_deref(),
            above.as_deref(),
            *gap,
            place_align.clone(),
        ),
        LayoutCommands::FitContainer {
            id,
            padding,
            expand_only,
            ..
        } => ops::fit_container(scene, id, *padding, *expand_only),
        LayoutCommands::FitText {
            id,
            all,
            padding_x,
            padding_y,
            ..
        } => ops::fit_text(scene, id.as_deref(), *all, *padding_x, *padding_y),
    }
}

pub fn strip_nops(change: &mut Change, scene: &Scene) {
    let mut seen = std::collections::HashSet::new();
    change.moves.retain(|(id, dx, dy)| {
        if *dx == 0.0 && *dy == 0.0 {
            return false;
        }
        seen.insert(id.clone())
    });
    let mut seen_r = std::collections::HashSet::new();
    change.resizes.retain(|(id, rect)| {
        if let Some(cur) = scene.rect_of(id) {
            if cur.x1 == rect.x1 && cur.y1 == rect.y1 && cur.x2 == rect.x2 && cur.y2 == rect.y2 {
                return false;
            }
        }
        seen_r.insert(id.clone())
    });
}

pub fn apply(
    doc: &mut Document,
    old_scene: &Scene,
    change: &Change,
) -> anyhow::Result<ApplyReport> {
    let mut changed: HashMap<String, (Rect, Rect)> = HashMap::new();
    let mut elem_changed: HashMap<String, (Rect, Rect)> = HashMap::new();
    let mut translated_elems: HashSet<String> = HashSet::new();

    let (expanded, connector_moves) = expand_moves(old_scene, &change.moves);

    for (id, dx, dy) in &expanded {
        let elements = if let Some(component) = old_scene.component(id) {
            changed.insert(
                id.clone(),
                (component.rect, component.rect.translate(*dx, *dy)),
            );
            component.elements.clone()
        } else if let Some(container) = old_scene.container(id) {
            changed.insert(
                id.clone(),
                (container.rect, container.rect.translate(*dx, *dy)),
            );
            container.elements.clone()
        } else {
            continue;
        };
        for eid in elements {
            if !translated_elems.insert(eid.clone()) {
                continue;
            }
            translate_element(doc, &eid, *dx, *dy, &mut elem_changed);
        }
    }

    let mut skip_sync: HashSet<String> = HashSet::new();
    for (id, dx, dy) in &connector_moves {
        skip_sync.insert(id.clone());
        if let Some(conn) = old_scene.connector(id) {
            translate_element(doc, &conn.id, *dx, *dy, &mut elem_changed);
            if let Some(label_id) = &conn.label_element {
                translate_element(doc, label_id, *dx, *dy, &mut elem_changed);
            }
        }
    }

    let resized = change.resizes.len();
    let mut resized_ids: HashSet<String> = HashSet::new();
    for (id, new_rect) in &change.resizes {
        apply_resize(
            doc,
            old_scene,
            id,
            *new_rect,
            &mut changed,
            &mut elem_changed,
            &mut resized_ids,
        )?;
    }

    let connectors = sync_arrows(
        doc,
        old_scene,
        &changed,
        &mut elem_changed,
        &skip_sync,
        &resized_ids,
    );
    Ok(ApplyReport {
        moved: expanded.len() + connector_moves.len(),
        resized,
        connectors,
    })
}

type MoveList = Vec<(String, f64, f64)>;

pub(crate) fn expand_moves(scene: &Scene, moves: &[(String, f64, f64)]) -> (MoveList, MoveList) {
    let mut expanded = Vec::new();
    let mut seen = HashSet::new();
    let mut connector_moves = Vec::new();
    let mut seen_conn = HashSet::new();

    for (id, dx, dy) in moves {
        if scene.connector(id).is_some() {
            if seen_conn.insert(id.clone()) {
                connector_moves.push((id.clone(), *dx, *dy));
            }
            continue;
        }
        push_move(&mut expanded, &mut seen, id.clone(), *dx, *dy);
        if scene.container(id).is_some() {
            for component in &scene.components {
                if ancestor_contains(scene, component.parent.clone(), id) {
                    push_move(&mut expanded, &mut seen, component.id.clone(), *dx, *dy);
                }
            }
            for container in &scene.containers {
                if ancestor_contains(scene, container.parent.clone(), id) {
                    push_move(&mut expanded, &mut seen, container.id.clone(), *dx, *dy);
                }
            }
        }
    }
    (expanded, connector_moves)
}

fn push_move(
    expanded: &mut Vec<(String, f64, f64)>,
    seen: &mut HashSet<String>,
    id: String,
    dx: f64,
    dy: f64,
) {
    if seen.insert(id.clone()) {
        expanded.push((id, dx, dy));
    }
}

fn ancestor_contains(scene: &Scene, mut parent: Option<String>, ancestor: &str) -> bool {
    while let Some(p) = parent {
        if p == ancestor {
            return true;
        }
        parent = if let Some(c) = scene.container(&p) {
            c.parent.clone()
        } else if let Some(c) = scene.component(&p) {
            c.parent.clone()
        } else {
            None
        };
    }
    false
}

fn translate_element(
    doc: &mut Document,
    id: &str,
    dx: f64,
    dy: f64,
    elem_changed: &mut HashMap<String, (Rect, Rect)>,
) {
    let Some(el) = doc.element(id) else {
        return;
    };
    let old_rect = el.rect();
    let new_x = el.x() + dx;
    let new_y = el.y() + dy;
    let new_rect = if el.element_type == "arrow" || el.element_type == "line" {
        old_rect.translate(dx, dy)
    } else {
        Rect::from_xywh(new_x, new_y, el.width(), el.height())
    };
    doc.set_num(id, "x", new_x);
    doc.set_num(id, "y", new_y);
    elem_changed.insert(id.to_string(), (old_rect, new_rect));
}

fn apply_resize(
    doc: &mut Document,
    old_scene: &Scene,
    id: &str,
    new_rect: Rect,
    changed: &mut HashMap<String, (Rect, Rect)>,
    elem_changed: &mut HashMap<String, (Rect, Rect)>,
    resized_ids: &mut HashSet<String>,
) -> anyhow::Result<()> {
    if let Some(component) = old_scene.component(id) {
        if component.kind != ComponentKind::Shape {
            anyhow::bail!("cannot resize {id}: not a Shape component or container");
        }
        let primary = component
            .elements
            .first()
            .cloned()
            .unwrap_or_else(|| component.id.clone());
        resized_ids.insert(id.to_string());
        resized_ids.insert(primary.clone());
        set_primary_rect(doc, &primary, new_rect, elem_changed);
        if let Some(label_id) = &component.label_element {
            reposition_bound_text(doc, label_id, &primary, &new_rect, elem_changed);
        }
        changed.insert(id.to_string(), (component.rect, new_rect));
        return Ok(());
    }
    if let Some(container) = old_scene.container(id) {
        let primary = container
            .elements
            .first()
            .cloned()
            .unwrap_or_else(|| container.id.clone());
        resized_ids.insert(id.to_string());
        resized_ids.insert(primary.clone());
        set_primary_rect(doc, &primary, new_rect, elem_changed);
        if let Some(title_id) = &container.title_element {
            let bound = doc
                .element(title_id)
                .and_then(|el| el.container_id.as_deref().map(|cid| cid == primary))
                .unwrap_or(false);
            if bound {
                reposition_bound_text(doc, title_id, &primary, &new_rect, elem_changed);
            } else if container.kind == ContainerKind::Zone {
                let dx = new_rect.x1 - container.rect.x1;
                let mut dy = new_rect.y1 - container.rect.y1;
                let kids = crate::spatial::children(old_scene, id);
                if !kids.is_empty() {
                    if let Some(el) = doc.element(title_id) {
                        let u_y1 = kids
                            .iter()
                            .map(|(_, rect)| rect.y1)
                            .fold(f64::INFINITY, f64::min);
                        let ceiling = (u_y1 - text_size(el).height - 6.0).max(new_rect.y1);
                        if el.y() + dy > ceiling {
                            dy = ceiling - el.y();
                        }
                    }
                }
                translate_element(doc, title_id, dx, dy, elem_changed);
            }
        }
        changed.insert(id.to_string(), (container.rect, new_rect));
        return Ok(());
    }
    anyhow::bail!("cannot resize {id}: not a Shape component or container")
}

fn set_primary_rect(
    doc: &mut Document,
    id: &str,
    r: Rect,
    elem_changed: &mut HashMap<String, (Rect, Rect)>,
) {
    if let Some(el) = doc.element(id) {
        elem_changed.insert(id.to_string(), (el.rect(), r));
    }
    doc.set_num(id, "x", r.x1);
    doc.set_num(id, "y", r.y1);
    doc.set_num(id, "width", r.width());
    doc.set_num(id, "height", r.height());
}

fn reposition_bound_text(
    doc: &mut Document,
    label_id: &str,
    primary_id: &str,
    r: &Rect,
    elem_changed: &mut HashMap<String, (Rect, Rect)>,
) {
    let Some(el) = doc.element(label_id) else {
        return;
    };
    if el.container_id.as_deref() != Some(primary_id) {
        return;
    }
    let ts = text_size(el);
    let tw = ts.width;
    let th = ts.height;
    let estimated = ts.estimated;
    let text_align = el.text_align.clone();
    let vertical_align = el.vertical_align.clone();
    let old_rect = el.rect();
    let (x, y) = bound_text_xy(r, tw, th, text_align.as_deref(), vertical_align.as_deref());
    doc.set_num(label_id, "x", x);
    doc.set_num(label_id, "y", y);
    if estimated {
        doc.set_num(label_id, "width", tw);
        doc.set_num(label_id, "height", th);
    }
    elem_changed.insert(
        label_id.to_string(),
        (old_rect, Rect::from_xywh(x, y, tw, th)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene_graph::Scene;
    use crate::schema::Document;

    fn json_el<'a>(doc: &'a Document, id: &str) -> &'a serde_json::Value {
        doc.value["elements"]
            .as_array()
            .expect("elements")
            .iter()
            .find(|el| el["id"] == id)
            .unwrap_or_else(|| panic!("missing {id}"))
    }

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    #[test]
    fn elbow_two_points_inserts_orthogonal_midpoints() {
        let src = r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "A", "x": 0, "y": 0, "width": 100, "height": 100},
    {"type": "rectangle", "id": "B", "x": 200, "y": 0, "width": 100, "height": 100},
    {
      "type": "arrow",
      "id": "arr",
      "x": 106,
      "y": 50,
      "width": 88,
      "height": 0,
      "points": [[0, 0], [88, 0]],
      "elbowed": true,
      "startBinding": {"elementId": "A", "fixedPoint": [1, 0.5]},
      "endBinding": {"elementId": "B", "fixedPoint": [0, 0.5]}
    }
  ]
}"#;
        let mut doc = Document::parse(src).expect("parse");
        let scene = Scene::build(&doc);
        apply(
            &mut doc,
            &scene,
            &Change {
                moves: vec![("B".to_string(), 0.0, 100.0)],
                resizes: vec![],
            },
        )
        .expect("apply");
        assert_eq!(
            json_el(&doc, "arr")["points"],
            serde_json::json!([[0, 0], [44, 0], [44, 100], [88, 100]])
        );
    }

    #[test]
    fn zone_free_title_clamps_above_children_on_resize() {
        let src = r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "z", "x": 0, "y": 0, "width": 400, "height": 300},
    {"type": "text", "id": "t", "x": 16, "y": 22, "width": 80, "height": 25, "text": "Lane", "fontSize": 20, "lineHeight": 1.25},
    {"type": "rectangle", "id": "c1", "x": 40, "y": 100, "width": 120, "height": 80},
    {"type": "rectangle", "id": "c2", "x": 200, "y": 100, "width": 120, "height": 80}
  ]
}"#;
        let mut doc = Document::parse(src).expect("parse");
        let scene = Scene::build(&doc);
        assert_eq!(
            scene.containers.len(),
            1,
            "z must be a Zone: {:?}",
            scene.containers.iter().map(|c| &c.id).collect::<Vec<_>>()
        );
        assert_eq!(scene.containers[0].id, "z");
        assert_eq!(scene.containers[0].title_element.as_deref(), Some("t"));
        let kids: Vec<_> = crate::spatial::children(&scene, "z")
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(kids, vec!["c1".to_string(), "c2".to_string()]);

        let change = super::ops::fit_container(&scene, "z", Some(10.0), false).expect("fit");
        assert_eq!(change.resizes.len(), 1);
        let (id, new_rect) = &change.resizes[0];
        assert_eq!(id, "z");
        approx(new_rect.y1, 55.0);

        apply(&mut doc, &scene, &change).expect("apply");
        approx(json_el(&doc, "t")["y"].as_f64().expect("t.y"), 69.0);
        approx(json_el(&doc, "c1")["x"].as_f64().expect("c1.x"), 40.0);
        approx(json_el(&doc, "c1")["y"].as_f64().expect("c1.y"), 100.0);
        approx(json_el(&doc, "c2")["x"].as_f64().expect("c2.x"), 200.0);
        approx(json_el(&doc, "c2")["y"].as_f64().expect("c2.y"), 100.0);
    }

    #[test]
    fn resize_recenters_bound_text() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/basic_flow.excalidraw");
        let src = std::fs::read_to_string(&path).expect("fixture");
        let mut doc = Document::parse(&src).expect("parse");
        let scene = Scene::build(&doc);
        apply(
            &mut doc,
            &scene,
            &Change {
                moves: vec![],
                resizes: vec![(
                    "api".to_string(),
                    Rect {
                        x1: 360.0,
                        y1: 100.0,
                        x2: 560.0,
                        y2: 200.0,
                    },
                )],
            },
        )
        .expect("apply");
        let label = json_el(&doc, "api-label");
        let width = label["width"].as_f64().expect("width");
        approx(label["x"].as_f64().expect("x"), 460.0 - width / 2.0);
        approx(label["y"].as_f64().expect("y"), 137.5);
    }
}
