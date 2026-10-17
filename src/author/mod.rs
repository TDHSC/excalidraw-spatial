use crate::geometry::{ceil_px, Point, Rect};
use crate::linter::text_factor;
use crate::mutator::ops::{relative_xy, RelativePlace};
use crate::mutator::sync_bindings::bound_text_xy;
use crate::scene_graph::{Scene, Target};
use crate::schema::{json_num, Document};
use crate::text_metrics;
use crate::{DrawCommands, DrawShape, PlaceAlign, StrokeStyle};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AuthorReport {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
}

pub fn fnv1a_u31(salt: &str, id: &str) -> i64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for byte in format!("{salt}:{id}").bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    ((hash % 2_147_483_647) as i64).max(1)
}

pub fn normalize_text(raw: &str) -> String {
    raw.replace("\\n", "\n")
}

pub fn validate_new_id(doc: &Document, scene: &Scene, id: &str) -> anyhow::Result<()> {
    if id.is_empty()
        || id.starts_with("group:")
        || id.chars().any(|ch| ch.is_whitespace() || ch == ',')
    {
        anyhow::bail!("invalid id \"{id}\"");
    }
    if doc.has_raw_id(id) || id_in_scene(scene, id) {
        anyhow::bail!("id \"{id}\" already exists");
    }
    Ok(())
}

fn id_in_scene(scene: &Scene, id: &str) -> bool {
    scene.component(id).is_some() || scene.container(id).is_some() || scene.connector(id).is_some()
}

pub fn derived_label_id(doc: &Document, base_id: &str) -> String {
    unused_suffixed(doc, &format!("{base_id}-label"))
}

pub fn derived_title_id(doc: &Document, base_id: &str) -> String {
    unused_suffixed(doc, &format!("{base_id}-title"))
}

pub fn derived_connector_id(doc: &Document, from_id: &str, to_id: &str) -> String {
    let from = from_id.replace(':', "_");
    let to = to_id.replace(':', "_");
    unused_suffixed(doc, &format!("{from}-{to}"))
}

fn unused_suffixed(doc: &Document, base: &str) -> String {
    if !doc.has_raw_id(base) {
        return base.to_string();
    }
    let mut k = 2;
    loop {
        let candidate = format!("{base}-{k}");
        if !doc.has_raw_id(&candidate) {
            return candidate;
        }
        k += 1;
    }
}

pub struct ElementStyle<'a> {
    pub bg: Option<&'a str>,
    pub stroke: Option<&'a str>,
    pub stroke_style: Option<&'a StrokeStyle>,
    pub frame_id: Option<&'a str>,
}

#[allow(dead_code)]
pub fn shape_element(
    id: &str,
    shape: &DrawShape,
    rect: Rect,
    now_ms: i64,
    style: &ElementStyle<'_>,
    bound_label: Option<&str>,
) -> serde_json::Value {
    let element_type = match shape {
        DrawShape::Stickynote => "rectangle",
        DrawShape::Rectangle => "rectangle",
        DrawShape::Ellipse => "ellipse",
        DrawShape::Diamond => "diamond",
    };
    let roundness = match shape {
        DrawShape::Rectangle | DrawShape::Diamond => Some(3),
        DrawShape::Ellipse => Some(2),
        DrawShape::Stickynote => None,
    };
    let background = style
        .bg
        .unwrap_or(if matches!(shape, DrawShape::Stickynote) {
            "#fff3bf"
        } else {
            "transparent"
        });
    let stroke_color = style.stroke.unwrap_or("#1e1e1e");
    let stroke_style = stroke_style_name(style.stroke_style);
    let bound = bound_label.map(|label_id| {
        serde_json::Value::Array(vec![serde_json::json!({"type": "text", "id": label_id})])
    });
    let mut el = common_element(CommonElement {
        id,
        element_type,
        rect,
        stroke_color,
        background,
        stroke_style: &stroke_style,
        frame_id: style.frame_id,
        roundness,
        bound_elements: bound,
        now_ms,
    });
    if matches!(shape, DrawShape::Stickynote) {
        if let Some(obj) = el.as_object_mut() {
            obj.insert("baseHeight".to_string(), json_num(rect.height()));
        }
    }
    el
}

