use excalidraw_spatial::geometry::{Point, Rect};
use excalidraw_spatial::schema::{json_num, Document};

const FIXTURES: &[&str] = &[
    "basic_flow",
    "overlap",
    "swimlanes",
    "arrows",
    "text_layout",
    "icon_group",
];

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{name}.excalidraw"))
}

fn load_src(name: &str) -> String {
    std::fs::read_to_string(fixture_path(name)).expect("fixture")
}

fn element_value<'a>(doc: &'a Document, id: &str) -> &'a serde_json::Value {
    doc.value["elements"]
        .as_array()
        .expect("elements array")
        .iter()
        .find(|e| e["id"] == id)
        .expect("element id")
}

#[test]
fn parse_roundtrips_all_fixtures_byte_for_byte() {
    for name in FIXTURES {
        let src = load_src(name);
        let doc = Document::parse(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            doc.to_json_string(),
            src,
            "{name} round-trip must be byte-identical"
        );
        assert!(
            !src.ends_with('\n'),
            "{name} fixture is expected not to end with a newline"
        );
    }
}

#[test]
fn parse_preserves_trailing_newline() {
    for name in FIXTURES {
        let src = load_src(name) + "\n";
        let doc = Document::parse(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            doc.to_json_string(),
            src,
            "{name} with trailing newline must round-trip"
        );
    }
}

#[test]
fn fixture_element_counts() {
    let expected = [
        ("basic_flow", 10usize),
        ("overlap", 4),
        ("swimlanes", 13),
        ("arrows", 13),
        ("text_layout", 8),
        ("icon_group", 11),
    ];
    for (name, n) in expected {
        let doc = Document::parse(&load_src(name)).unwrap();
        assert_eq!(doc.elements.len(), n, "{name}");
    }
}

#[test]
fn icon_group_old_binding_format() {
    let doc = Document::parse(&load_src("icon_group")).unwrap();
    let el = doc.element("Lg7Hs4Kd1Fp8Wq5Er2TyU").expect("arrow");
    let binding = el.start_binding.as_ref().expect("start_binding");
    assert_eq!(binding.element_id.as_deref(), Some("Mx9Cv2Bn5Lk8Jh1Gf4DsA"));
    assert_eq!(binding.fixed_point, None);
    assert_eq!(el.elbowed, None);
}

#[test]
fn arrows_arr_near_abs_points_and_rect() {
    let doc = Document::parse(&load_src("arrows")).unwrap();
    let el = doc.element("arr_near").expect("arr_near");
    assert_eq!(
        el.abs_points(),
        vec![
            Point { x: 290.0, y: 186.0 },
            Point { x: 290.0, y: 340.0 },
            Point { x: 432.0, y: 340.0 },
        ]
    );
    assert_eq!(
        el.rect(),
        Rect {
            x1: 290.0,
            y1: 186.0,
            x2: 432.0,
            y2: 340.0
        }
    );
}

#[test]
fn set_num_and_finish_updates_client_and_leaves_others() {
    let src = load_src("basic_flow");
    let original: serde_json::Value = serde_json::from_str(&src).unwrap();
    let mut doc = Document::parse(&src).unwrap();
    doc.set_num("client", "x", 130.0);
    doc.finish(1_700_000_000_000);

    assert_eq!(doc.element("client").unwrap().x(), 130.0);

    let out = doc.to_json_string();
    assert!(
        !out.contains("130.0"),
        "x must be written as integer 130, not 130.0:\n{out}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    let client = parsed["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "client")
        .unwrap();
    assert_eq!(client["x"].as_i64(), Some(130));
    assert_eq!(client["version"].as_i64(), Some(4));
    assert_eq!(client["versionNonce"].as_i64(), Some(1_408_551_190));
    assert_eq!(client["updated"].as_i64(), Some(1_700_000_000_000));

    let orig_els = original["elements"].as_array().unwrap();
    let new_els = parsed["elements"].as_array().unwrap();
    assert_eq!(orig_els.len(), new_els.len());
    let mut others = 0;
    for (a, b) in orig_els.iter().zip(new_els) {
        if a["id"] == "client" {
            continue;
        }
        assert_eq!(a, b);
        others += 1;
    }
    assert_eq!(others, 9);
}

