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

pub(crate) fn round_json(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

pub(crate) fn normalized_anchor(rect: &Rect, p: Point) -> (f64, f64) {
    let q = rect.closest_point(p);
    let u = if rect.width() > 0.0 {
        (q.x - rect.x1) / rect.width()
    } else {
        0.5
    };
    let v = if rect.height() > 0.0 {
        (q.y - rect.y1) / rect.height()
    } else {
        0.5
    };
    (u, v)
}

fn clamped_anchor(rect: &Rect, p: Point) -> [f64; 2] {
    let (u, v) = normalized_anchor(rect, p);
    [u.clamp(0.1, 0.9), v.clamp(0.1, 0.9)]
}

fn attach_target(attach: &Attach) -> Option<&str> {
    match attach {
        Attach::Bound { target, .. } | Attach::Near { target, .. } => Some(target.as_str()),
        Attach::Free => None,
    }
}

fn unordered_pair(start: &str, end: &str) -> (String, String) {
    if start <= end {
        (start.to_string(), end.to_string())
    } else {
        (end.to_string(), start.to_string())
    }
}

pub(crate) fn focus_point(rect: &Rect, fixed: Option<[f64; 2]>) -> Point {
    match fixed {
        Some([fx, fy]) => Point {
            x: rect.x1 + fx * rect.width(),
            y: rect.y1 + fy * rect.height(),
        },
        None => rect.center(),
    }
}

fn project_shift(old_rect: &Rect, new_rect: &Rect, p: Point) -> Point {
    let (u, v) = normalized_anchor(old_rect, p);
    let q = old_rect.closest_point(p);
    let q_new = Point {
        x: new_rect.x1 + u * new_rect.width(),
        y: new_rect.y1 + v * new_rect.height(),
    };
    Point {
        x: q_new.x - q.x,
        y: q_new.y - q.y,
    }
}

fn attach_resized(attach: &Attach, resized_ids: &HashSet<String>) -> bool {
    match attach {
        Attach::Bound { target, element } => {
            resized_ids.contains(target) || resized_ids.contains(element)
        }
        Attach::Near { target, .. } => resized_ids.contains(target),
        Attach::Free => false,
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
    resized_ids: &HashSet<String>,
    p: Point,
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
            let resized = resized_ids.contains(target) || resized_ids.contains(element);
            if resized && fixed_point.is_none() {
                return Some(project_shift(&old_rect, &new_rect, p));
            }
            let old_focus = focus_point(&old_rect, fixed_point);
            let new_focus = focus_point(&new_rect, fixed_point);
            Some(Point {
                x: new_focus.x - old_focus.x,
                y: new_focus.y - old_focus.y,
            })
        }
        Attach::Near { target, .. } => {
            let (old_rect, new_rect) = changed.get(target)?;
            if resized_ids.contains(target) {
                return Some(project_shift(old_rect, new_rect, p));
            }
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

struct EndGeomInput<'a> {
    fixed_point: Option<[f64; 2]>,
    original_end: Point,
    preserve_offset: bool,
    old_scene: &'a Scene,
    doc: &'a Document,
}

fn end_geom(
    attach: &Attach,
    changed: &HashMap<String, (Rect, Rect)>,
    elem_changed: &HashMap<String, (Rect, Rect)>,
    input: EndGeomInput<'_>,
) -> Option<EndGeom> {
    match attach {
        Attach::Free => None,
        Attach::Bound { element, .. } => {
            let rect = current_elem_rect(element, elem_changed, input.doc)?;
            let gap = BINDING_GAP_BASE as f64
                + input
                    .doc
                    .element(element)
                    .map(|el| el.stroke_width())
                    .unwrap_or(2.0)
                    / 2.0;
            let focus = if input.fixed_point.is_none() && input.preserve_offset {
                let old_rect = elem_changed
                    .get(element)
                    .map(|(old, _)| *old)
                    .unwrap_or(rect);
                focus_point(&rect, Some(clamped_anchor(&old_rect, input.original_end)))
            } else {
                focus_point(&rect, input.fixed_point)
            };
            Some(EndGeom { focus, rect, gap })
        }
        Attach::Near { target, .. } => {
            let rect = changed
                .get(target)
                .map(|(_, new_rect)| *new_rect)
                .or_else(|| input.old_scene.rect_of(target))?;
            Some(EndGeom {
                focus: rect.center(),
                rect,
                gap: 6.0,
            })
        }
    }
}

pub(crate) fn clip_straight_points(
    s_rect: &Rect,
    gs: f64,
    focus_s: Point,
    e_rect: &Rect,
    ge: f64,
    focus_e: Point,
) -> Option<Vec<Point>> {
    let cs = clip_segment(focus_s, focus_e, &s_rect.inflate(gs));
    let ce = clip_segment(focus_s, focus_e, &e_rect.inflate(ge));
    if let (Some(cs), Some(ce)) = (cs, ce) {
        if cs.1 < ce.0 {
            return Some(vec![
                lerp(focus_s, focus_e, cs.1),
                lerp(focus_s, focus_e, ce.0),
            ]);
        }
    }
    let cs0 = clip_segment(focus_s, focus_e, s_rect);
    let ce0 = clip_segment(focus_s, focus_e, e_rect);
    if let (Some(cs0), Some(ce0)) = (cs0, ce0) {
        if cs0.1 < ce0.0 {
            return Some(vec![
                lerp(focus_s, focus_e, cs0.1),
                lerp(focus_s, focus_e, ce0.0),
            ]);
        }
    }
    None
}

fn route_hits_inset(route: &[Point], s_rect: &Rect, e_rect: &Rect) -> bool {
    let s_inset = s_rect.inflate(-2.0);
    let e_inset = e_rect.inflate(-2.0);
    route.windows(2).any(|seg| {
        clip_segment(seg[0], seg[1], &s_inset).is_some()
            || clip_segment(seg[0], seg[1], &e_inset).is_some()
    })
}

pub(crate) fn elbow_two_point_route(
    s: Point,
    e: Point,
    horizontal: bool,
    s_rect: &Rect,
    e_rect: &Rect,
) -> Vec<Point> {
    let route_h = {
        let m = (s.x + e.x) / 2.0;
        vec![s, Point { x: m, y: s.y }, Point { x: m, y: e.y }, e]
    };
    let route_v = {
        let m = (s.y + e.y) / 2.0;
        vec![s, Point { x: s.x, y: m }, Point { x: e.x, y: m }, e]
    };
    let (primary, alt) = if horizontal {
        (route_h, route_v)
    } else {
        (route_v, route_h)
    };
    if route_hits_inset(&primary, s_rect, e_rect) && !route_hits_inset(&alt, s_rect, e_rect) {
        alt
    } else {
        primary
    }
}

pub(crate) fn write_arrow_points(doc: &mut Document, id: &str, abs: &[Point]) {
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

pub(crate) fn place_label(
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
    resized_ids: &HashSet<String>,
) -> usize {
    let mut updated = 0;
    let connectors = old_scene.connectors.clone();
    let mut pair_counts: HashMap<(String, String), usize> = HashMap::new();
    for conn in &connectors {
        if let (Some(start), Some(end)) = (attach_target(&conn.start), attach_target(&conn.end)) {
            *pair_counts.entry(unordered_pair(start, end)).or_insert(0) += 1;
        }
    }
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
        let start_p = conn
            .points
            .first()
            .copied()
            .unwrap_or(Point { x: 0.0, y: 0.0 });
        let end_p = conn.points.last().copied().unwrap_or(start_p);
        let start_shift = endpoint_shift(
            &conn.start,
            changed,
            elem_changed,
            start_fp,
            resized_ids,
            start_p,
        );
        let end_shift =
            endpoint_shift(&conn.end, changed, elem_changed, end_fp, resized_ids, end_p);
        if start_shift.is_none() && end_shift.is_none() {
            continue;
        }
        let same = match (start_shift, end_shift) {
            (Some(s), Some(e)) => {
                (s.x - e.x).abs() < 1e-9
                    && (s.y - e.y).abs() < 1e-9
                    && !attach_resized(&conn.start, resized_ids)
                    && !attach_resized(&conn.end, resized_ids)
            }
            _ => false,
        };

        let original = conn.points.clone();
        let mut abs = original.clone();
        let mut used = false;

        let bundle_count = attach_target(&conn.start)
            .zip(attach_target(&conn.end))
            .and_then(|(start, end)| pair_counts.get(&unordered_pair(start, end)).copied())
            .unwrap_or(1);
        let preserve_offset = bundle_count >= 2;
        let case_a = !conn.elbowed
            && original.len() == 2
            && !matches!(conn.start, Attach::Free)
            && !matches!(conn.end, Attach::Free)
            && !same;
        if case_a {
            if let (Some(gs), Some(ge)) = (
                end_geom(
                    &conn.start,
                    changed,
                    elem_changed,
                    EndGeomInput {
                        fixed_point: start_fp,
                        original_end: start_p,
                        preserve_offset,
                        old_scene,
                        doc,
                    },
                ),
                end_geom(
                    &conn.end,
                    changed,
                    elem_changed,
                    EndGeomInput {
                        fixed_point: end_fp,
                        original_end: end_p,
                        preserve_offset,
                        old_scene,
                        doc,
                    },
                ),
            ) {
                if let Some(clipped) =
                    clip_straight_points(&gs.rect, gs.gap, gs.focus, &ge.rect, ge.gap, ge.focus)
                {
                    abs = clipped;
                    used = true;
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
                        if let (Some(gs), Some(ge)) = (
                            end_geom(
                                &conn.start,
                                changed,
                                elem_changed,
                                EndGeomInput {
                                    fixed_point: start_fp,
                                    original_end: start_p,
                                    preserve_offset: false,
                                    old_scene,
                                    doc,
                                },
                            ),
                            end_geom(
                                &conn.end,
                                changed,
                                elem_changed,
                                EndGeomInput {
                                    fixed_point: end_fp,
                                    original_end: end_p,
                                    preserve_offset: false,
                                    old_scene,
                                    doc,
                                },
                            ),
                        ) {
                            abs = elbow_two_point_route(
                                s,
                                e,
                                is_horizontal(original[0], original[1]),
                                &gs.rect,
                                &ge.rect,
                            );
                        } else if is_horizontal(original[0], original[1]) {
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