#[allow(dead_code)]
pub struct TextStyle<'a> {
    pub font_size: f64,
    pub text_align: &'a str,
    pub vertical_align: &'a str,
    pub container_id: Option<&'a str>,
    pub frame_id: Option<&'a str>,
}

#[allow(dead_code)]
pub fn text_element(
    id: &str,
    rect: Rect,
    text: &str,
    now_ms: i64,
    style: &TextStyle<'_>,
) -> serde_json::Value {
    let mut el = common_element(CommonElement {
        id,
        element_type: "text",
        rect,
        stroke_color: "#1e1e1e",
        background: "transparent",
        stroke_style: "solid",
        frame_id: style.frame_id,
        roundness: None,
        bound_elements: None,
        now_ms,
    });
    if let Some(obj) = el.as_object_mut() {
        obj.insert(
            "text".to_string(),
            serde_json::Value::String(text.to_string()),
        );
        obj.insert("fontSize".to_string(), json_num(style.font_size));
        obj.insert(
            "fontFamily".to_string(),
            serde_json::Value::Number(serde_json::Number::from(5)),
        );
        obj.insert(
            "textAlign".to_string(),
            serde_json::Value::String(style.text_align.to_string()),
        );
        obj.insert(
            "verticalAlign".to_string(),
            serde_json::Value::String(style.vertical_align.to_string()),
        );
        obj.insert(
            "containerId".to_string(),
            style
                .container_id
                .map(|id| serde_json::Value::String(id.to_string()))
                .unwrap_or(serde_json::Value::Null),
        );
        obj.insert(
            "originalText".to_string(),
            serde_json::Value::String(text.to_string()),
        );
        obj.insert("autoResize".to_string(), serde_json::Value::Bool(true));
        obj.insert("lineHeight".to_string(), json_num(1.25));
        obj.insert("baseFontSize".to_string(), serde_json::Value::Null);
    }
    el
}

#[allow(dead_code)]
pub struct ArrowEnds {
    pub start: Option<serde_json::Value>,
    pub end: Option<serde_json::Value>,
    pub elbowed: bool,
    pub bound_label: Option<String>,
    pub frame_id: Option<String>,
}

#[allow(dead_code)]
pub fn arrow_element(
    id: &str,
    origin: Point,
    width: f64,
    height: f64,
    points: &[[f64; 2]],
    ends: &ArrowEnds,
    now_ms: i64,
) -> serde_json::Value {
    let bound = ends.bound_label.as_deref().map(|label_id| {
        serde_json::Value::Array(vec![serde_json::json!({"type": "text", "id": label_id})])
    });
    let roundness = if ends.elbowed { None } else { Some(2) };
    let mut el = common_element(CommonElement {
        id,
        element_type: "arrow",
        rect: Rect::from_xywh(origin.x, origin.y, width, height),
        stroke_color: "#1e1e1e",
        background: "transparent",
        stroke_style: "solid",
        frame_id: ends.frame_id.as_deref(),
        roundness,
        bound_elements: bound,
        now_ms,
    });
    if let Some(obj) = el.as_object_mut() {
        let pts = points
            .iter()
            .map(|p| serde_json::Value::Array(vec![json_num(p[0]), json_num(p[1])]))
            .collect();
        obj.insert("points".to_string(), serde_json::Value::Array(pts));
        obj.insert(
            "startBinding".to_string(),
            ends.start.clone().unwrap_or(serde_json::Value::Null),
        );
        obj.insert(
            "endBinding".to_string(),
            ends.end.clone().unwrap_or(serde_json::Value::Null),
        );
        obj.insert("startArrowhead".to_string(), serde_json::Value::Null);
        obj.insert(
            "endArrowhead".to_string(),
            serde_json::Value::String("arrow".to_string()),
        );
        obj.insert("elbowed".to_string(), serde_json::Value::Bool(ends.elbowed));
    }
    el
}

#[allow(dead_code)]
pub fn frame_element(id: &str, rect: Rect, name: &str, now_ms: i64) -> serde_json::Value {
    let mut el = common_element(CommonElement {
        id,
        element_type: "frame",
        rect,
        stroke_color: "#1e1e1e",
        background: "transparent",
        stroke_style: "solid",
        frame_id: None,
        roundness: None,
        bound_elements: None,
        now_ms,
    });
    if let Some(obj) = el.as_object_mut() {
        obj.insert(
            "name".to_string(),
            serde_json::Value::String(name.to_string()),
        );
    }
    el
}