#[test]
fn set_num_writes_rounded_float_and_integer() {
    let mut doc = Document::parse(&load_src("basic_flow")).unwrap();
    doc.set_num("client", "x", 12.3456);
    let x = &element_value(&doc, "client")["x"];
    assert_eq!(x.as_f64(), Some(12.35));
    assert_eq!(x.as_i64(), None);
    let pretty = doc.to_json_string();
    assert!(
        pretty.contains("\"x\": 12.35"),
        "pretty JSON should contain 12.35:\n{pretty}"
    );
    assert!(
        !pretty.contains("12.3456"),
        "pretty JSON must not contain the unrounded value"
    );

    doc.set_num("client", "x", 7.0);
    let x = &element_value(&doc, "client")["x"];
    assert_eq!(x.as_i64(), Some(7));
    let pretty = doc.to_json_string();
    assert!(pretty.contains("\"x\": 7"), "integer 7:\n{pretty}");
    assert!(
        !pretty.contains("\"x\": 7.0"),
        "must not write 7.0:\n{pretty}"
    );
}

#[test]
fn json_num_rounds_and_chooses_int_or_float() {
    assert_eq!(json_num(12.3456).to_string(), "12.35");
    assert_eq!(json_num(7.0).to_string(), "7");
    assert_eq!(json_num(1.0).to_string(), "1");
    assert_eq!(json_num(-7.0).to_string(), "-7");
    assert_eq!(json_num(-0.0).to_string(), "0");
}

#[test]
fn set_num_appends_missing_key_last() {
    let mut doc = Document::parse(&load_src("basic_flow")).unwrap();
    let before: Vec<String> = element_value(&doc, "client")
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert!(!before.iter().any(|k| k == "foo"));

    doc.set_num("client", "foo", 1.0);

    let obj = element_value(&doc, "client").as_object().unwrap();
    let keys: Vec<&String> = obj.keys().collect();
    assert_eq!(keys.last().copied().map(String::as_str), Some("foo"));
    assert_eq!(obj["foo"].as_i64(), Some(1));
    assert_eq!(
        keys[..keys.len() - 1],
        before.iter().collect::<Vec<_>>()[..]
    );
}

#[test]
fn parse_rejects_illegal_and_malformed_documents() {
    assert!(Document::parse("not json").is_err());
    assert!(Document::parse(r#"{"elements": 3}"#).is_err());
    assert!(Document::parse("[1, 2]").is_err());
}

#[test]
fn parse_skips_element_missing_id_and_records_source_z() {
    let src = r#"{"elements":[{"type":"rectangle","x":1},{"id":"ok","type":"rectangle"}]}"#;
    let doc = Document::parse(src).unwrap();
    assert_eq!(doc.elements.len(), 1);
    assert_eq!(doc.elements[0].id, "ok");
    assert_eq!(doc.elements[0].z, 1);
    assert_eq!(doc.value["elements"].as_array().map(|a| a.len()), Some(2));
}

#[test]
fn set_points_writes_json_num_and_finish_bumps_version() {
    let mut doc = Document::parse(&load_src("arrows")).unwrap();
    let old_version = element_value(&doc, "arr_near")["version"].as_i64().unwrap();
    doc.set_points("arr_near", &[[0.0, 0.0], [0.1234, 1.0]]);
    let pts = &element_value(&doc, "arr_near")["points"];
    assert_eq!(pts[0][0].as_i64(), Some(0));
    assert_eq!(pts[0][1].as_i64(), Some(0));
    assert_eq!(pts[1][0].as_f64(), Some(0.12));
    assert_eq!(pts[1][0].as_i64(), None);
    assert_eq!(pts[1][1].as_i64(), Some(1));

    doc.finish(42);
    assert_eq!(
        element_value(&doc, "arr_near")["version"].as_i64(),
        Some(old_version + 1)
    );
    assert_eq!(
        doc.element("arr_near").unwrap().points,
        Some(vec![[0.0, 0.0], [0.12, 1.0]])
    );
}

#[test]
fn load_from_fixture_path_matches_parse() {
    let src = load_src("overlap");
    let path = fixture_path("overlap");
    let loaded = Document::load(path.to_str().unwrap()).unwrap();
    assert_eq!(loaded.to_json_string(), src);
    assert_eq!(loaded.elements.len(), 4);
}

#[test]
fn raw_element_defaulting_methods() {
    let doc = Document::parse(r#"{"elements":[{"id":"a","type":"rectangle"}]}"#).unwrap();
    let el = doc.element("a").unwrap();
    assert_eq!(el.x(), 0.0);
    assert_eq!(el.y(), 0.0);
    assert_eq!(el.width(), 0.0);
    assert_eq!(el.height(), 0.0);
    assert_eq!(el.stroke_width(), 2.0);
    assert_eq!(el.font_size(), 20.0);
    assert_eq!(el.line_height(), 1.25);
    assert!(!el.is_deleted());
    assert!(el.group_ids().is_empty());
    assert_eq!(el.outer_group(), None);
    assert_eq!(el.abs_points(), vec![Point { x: 0.0, y: 0.0 }]);
    assert_eq!(
        el.rect(),
        Rect {
            x1: 0.0,
            y1: 0.0,
            x2: 0.0,
            y2: 0.0
        }
    );
}
