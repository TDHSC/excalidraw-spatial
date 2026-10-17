use crate::geometry::{Point, Rect};
use crate::scene_graph::{Scene, Target};
use crate::schema::{json_num, Document};
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
            label, edge_label, ..
        } => {
            let _ = label.as_deref().map(normalize_text);
            let _ = edge_label.as_deref().map(normalize_text);
            let _ = (doc, now_ms);
            anyhow::bail!("not implemented")
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
        DrawCommands::Text { text, .. } => {
            let _ = normalize_text(text);
            anyhow::bail!("not implemented")
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