fn stroke_style_name(style: Option<&StrokeStyle>) -> String {
    match style {
        Some(StrokeStyle::Dashed) => "dashed".to_string(),
        Some(StrokeStyle::Dotted) => "dotted".to_string(),
        Some(StrokeStyle::Solid) | None => "solid".to_string(),
    }
}

struct CommonElement<'a> {
    id: &'a str,
    element_type: &'a str,
    rect: Rect,
    stroke_color: &'a str,
    background: &'a str,
    stroke_style: &'a str,
    frame_id: Option<&'a str>,
    roundness: Option<i64>,
    bound_elements: Option<serde_json::Value>,
    now_ms: i64,
}

fn common_element(el: CommonElement<'_>) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert(
        "id".to_string(),
        serde_json::Value::String(el.id.to_string()),
    );
    obj.insert(
        "type".to_string(),
        serde_json::Value::String(el.element_type.to_string()),
    );
    obj.insert("x".to_string(), json_num(el.rect.x1));
    obj.insert("y".to_string(), json_num(el.rect.y1));
    obj.insert("width".to_string(), json_num(el.rect.width()));
    obj.insert("height".to_string(), json_num(el.rect.height()));
    obj.insert("angle".to_string(), json_num(0.0));
    obj.insert(
        "strokeColor".to_string(),
        serde_json::Value::String(el.stroke_color.to_string()),
    );
    obj.insert(
        "backgroundColor".to_string(),
        serde_json::Value::String(el.background.to_string()),
    );
    obj.insert(
        "fillStyle".to_string(),
        serde_json::Value::String("solid".to_string()),
    );
    obj.insert("strokeWidth".to_string(), json_num(2.0));
    obj.insert(
        "strokeStyle".to_string(),
        serde_json::Value::String(el.stroke_style.to_string()),
    );
    obj.insert("roughness".to_string(), json_num(1.0));
    obj.insert("opacity".to_string(), json_num(100.0));
    obj.insert("groupIds".to_string(), serde_json::Value::Array(Vec::new()));
    obj.insert(
        "frameId".to_string(),
        el.frame_id
            .map(|id| serde_json::Value::String(id.to_string()))
            .unwrap_or(serde_json::Value::Null),
    );
    obj.insert("index".to_string(), serde_json::Value::Null);
    obj.insert(
        "roundness".to_string(),
        match el.roundness {
            Some(kind) => serde_json::json!({"type": kind}),
            None => serde_json::Value::Null,
        },
    );
    obj.insert(
        "seed".to_string(),
        serde_json::Value::Number(serde_json::Number::from(fnv1a_u31("seed", el.id))),
    );
    obj.insert(
        "version".to_string(),
        serde_json::Value::Number(serde_json::Number::from(1)),
    );
    obj.insert(
        "versionNonce".to_string(),
        serde_json::Value::Number(serde_json::Number::from(fnv1a_u31("nonce", el.id))),
    );
    obj.insert("isDeleted".to_string(), serde_json::Value::Bool(false));
    obj.insert(
        "boundElements".to_string(),
        el.bound_elements.unwrap_or(serde_json::Value::Null),
    );
    obj.insert(
        "updated".to_string(),
        serde_json::Value::Number(serde_json::Number::from(el.now_ms)),
    );
    obj.insert("link".to_string(), serde_json::Value::Null);
    obj.insert("locked".to_string(), serde_json::Value::Bool(false));
    obj.insert(
        "created".to_string(),
        serde_json::Value::Number(serde_json::Number::from(el.now_ms)),
    );
    serde_json::Value::Object(obj)
}

