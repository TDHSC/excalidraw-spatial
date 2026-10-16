use std::collections::{HashMap, HashSet};

use crate::geometry::{clip_segment, polyline_point_at, Point, Rect};
use crate::linter::{BINDING_GAP_BASE, BOUND_TEXT_PADDING};
use crate::scene_graph::{Attach, Scene};
use crate::schema::Document;
use crate::text_metrics::text_size;

pub(crate) fn bound_text_xy(
    r: &Rect,
    tw: f64,
    th: f64,
    text_align: Option<&str>,
    vertical_align: Option<&str>,
) -> (f64, f64) {
    let p = BOUND_TEXT_PADDING as f64;
    let x = match text_align {
        Some("left") => r.x1 + p,
        Some("right") => r.x2 - p - tw,
        _ => r.x1 + (r.width() - tw) / 2.0,
    };
    let y = match vertical_align {
        Some("top") => r.y1 + p,
        Some("bottom") => r.y2 - p - th,
        _ => r.y1 + (r.height() - th) / 2.0,
    };
    (x, y)
}

fn round_json(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn focus_point(rect: &Rect, fixed: Option<[f64; 2]>) -> Point {
    match fixed {
        Some([fx, fy]) => Point {
            x: rect.x1 + fx * rect.width(),
            y: rect.y1 + fy * rect.height(),
        },
        None => rect.center(),
    }
}

fn lerp(a: Point, b: Point, t: f64) -> Point {
    Point {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
    }
}

fn is_horizontal(a: Point, b: Point) -> bool {
    (b.x - a.x).abs() >= (b.y - a.y).abs()
}

fn current_elem_rect(
    element_id: &str,
    elem_changed: &HashMap<String, (Rect, Rect)>,
    doc: &Document,
) -> Option<Rect> {
    if let Some((_, new_rect)) = elem_changed.get(element_id) {
        return Some(*new_rect);
    }
    doc.element(element_id).map(|el| el.rect())
}

fn endpoint_shift(
    attach: &Attach,
    changed: &HashMap<String, (Rect, Rect)>,
    elem_changed: &HashMap<String, (Rect, Rect)>,
    fixed_point: Option<[f64; 2]>,
) -> Option<Point> {
    match attach {
        Attach::Free => None,
        Attach::Bound { target, element } => {
            if !changed.contains_key(target) {
                return None;
            }
            let (old_rect, new_rect) = if let Some(pair) = elem_changed.get(element) {
                *pair
            } else {
                *changed.get(target)?
            };
            let old_focus = focus_point(&old_rect, fixed_point);
            let new_focus = focus_point(&new_rect, fixed_point);
            Some(Point {
                x: new_focus.x - old_focus.x,
                y: new_focus.y - old_focus.y,
            })
        }
        Attach::Near { target, .. } => {
            let (old_rect, new_rect) = changed.get(target)?;
            Some(Point {
                x: new_rect.x1 - old_rect.x1,
                y: new_rect.y1 - old_rect.y1,
            })
        }
    }
}

struct EndGeom {
    rect: Rect,
    focus: Point,
    gap: f64,
}

fn end_geom(
    attach: &Attach,
    changed: &HashMap<String, (Rect, Rect)>,
    elem_changed: &HashMap<String, (Rect, Rect)>,
    fixed_point: Option<[f64; 2]>,
    old_scene: &Scene,
    doc: &Document,
) -> Option<EndGeom> {
    match attach {
        Attach::Free => None,
        Attach::Bound { element, .. } => {
            let rect = current_elem_rect(element, elem_changed, doc)?;
            let gap = BINDING_GAP_BASE as f64
                + doc
                    .element(element)
                    .map(|el| el.stroke_width())
                    .unwrap_or(2.0)
                    / 2.0;
            Some(EndGeom {
                focus: focus_point(&rect, fixed_point),
                rect,
                gap,
            })
        }
        Attach::Near { target, .. } => {
            let rect = changed
                .get(target)
                .map(|(_, new_rect)| *new_rect)
                .or_else(|| old_scene.rect_of(target))?;
            Some(EndGeom {
                focus: rect.center(),
                rect,
                gap: 6.0,
            })
        }
    }
}

fn write_arrow_points(doc: &mut Document, id: &str, abs: &[Point]) {
    if abs.is_empty() {
        return;
    }
    let p0 = abs[0];
    doc.set_num(id, "x", p0.x);
    doc.set_num(id, "y", p0.y);
    let rel: Vec<[f64; 2]> = abs.iter().map(|p| [p.x - p0.x, p.y - p0.y]).collect();
    doc.set_points(id, &rel);
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for [x, y] in &rel {
        let x = round_json(*x);
        let y = round_json(*y);
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    doc.set_num(id, "width", max_x - min_x);
    doc.set_num(id, "height", max_y - min_y);
}

fn place_label(
    doc: &mut Document,
    label_id: &str,
    abs: &[Point],
    fraction: f64,
    elem_changed: &mut HashMap<String, (Rect, Rect)>,
) {
    let Some(el) = doc.element(label_id) else {
        return;
    };
    let ts = text_size(el);
    let tw = ts.width;
    let th = ts.height;
    let estimated = ts.estimated;
    let old_rect = el.rect();
    let c = polyline_point_at(abs, fraction);
    let new_x = c.x - tw / 2.0;
    let new_y = c.y - th / 2.0;
    doc.set_num(label_id, "x", new_x);
    doc.set_num(label_id, "y", new_y);
    if estimated {
        doc.set_num(label_id, "width", tw);
        doc.set_num(label_id, "height", th);
    }
    elem_changed.insert(
        label_id.to_string(),
        (old_rect, Rect::from_xywh(new_x, new_y, tw, th)),
    );
}

/// Sync connectors whose attached nodes moved or resized. Returns how many
/// connectors had their points written back.
pub(crate) fn sync_arrows(
    doc: &mut Document,
    old_scene: &Scene,
    changed: &HashMap<String, (Rect, Rect)>,
    elem_changed: &mut HashMap<String, (Rect, Rect)>,
    skip: &HashSet<String>,
) -> usize {
    let mut updated = 0;
    let connectors = old_scene.connectors.clone();
    for conn in connectors {
        if skip.contains(&conn.id) {
            continue;
        }
        let (start_fp, end_fp, label_pos) = match doc.element(&conn.id) {
            Some(el) => (
                el.start_binding.as_ref().and_then(|b| b.fixed_point),
                el.end_binding.as_ref().and_then(|b| b.fixed_point),
                el.label_position.unwrap_or(0.5),
            ),
            None => continue,
        };
        let start_shift = endpoint_shift(&conn.start, changed, elem_changed, start_fp);
        let end_shift = endpoint_shift(&conn.end, changed, elem_changed, end_fp);
        if start_shift.is_none() && end_shift.is_none() {
            continue;
        }
        let same = match (start_shift, end_shift) {
            (Some(s), Some(e)) => (s.x - e.x).abs() < 1e-9 && (s.y - e.y).abs() < 1e-9,
            _ => false,
        };

        let original = conn.points.clone();
        let mut abs = original.clone();
        let mut used = false;

        let case_a = !conn.elbowed
            && original.len() == 2
            && !matches!(conn.start, Attach::Free)
            && !matches!(conn.end, Attach::Free)
            && !same;
        if case_a {
            if let (Some(gs), Some(ge)) = (
                end_geom(&conn.start, changed, elem_changed, start_fp, old_scene, doc),
                end_geom(&conn.end, changed, elem_changed, end_fp, old_scene, doc),
            ) {
                let cs = clip_segment(gs.focus, ge.focus, &gs.rect.inflate(gs.gap));
                let ce = clip_segment(gs.focus, ge.focus, &ge.rect.inflate(ge.gap));
                if let (Some(cs), Some(ce)) = (cs, ce) {
                    if cs.1 < ce.0 {
                        abs = vec![
                            lerp(gs.focus, ge.focus, cs.1),
                            lerp(gs.focus, ge.focus, ce.0),
                        ];
                        used = true;
                    }
                }
            }
        }

        if !used && same {
            if let Some(shift) = start_shift.or(end_shift) {
                for p in &mut abs {
                    p.x += shift.x;
                    p.y += shift.y;
                }
                used = true;
            }
        }

        if !used {
            if let Some(shift) = start_shift {
                if let Some(p) = abs.first_mut() {
                    p.x += shift.x;
                    p.y += shift.y;
                }
            }
            if let Some(shift) = end_shift {
                if let Some(p) = abs.last_mut() {
                    p.x += shift.x;
                    p.y += shift.y;
                }
            }
            if conn.elbowed && abs.len() >= 2 {
                if original.len() == 2 {
                    let s = abs[0];
                    let e = abs[1];
                    if (e.x - s.x).abs() > 0.5 && (e.y - s.y).abs() > 0.5 {
                        if is_horizontal(original[0], original[1]) {
                            let m = (s.x + e.x) / 2.0;
                            abs = vec![s, Point { x: m, y: s.y }, Point { x: m, y: e.y }, e];
                        } else {
                            let m = (s.y + e.y) / 2.0;
                            abs = vec![s, Point { x: s.x, y: m }, Point { x: e.x, y: m }, e];
                        }
                    }
                } else if original.len() >= 3 && abs.len() >= 3 {
                    let n = abs.len();
                    if start_shift.is_some() && is_horizontal(original[0], original[1]) {
                        abs[1].y = abs[0].y;
                    } else if start_shift.is_some() {
                        abs[1].x = abs[0].x;
                    }
                    if end_shift.is_some()
                        && is_horizontal(original[original.len() - 2], original[original.len() - 1])
                    {
                        abs[n - 2].y = abs[n - 1].y;
                    } else if end_shift.is_some() {
                        abs[n - 2].x = abs[n - 1].x;
                    }
                }
            }
        }

        write_arrow_points(doc, &conn.id, &abs);
        if let Some(label_id) = &conn.label_element {
            place_label(doc, label_id, &abs, label_pos, elem_changed);
        }
        updated += 1;
    }
    updated
}