pub fn apply_draw(
    doc: &mut Document,
    cmd: &DrawCommands,
    now_ms: i64,
) -> anyhow::Result<AuthorReport> {
    match cmd {
        DrawCommands::Init { .. } => Ok(AuthorReport::default()),
        DrawCommands::Node {
            id,
            label,
            shape,
            x,
            y,
            right_of,
            left_of,
            below,
            above,
            gap,
            align,
            in_container,
            from,
            edge_label,
            width,
            height,
            bg,
            stroke,
            stroke_style,
            ..
        } => {
            // `--in` and the connector itself are wired in T35 and T36.
            // Gap still accounts for `--edge-label` when `--from` is set.
            let _ = in_container;
            let label = label.as_deref().map(normalize_text);
            let edge_label = edge_label.as_deref().map(normalize_text);
            let gap_label = if from.is_some() {
                edge_label.as_deref()
            } else {
                None
            };
            draw_node(
                doc,
                NodeDraw {
                    id,
                    label: label.as_deref(),
                    shape,
                    place: Placement {
                        x: *x,
                        y: *y,
                        right_of: right_of.as_deref(),
                        left_of: left_of.as_deref(),
                        below: below.as_deref(),
                        above: above.as_deref(),
                        gap: *gap,
                        align: align.as_ref(),
                        edge_label: gap_label,
                    },
                    width: *width,
                    height: *height,
                    style: ElementStyle {
                        bg: bg.as_deref(),
                        stroke: stroke.as_deref(),
                        stroke_style: stroke_style.as_ref(),
                        frame_id: None,
                    },
                    now_ms,
                },
            )
        }
        DrawCommands::Connect { label, .. } => {
            let _ = label.as_deref().map(normalize_text);
            anyhow::bail!("not implemented")
        }
        DrawCommands::Zone { title, .. } => {
            let _ = normalize_text(title);
            anyhow::bail!("not implemented")
        }
        DrawCommands::Frame { name, .. } => {
            let _ = name.as_deref().map(normalize_text);
            anyhow::bail!("not implemented")
        }
        DrawCommands::Text {
            id,
            text,
            x,
            y,
            right_of,
            left_of,
            below,
            above,
            gap,
            align,
            font_size,
            ..
        } => {
            let text = normalize_text(text);
            draw_text(
                doc,
                TextDraw {
                    id,
                    text: &text,
                    place: Placement {
                        x: *x,
                        y: *y,
                        right_of: right_of.as_deref(),
                        left_of: left_of.as_deref(),
                        below: below.as_deref(),
                        above: above.as_deref(),
                        gap: *gap,
                        align: align.as_ref(),
                        edge_label: None,
                    },
                    font_size: *font_size,
                    now_ms,
                },
            )
        }
        DrawCommands::Label { text, .. } => {
            let _ = normalize_text(text);
            anyhow::bail!("not implemented")
        }
        DrawCommands::Route { .. } | DrawCommands::Remove { .. } | DrawCommands::Batch { .. } => {
            anyhow::bail!("not implemented")
        }
    }
}

fn shape_type_name(shape: &DrawShape) -> &'static str {
    match shape {
        DrawShape::Ellipse => "ellipse",
        DrawShape::Diamond => "diamond",
        DrawShape::Rectangle | DrawShape::Stickynote => "rectangle",
    }
}

fn node_size(
    shape: &DrawShape,
    label: Option<&str>,
    width: Option<f64>,
    height: Option<f64>,
) -> (f64, f64, f64, f64) {
    let (tw, th) = match label {
        Some(text) => text_metrics::estimate(text, 20.0, 1.25),
        None => (0.0, 0.0),
    };
    let (auto_w, auto_h) = if label.is_some() {
        let f = text_factor(shape_type_name(shape));
        (
            160.0_f64.max(ceil_px((tw + 48.0) / f)),
            80.0_f64.max(ceil_px((th + 32.0) / f)),
        )
    } else {
        (160.0, 80.0)
    };
    (width.unwrap_or(auto_w), height.unwrap_or(auto_h), tw, th)
}

fn anchor_rect(scene: &Scene, id: &str) -> anyhow::Result<Rect> {
    match scene.resolve(id)? {
        Target::Component(resolved) => scene
            .component(&resolved)
            .map(|component| component.rect)
            .ok_or_else(|| anyhow::anyhow!("unknown id \"{id}\"")),
        Target::Container(resolved) => scene
            .container(&resolved)
            .map(|container| container.rect)
            .ok_or_else(|| anyhow::anyhow!("unknown id \"{id}\"")),
        Target::Connector(_) => anyhow::bail!("unknown id \"{id}\""),
    }
}

struct Placement<'a> {
    x: Option<f64>,
    y: Option<f64>,
    right_of: Option<&'a str>,
    left_of: Option<&'a str>,
    below: Option<&'a str>,
    above: Option<&'a str>,
    gap: Option<f64>,
    align: Option<&'a PlaceAlign>,
    edge_label: Option<&'a str>,
}

fn place_xy(scene: &Scene, w: f64, h: f64, place: &Placement<'_>) -> anyhow::Result<(f64, f64)> {
    if let (Some(x), Some(y)) = (place.x, place.y) {
        return Ok((x, y));
    }
    let relative = place
        .right_of
        .or(place.left_of)
        .or(place.below)
        .or(place.above);
    if let Some(anchor_id) = relative {
        let anchor = anchor_rect(scene, anchor_id)?;
        let gap = default_gap(
            place.gap,
            place.edge_label,
            place.right_of.is_some() || place.left_of.is_some(),
        );
        let align = place.align.unwrap_or(&PlaceAlign::Center);
        return Ok(relative_xy(RelativePlace {
            anchor,
            w,
            h,
            right_of: place.right_of.is_some(),
            left_of: place.left_of.is_some(),
            below: place.below.is_some(),
            above: place.above.is_some(),
            gap,
            align,
        }));
    }
    if scene.components.is_empty() && scene.containers.is_empty() {
        return Ok((100.0, 100.0));
    }
    anyhow::bail!(
        "node placement requires --x and --y or one of --right-of, --left-of, --below, --above"
    )
}

fn default_gap(gap: Option<f64>, edge_label: Option<&str>, horizontal_relation: bool) -> f64 {
    if let Some(gap) = gap {
        return gap;
    }
    if let Some(edge_label) = edge_label {
        let (el_w, el_h) = text_metrics::estimate(edge_label, 16.0, 1.25);
        let dim = if horizontal_relation { el_w } else { el_h };
        return 60.0_f64.max(ceil_px(dim + 24.0));
    }
    60.0
}

struct NodeDraw<'a> {
    id: &'a str,
    label: Option<&'a str>,
    shape: &'a DrawShape,
    place: Placement<'a>,
    width: Option<f64>,
    height: Option<f64>,
    style: ElementStyle<'a>,
    now_ms: i64,
}

fn draw_node(doc: &mut Document, node: NodeDraw<'_>) -> anyhow::Result<AuthorReport> {
    let scene = Scene::build(doc);
    validate_new_id(doc, &scene, node.id)?;
    let (w, h, tw, th) = node_size(node.shape, node.label, node.width, node.height);
    let (x, y) = place_xy(&scene, w, h, &node.place)?;
    let rect = Rect::from_xywh(x, y, w, h);
    let label_id = node.label.map(|_| derived_label_id(doc, node.id));
    doc.push_element(shape_element(
        node.id,
        node.shape,
        rect,
        node.now_ms,
        &node.style,
        label_id.as_deref(),
    ));
    if let (Some(text), Some(label_id)) = (node.label, label_id.as_deref()) {
        let (lx, ly) = bound_text_xy(&rect, tw, th, Some("center"), Some("middle"));
        doc.push_element(text_element(
            label_id,
            Rect::from_xywh(lx, ly, tw, th),
            text,
            node.now_ms,
            &TextStyle {
                font_size: 20.0,
                text_align: "center",
                vertical_align: "middle",
                container_id: Some(node.id),
                frame_id: node.style.frame_id,
            },
        ));
    }
    Ok(AuthorReport {
        added: 1,
        ..AuthorReport::default()
    })
}

struct TextDraw<'a> {
    id: &'a str,
    text: &'a str,
    place: Placement<'a>,
    font_size: Option<f64>,
    now_ms: i64,
}

fn draw_text(doc: &mut Document, text: TextDraw<'_>) -> anyhow::Result<AuthorReport> {
    let scene = Scene::build(doc);
    validate_new_id(doc, &scene, text.id)?;
    let font_size = text.font_size.unwrap_or(20.0);
    let (tw, th) = text_metrics::estimate(text.text, font_size, 1.25);
    let (x, y) = place_xy(&scene, tw, th, &text.place)?;
    doc.push_element(text_element(
        text.id,
        Rect::from_xywh(x, y, tw, th),
        text.text,
        text.now_ms,
        &TextStyle {
            font_size,
            text_align: "left",
            vertical_align: "top",
            container_id: None,
            frame_id: None,
        },
    ));
    Ok(AuthorReport {
        added: 1,
        ..AuthorReport::default()
    })
}

#[allow(dead_code)]
fn _scene_target(scene: &Scene, id: &str) -> anyhow::Result<Target> {
    scene.resolve(id)
}

#[allow(dead_code)]
fn _align(align: PlaceAlign) -> PlaceAlign {
    align
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;

    fn err_of(result: anyhow::Result<()>) -> String {
        result.expect_err("expected error").to_string()
    }

    #[test]
    fn fnv1a_u31_matches_known_seeds_and_nonces() {
        assert_eq!(fnv1a_u31("seed", "api"), 1_269_111_100);
        assert_eq!(fnv1a_u31("nonce", "api"), 1_162_108_891);
        assert_eq!(fnv1a_u31("seed", "client"), 2_060_559_059);
        assert_eq!(fnv1a_u31("nonce", "client"), 491_086_952);
        assert_eq!(fnv1a_u31("seed", "client-label"), 1_838_114_862);
        assert_eq!(fnv1a_u31("nonce", "client-label"), 290_245_772);
    }

    #[test]
    fn validate_new_id_rejects_invalid_and_existing_ids() {
        let doc = Document::parse(
            r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"id": "client", "type": "rectangle", "x": 0, "y": 0, "width": 10, "height": 10},
    {"id": "ghost", "type": "rectangle", "isDeleted": true}
  ]
}"#,
        )
        .expect("parse");
        let scene = Scene::build(&doc);

        for id in ["", "group:abc", "a b", "a,b"] {
            let msg = err_of(validate_new_id(&doc, &scene, id));
            assert_eq!(msg, format!("invalid id \"{id}\""));
        }
        for id in ["client", "ghost"] {
            let msg = err_of(validate_new_id(&doc, &scene, id));
            assert_eq!(msg, format!("id \"{id}\" already exists"));
        }
        validate_new_id(&doc, &scene, "fresh").expect("unused id");
    }

    #[test]
    fn normalize_text_converts_backslash_n_escape() {
        assert_eq!(normalize_text(r"Fraud / 3DS\nCheck"), "Fraud / 3DS\nCheck");
        assert_eq!(normalize_text("already\nline"), "already\nline");
    }

    #[test]
    fn derived_ids_skip_ids_already_present_in_the_raw_array() {
        let doc = Document::parse(
            r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"id": "box-label", "type": "text"},
    {"id": "zone-title", "type": "text"},
    {"id": "a-b", "type": "arrow"}
  ]
}"#,
        )
        .expect("parse");
        assert_eq!(derived_label_id(&doc, "box"), "box-label-2");
        assert_eq!(derived_title_id(&doc, "zone"), "zone-title-2");
        assert_eq!(derived_connector_id(&doc, "a", "b"), "a-b-2");
        assert_eq!(derived_connector_id(&doc, "group:a", "b"), "group_a-b");
    }

    #[allow(clippy::too_many_arguments)]
    fn node_cmd(
        id: &str,
        label: Option<&str>,
        shape: DrawShape,
        x: Option<f64>,
        y: Option<f64>,
        right_of: Option<&str>,
        left_of: Option<&str>,
        below: Option<&str>,
        above: Option<&str>,
        gap: Option<f64>,
        align: Option<PlaceAlign>,
        width: Option<f64>,
        height: Option<f64>,
        bg: Option<&str>,
        stroke: Option<&str>,
        stroke_style: Option<StrokeStyle>,
    ) -> DrawCommands {
        DrawCommands::Node {
            file: "mem.excalidraw".to_string(),
            id: id.to_string(),
            label: label.map(str::to_string),
            shape,
            x,
            y,
            right_of: right_of.map(str::to_string),
            left_of: left_of.map(str::to_string),
            below: below.map(str::to_string),
            above: above.map(str::to_string),
            gap,
            align,
            in_container: None,
            from: None,
            edge_label: None,
            elbow: false,
            width,
            height,
            bg: bg.map(str::to_string),
            stroke: stroke.map(str::to_string),
            stroke_style,
            output: crate::OutputArgs {
                in_place: false,
                output: None,
            },
        }
    }

    fn empty_doc() -> Document {
        Document::empty()
    }

    fn element_num(el: &serde_json::Value, key: &str) -> f64 {
        el[key].as_f64().expect(key)
    }

    #[test]
    fn draw_node_places_relative_to_anchor_with_align_gap_and_style() {
        let mut doc = empty_doc();
        apply_draw(
            &mut doc,
            &node_cmd(
                "anchor",
                Some("Anchor"),
                DrawShape::Rectangle,
                Some(0.0),
                Some(0.0),
                None,
                None,
                None,
                None,
                None,
                None,
                Some(200.0),
                Some(100.0),
                None,
                None,
                None,
            ),
            1_700_000_000_000,
        )
        .expect("anchor");

        // --left-of, align start: x = 0 - 60 - 160 = -220, y = 0.
        apply_draw(
            &mut doc,
            &node_cmd(
                "left",
                Some("Left"),
                DrawShape::Rectangle,
                None,
                None,
                None,
                Some("anchor"),
                None,
                None,
                None,
                Some(PlaceAlign::Start),
                None,
                None,
                None,
                None,
                None,
            ),
            1_700_000_000_000,
        )
        .expect("left");
        let left = doc.value["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|el| el["id"] == "left")
            .expect("left element");
        assert_eq!(element_num(left, "x"), -220.0);
        assert_eq!(element_num(left, "y"), 0.0);
        assert_eq!(element_num(left, "width"), 160.0);
        assert_eq!(element_num(left, "height"), 80.0);

        // --above, align end, gap 80: y = 0 - 80 - 80 = -160, x = 200 - 160 = 40.
        apply_draw(
            &mut doc,
            &node_cmd(
                "up",
                Some("Up"),
                DrawShape::Rectangle,
                None,
                None,
                None,
                None,
                None,
                Some("anchor"),
                Some(80.0),
                Some(PlaceAlign::End),
                None,
                None,
                None,
                None,
                None,
            ),
            1_700_000_000_000,
        )
        .expect("above");
        let up = doc.value["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|el| el["id"] == "up")
            .expect("up element");
        assert_eq!(element_num(up, "x"), 40.0);
        assert_eq!(element_num(up, "y"), -160.0);

        // --below, align center: y = 100 + 60 = 160, x = (200 - 160) / 2 = 20.
        apply_draw(
            &mut doc,
            &node_cmd(
                "down",
                Some("Down"),
                DrawShape::Rectangle,
                None,
                None,
                None,
                None,
                Some("anchor"),
                None,
                None,
                Some(PlaceAlign::Center),
                None,
                None,
                None,
                None,
                None,
            ),
            1_700_000_000_000,
        )
        .expect("below");
        let down = doc.value["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|el| el["id"] == "down")
            .expect("down element");
        assert_eq!(element_num(down, "x"), 20.0);
        assert_eq!(element_num(down, "y"), 160.0);

        // --right-of is covered with explicit size and style, including negative origin.
        let mut styled = empty_doc();
        apply_draw(
            &mut styled,
            &node_cmd(
                "origin",
                None,
                DrawShape::Rectangle,
                Some(-40.0),
                Some(-20.0),
                None,
                None,
                None,
                None,
                None,
                None,
                Some(200.0),
                Some(100.0),
                None,
                None,
                None,
            ),
            1_700_000_000_000,
        )
        .expect("origin");
        apply_draw(
            &mut styled,
            &node_cmd(
                "styled",
                Some("Styled"),
                DrawShape::Rectangle,
                None,
                None,
                Some("origin"),
                None,
                None,
                None,
                Some(80.0),
                Some(PlaceAlign::Center),
                Some(200.0),
                Some(100.0),
                Some("#e7f5ff"),
                Some("#1971c2"),
                Some(StrokeStyle::Dashed),
            ),
            1_700_000_000_000,
        )
        .expect("styled");
        let styled_el = styled.value["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|el| el["id"] == "styled")
            .expect("styled element");
        // origin x2 = 160; gap 80 => x = 240. Heights match, so center y stays -20.
        assert_eq!(element_num(styled_el, "x"), 240.0);
        assert_eq!(element_num(styled_el, "y"), -20.0);
        assert_eq!(element_num(styled_el, "width"), 200.0);
        assert_eq!(element_num(styled_el, "height"), 100.0);
        assert_eq!(styled_el["backgroundColor"], "#e7f5ff");
        assert_eq!(styled_el["strokeColor"], "#1971c2");
        assert_eq!(styled_el["strokeStyle"], "dashed");
    }

    #[test]
    fn draw_node_on_non_empty_canvas_requires_placement() {
        let mut doc = empty_doc();
        apply_draw(
            &mut doc,
            &node_cmd(
                "anchor",
                Some("Anchor"),
                DrawShape::Rectangle,
                Some(0.0),
                Some(0.0),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            1_700_000_000_000,
        )
        .expect("anchor");
        let err = apply_draw(
            &mut doc,
            &node_cmd(
                "orphan",
                Some("Orphan"),
                DrawShape::Rectangle,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            1_700_000_000_000,
        )
        .expect_err("placement required")
        .to_string();
        assert_eq!(
            err,
            "node placement requires --x and --y or one of --right-of, --left-of, --below, --above"
        );
    }

    #[test]
    fn draw_node_normalizes_newline_escapes_without_trimming() {
        let mut doc = empty_doc();
        apply_draw(
            &mut doc,
            &node_cmd(
                "dec",
                Some("  Fraud / 3DS\nCheck  "),
                DrawShape::Diamond,
                Some(0.0),
                Some(0.0),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            1_700_000_000_000,
        )
        .expect("diamond");
        let label = doc.value["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|el| el["id"] == "dec-label")
            .expect("label");
        assert_eq!(label["text"], "  Fraud / 3DS\nCheck  ");
        assert_eq!(label["originalText"], "  Fraud / 3DS\nCheck  ");
    }

    #[test]
    fn shape_and_text_builders_emit_fixture_key_order_and_json_numbers() {
        let shape = shape_element(
            "client",
            &DrawShape::Rectangle,
            Rect::from_xywh(100.0, 100.5, 160.0, 80.0),
            1_700_000_000_000,
            &ElementStyle {
                bg: None,
                stroke: None,
                stroke_style: None,
                frame_id: None,
            },
            Some("client-label"),
        );
        assert_eq!(
            shape
                .as_object()
                .expect("object")
                .keys()
                .collect::<Vec<_>>(),
            vec![
                "id",
                "type",
                "x",
                "y",
                "width",
                "height",
                "angle",
                "strokeColor",
                "backgroundColor",
                "fillStyle",
                "strokeWidth",
                "strokeStyle",
                "roughness",
                "opacity",
                "groupIds",
                "frameId",
                "index",
                "roundness",
                "seed",
                "version",
                "versionNonce",
                "isDeleted",
                "boundElements",
                "updated",
                "link",
                "locked",
                "created",
            ]
        );
        assert_eq!(shape["seed"], 2_060_559_059);
        assert_eq!(shape["versionNonce"], 491_086_952);
        assert_eq!(shape["version"], 1);
        assert_eq!(shape["index"], serde_json::Value::Null);
        assert_eq!(shape["y"], serde_json::json!(100.5));
        assert!(shape["angle"].as_i64().is_some());
        assert_eq!(shape["roundness"], serde_json::json!({"type": 3}));

        let text = text_element(
            "client-label",
            Rect::from_xywh(144.0, 127.5, 72.0, 25.0),
            "Client",
            1_700_000_000_000,
            &TextStyle {
                font_size: 20.0,
                text_align: "center",
                vertical_align: "middle",
                container_id: Some("client"),
                frame_id: None,
            },
        );
        let keys: Vec<_> = text.as_object().expect("object").keys().cloned().collect();
        assert_eq!(
            &keys[keys.len() - 10..],
            [
                "text".to_string(),
                "fontSize".to_string(),
                "fontFamily".to_string(),
                "textAlign".to_string(),
                "verticalAlign".to_string(),
                "containerId".to_string(),
                "originalText".to_string(),
                "autoResize".to_string(),
                "lineHeight".to_string(),
                "baseFontSize".to_string(),
            ]
        );
        assert_eq!(text["seed"], 1_838_114_862);
        assert_eq!(text["versionNonce"], 290_245_772);
        assert_eq!(text["containerId"], "client");
    }
}
