use std::fs::File;
use std::process::{Command, Stdio};

use excalidraw_spatial::formatter::ai_report::format_inspect;
use excalidraw_spatial::formatter::ascii_map::render;
use excalidraw_spatial::geometry::{Point, Rect};
use excalidraw_spatial::scene_graph::{
    Attach, Component, ComponentKind, Connector, Container, ContainerKind, Scene,
};
use excalidraw_spatial::schema::Document;
const FIXTURES: [&str; 6] = [
    "arrows",
    "basic_flow",
    "icon_group",
    "overlap",
    "swimlanes",
    "text_layout",
];

fn run(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_excal"))
        .args(args)
        .output()
        .expect("failed to execute excal");
    let code = output.status.code().expect("process terminated by signal");
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr utf-8");
    (code, stdout, stderr)
}

fn scene(kind: ComponentKind, id: &str, rect: (f64, f64, f64, f64)) -> Scene {
    Scene {
        components: vec![component(kind, id, rect)],
        containers: Vec::new(),
        connectors: Vec::new(),
        notes: Vec::new(),
    }
}

fn component(kind: ComponentKind, id: &str, rect: (f64, f64, f64, f64)) -> Component {
    Component {
        id: id.to_string(),
        kind,
        type_name: match kind {
            ComponentKind::Shape => "rectangle".to_string(),
            ComponentKind::Group => "group".to_string(),
            ComponentKind::Text => "text".to_string(),
        },
        rect: Rect::from_xywh(rect.0, rect.1, rect.2, rect.3),
        label: None,
        label_element: None,
        label_size: None,
        elements: vec![id.to_string()],
        parent: None,
        frame_id: None,
        z: 0,
    }
}

#[test]
fn inspect_all_fixtures_match_golden_and_exit_0() {
    for name in FIXTURES {
        let (code, stdout, stderr) =
            run(&["inspect", &format!("tests/fixtures/{name}.excalidraw")]);
        let expected = std::fs::read_to_string(format!("tests/expected/{name}.inspect.txt"))
            .unwrap_or_else(|err| panic!("read expected {name}: {err}"));
        assert_eq!(code, 0, "{name} stderr:\n{stderr}");
        assert_eq!(stdout, expected, "{name}");
    }
}

#[test]
fn inspect_basic_flow_ascii_is_five_map_lines() {
    let (code, stdout, stderr) = run(&[
        "inspect",
        "tests/fixtures/basic_flow.excalidraw",
        "--format",
        "ascii",
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        stdout,
        "\
AAAAAAAAAAAAAAAA        BBBBBBBBBBBBBBBB        CCCCCCCCCCCCCCCC
AAAAAAAAAAAAAAAA********BBBBBBBBBBBBBBBB********CCCCCCCCCCCCCCCC
AAAAAAAAAAAAAAAA        BBBBBBBBBBBBBBBB        CCCCCCCCCCCCCCCC
AAAAAAAAAAAAAAAA        BBBBBBBBBBBBBBBB        CCCCCCCCCCCCCCCC
A=client B=api C=svc
"
    );
}

#[test]
fn inspect_stdin_title_uses_stdin_and_run_commands_use_file() {
    let fixture = File::open("tests/fixtures/basic_flow.excalidraw").expect("open fixture");
    let output = Command::new(env!("CARGO_BIN_EXE_excal"))
        .args(["inspect", "-"])
        .stdin(Stdio::from(fixture))
        .output()
        .expect("failed to execute excal");
    let code = output.status.code().expect("process terminated by signal");
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    assert_eq!(code, 0);
    let expected = include_str!("expected/basic_flow.inspect.txt").replacen(
        "tests/fixtures/basic_flow.excalidraw",
        "<stdin>",
        1,
    );
    assert_eq!(stdout, expected);
    assert!(stdout.starts_with("# excal inspect <stdin>\n"));
}

#[test]
fn inspect_empty_document_reports_empty_canvas_and_none_sections() {
    let path = std::env::temp_dir().join("excal-inspect-empty.excalidraw");
    std::fs::write(&path, r#"{"type":"excalidraw","version":2,"elements":[]}"#)
        .expect("write empty fixture");
    let (code, stdout, stderr) = run(&["inspect", path.to_str().expect("path")]);
    assert_eq!(code, 0, "{stderr}");
    let mut lines = stdout.lines();
    assert!(lines.next().unwrap().starts_with("# excal inspect "));
    assert_eq!(
        lines.next(),
        Some("canvas (empty) | components: 0, containers: 0, connectors: 0 | errors: 0, warnings: 0, info: 0")
    );
    let rest: Vec<_> = lines
        .filter(|line| !line.is_empty() && !line.starts_with("## "))
        .collect();
    assert_eq!(
        rest,
        ["(none)", "(none)", "(none)", "(none)", "(none)", "(none)", "(none)"]
    );
}

fn inspect_json(args: &[&str]) -> serde_json::Value {
    let (code, stdout, stderr) = run(args);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.ends_with('\n'),
        "inspect json must end with a newline"
    );
    serde_json::from_str(&stdout).unwrap_or_else(|err| panic!("inspect json: {err}\n{stdout}"))
}

fn assert_json_integers(value: &serde_json::Value) {
    match value {
        serde_json::Value::Number(n) => {
            assert!(
                n.is_i64() || n.is_u64(),
                "coordinates, sizes, gaps, and distances must be JSON integers, got {n}"
            );
        }
        serde_json::Value::Array(items) => {
            for item in items {
                assert_json_integers(item);
            }
        }
        serde_json::Value::Object(map) => {
            for item in map.values() {
                assert_json_integers(item);
            }
        }
        _ => {}
    }
}

#[test]
fn inspect_json_basic_flow_reports_neighbor_gap_and_bound_attach() {
    let v = inspect_json(&[
        "inspect",
        "tests/fixtures/basic_flow.excalidraw",
        "--format",
        "json",
    ]);
    assert_eq!(v["file"], "tests/fixtures/basic_flow.excalidraw");
    assert_eq!(v["components"].as_array().map(Vec::len), Some(3));
    assert_eq!(
        v["components"][1]["neighbors"]["left"],
        serde_json::json!({"id": "client", "gap": 100})
    );
    assert_eq!(
        v["connectors"][0]["start"],
        serde_json::json!({"attach": "bound", "target": "client", "element": "client"})
    );
    assert_json_integers(&v);
}

#[test]
fn inspect_json_arrows_distinguishes_near_and_free_ends() {
    let v = inspect_json(&[
        "inspect",
        "tests/fixtures/arrows.excalidraw",
        "--format",
        "json",
    ]);
    assert_eq!(
        v["connectors"][3]["end"],
        serde_json::json!({"attach": "near", "target": "d", "distance": 8})
    );
    assert_eq!(
        v["connectors"][2]["end"],
        serde_json::json!({"attach": "free"})
    );
    assert_json_integers(&v);
}

#[test]
fn inspect_json_text_layout_issues_match_lint_json() {
    let inspect = inspect_json(&[
        "inspect",
        "tests/fixtures/text_layout.excalidraw",
        "--format",
        "json",
    ]);
    let (code, stdout, stderr) = run(&[
        "lint",
        "tests/fixtures/text_layout.excalidraw",
        "--format",
        "json",
    ]);
    assert_eq!(code, 0, "{stderr}");
    let lint: serde_json::Value = serde_json::from_str(&stdout).expect("lint json");
    assert_eq!(inspect["issues"], lint["issues"]);
    assert_json_integers(&inspect);
}

#[test]
fn inspect_json_empty_scene_has_null_canvas() {
    let path = std::env::temp_dir().join("excal-inspect-json-empty.excalidraw");
    std::fs::write(&path, r#"{"type":"excalidraw","version":2,"elements":[]}"#)
        .expect("write empty fixture");
    let v = inspect_json(&["inspect", path.to_str().expect("path"), "--format", "json"]);
    assert!(
        v["canvas"].is_null(),
        "empty scene canvas is null, got {}",
        v["canvas"]
    );
    assert_eq!(v["components"], serde_json::json!([]));
    assert_eq!(v["issues"], serde_json::json!([]));
}

#[test]
fn inspect_json_text_component_neighbors_are_null() {
    let path = std::env::temp_dir().join("excal-inspect-json-text.excalidraw");
    std::fs::write(
        &path,
        r#"{"type":"excalidraw","version":2,"elements":[{"id":"note","type":"text","x":1.6,"y":2.4,"width":10.2,"height":8.6,"text":"hi","isDeleted":false}]}"#,
    )
    .expect("write text fixture");
    let v = inspect_json(&["inspect", path.to_str().expect("path"), "--format", "json"]);
    let note = &v["components"][0];
    assert_eq!(note["kind"], "text");
    assert!(
        note["neighbors"].is_null(),
        "text neighbors are null, got {}",
        note["neighbors"]
    );
    assert_eq!(
        note["rect"],
        serde_json::json!({"x1": 2, "y1": 2, "x2": 26, "y2": 27})
    );
    assert_eq!(note["center"], serde_json::json!({"x": 14, "y": 15}));
    assert!(note["rect"]["x1"].is_i64());
}

#[test]
fn inspect_json_swimlanes_padding_keeps_negative_right() {
    let v = inspect_json(&[
        "inspect",
        "tests/fixtures/swimlanes.excalidraw",
        "--format",
        "json",
    ]);
    assert_eq!(
        v["containers"][1]["padding"],
        serde_json::json!({"top": 110, "right": -20, "bottom": 110, "left": 40})
    );
    assert_json_integers(&v);
}

#[test]
fn format_inspect_omits_neighbor_suffix_for_text_components() {
    let report = format_inspect(
        "note.excalidraw",
        &scene(ComponentKind::Text, "note", (0.0, 0.0, 40.0, 20.0)),
        &[],
    );
    let component = report
        .lines()
        .find(|line| line.starts_with("- note "))
        .expect("text component line");
    assert!(component.contains("parent=-"));
    assert!(
        !component.contains(" L="),
        "text components stop after parent, got {component}"
    );
}

#[test]
fn format_inspect_includes_neighbor_suffix_for_shapes_and_groups() {
    let mut group = scene(ComponentKind::Group, "icon", (0.0, 0.0, 40.0, 40.0));
    group.components.push(component(
        ComponentKind::Shape,
        "box",
        (80.0, 0.0, 40.0, 40.0),
    ));
    group.components[1].z = 1;
    let report = format_inspect("g.excalidraw", &group, &[]);
    let icon = report
        .lines()
        .find(|line| line.starts_with("- icon "))
        .expect("group line");
    assert!(
        icon.contains("parent=- L=- R=box(40) T=- B=-"),
        "group is a node and keeps neighbors, got {icon}"
    );
}

#[test]
fn format_inspect_orders_tracks_top_then_container_z_then_pos() {
    let mut built = scene(ComponentKind::Shape, "late-a", (0.0, 200.0, 40.0, 40.0));
    built.components[0].parent = Some("zone-b".to_string());
    built.components[0].z = 3;
    built.components.push(component(
        ComponentKind::Shape,
        "late-b",
        (80.0, 200.0, 40.0, 40.0),
    ));
    built.components[1].parent = Some("zone-b".to_string());
    built.components[1].z = 4;
    built.components.push(component(
        ComponentKind::Shape,
        "early-a",
        (0.0, 0.0, 40.0, 40.0),
    ));
    built.components[2].parent = Some("zone-a".to_string());
    built.components[2].z = 1;
    built.components.push(component(
        ComponentKind::Shape,
        "early-b",
        (80.0, 0.0, 40.0, 40.0),
    ));
    built.components[3].parent = Some("zone-a".to_string());
    built.components[3].z = 2;
    built.components.push(component(
        ComponentKind::Shape,
        "top-a",
        (0.0, 400.0, 40.0, 40.0),
    ));
    built.components[4].z = 5;
    built.components.push(component(
        ComponentKind::Shape,
        "top-b",
        (0.0, 480.0, 40.0, 40.0),
    ));
    built.components[5].z = 6;
    built.containers.push(Container {
        id: "zone-b".to_string(),
        kind: ContainerKind::Zone,
        rect: Rect::from_xywh(0.0, 180.0, 200.0, 80.0),
        name: None,
        title_element: None,
        title_height: None,
        elements: vec!["zone-b".to_string()],
        parent: None,
        frame_id: None,
        z: 10,
    });
    built.containers.push(Container {
        id: "zone-a".to_string(),
        kind: ContainerKind::Zone,
        rect: Rect::from_xywh(0.0, 0.0, 200.0, 80.0),
        name: None,
        title_element: None,
        title_height: None,
        elements: vec!["zone-a".to_string()],
        parent: None,
        frame_id: None,
        z: 0,
    });
    let report = format_inspect("tracks.excalidraw", &built, &[]);
    let rows: Vec<_> = report
        .lines()
        .skip_while(|line| *line != "## Rows")
        .skip(1)
        .take_while(|line| !line.is_empty())
        .collect();
    let columns: Vec<_> = report
        .lines()
        .skip_while(|line| *line != "## Columns")
        .skip(1)
        .take_while(|line| !line.is_empty())
        .collect();
    assert_eq!(
        rows,
        [
            "- zone-a y≈20: early-a --40-- early-b",
            "- zone-b y≈220: late-a --40-- late-b",
        ],
        "container rows follow container z, not insertion order"
    );
    assert_eq!(columns, ["- top x≈20: top-a --40-- top-b"]);
}

#[test]
fn render_minimap_is_none_when_there_are_no_components() {
    let empty = Scene {
        components: Vec::new(),
        containers: vec![Container {
            id: "frame".to_string(),
            kind: ContainerKind::Frame,
            rect: Rect::from_xywh(0.0, 0.0, 100.0, 50.0),
            name: Some("Frame".to_string()),
            title_element: None,
            title_height: None,
            elements: vec!["frame".to_string()],
            parent: None,
            frame_id: None,
            z: 0,
        }],
        connectors: vec![Connector {
            id: "line".to_string(),
            kind: "line".to_string(),
            elbowed: false,
            points: vec![Point { x: 0.0, y: 0.0 }, Point { x: 100.0, y: 50.0 }],
            start: Attach::Free,
            end: Attach::Free,
            label: None,
            label_element: None,
            label_rect: None,
            z: 1,
        }],
        notes: Vec::new(),
    };
    assert!(render(&empty).is_empty());
    let report = format_inspect("frame-only.excalidraw", &empty, &[]);
    let minimap: Vec<_> = report
        .lines()
        .skip_while(|line| *line != "## Minimap")
        .skip(1)
        .take_while(|line| !line.is_empty())
        .collect();
    assert_eq!(minimap, ["(none)"]);
}

#[test]
fn render_minimap_legend_uses_hash_after_sixty_two_components() {
    let mut built = Scene {
        components: Vec::new(),
        containers: Vec::new(),
        connectors: Vec::new(),
        notes: Vec::new(),
    };
    for i in 0..63 {
        let mut item = component(
            ComponentKind::Shape,
            &format!("n{i}"),
            (i as f64, 0.0, 1.0, 1.0),
        );
        item.z = i;
        built.components.push(item);
    }
    let lines = render(&built);
    let legend = lines.last().expect("legend");
    assert!(legend.starts_with("A=n0 "));
    assert!(legend.contains(" Z=n25 "));
    assert!(legend.contains(" a=n26 "));
    assert!(legend.contains(" 9=n61 "));
    assert!(legend.ends_with(" #=n62"), "got {legend}");
}

#[test]
fn format_inspect_empty_scene_matches_spec_sections() {
    let empty = Scene {
        components: Vec::new(),
        containers: Vec::new(),
        connectors: Vec::new(),
        notes: Vec::new(),
    };
    let report = format_inspect("<stdin>", &empty, &[]);
    assert_eq!(
        report,
        "\
# excal inspect <stdin>
canvas (empty) | components: 0, containers: 0, connectors: 0 | errors: 0, warnings: 0, info: 0

## Containers
(none)

## Components
(none)

## Rows
(none)

## Columns
(none)

## Connections
(none)

## Minimap
(none)

## Issues
(none)
"
    );
}

/// Fixture F21: nested sub-zone `z_shards` sits between the left and right
/// serving pipelines inside `z_serving`.
fn fixture_f21() -> String {
    r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "z_serving", "x": 0, "y": 0, "width": 1560, "height": 520},
    {"type": "text", "id": "z_serving_title", "x": 16, "y": 12, "width": 100, "height": 25, "text": "Serving", "fontSize": 20, "textAlign": "left"},
    {"type": "rectangle", "id": "search_ui", "x": 40, "y": 220, "width": 160, "height": 80},
    {"type": "rectangle", "id": "q_rewrite", "x": 260, "y": 220, "width": 160, "height": 80},
    {"type": "rectangle", "id": "blender", "x": 510, "y": 220, "width": 160, "height": 80},
    {"type": "rectangle", "id": "z_shards", "x": 730, "y": 60, "width": 300, "height": 420},
    {"type": "text", "id": "z_shards_title", "x": 746, "y": 72, "width": 100, "height": 25, "text": "Shards", "fontSize": 20, "textAlign": "left"},
    {"type": "rectangle", "id": "shard_1", "x": 800, "y": 115, "width": 160, "height": 75},
    {"type": "rectangle", "id": "shard_2", "x": 800, "y": 222.5, "width": 160, "height": 75},
    {"type": "rectangle", "id": "shard_3", "x": 800, "y": 330, "width": 160, "height": 75},
    {"type": "rectangle", "id": "l1_ranker", "x": 1090, "y": 220, "width": 160, "height": 80},
    {"type": "rectangle", "id": "l2_reranker", "x": 1310, "y": 220, "width": 160, "height": 80}
  ]
}"#
    .to_string()
}

#[test]
fn b21_fixture_f21_splits_serving_row_around_nested_zone() {
    let doc = Document::parse(&fixture_f21()).expect("parse F21");
    let scene = Scene::build(&doc);
    let shards = scene
        .containers
        .iter()
        .find(|c| c.id == "z_shards")
        .expect("z_shards");
    assert_eq!(shards.parent.as_deref(), Some("z_serving"));

    let report = format_inspect("f21.excalidraw", &scene, &[]);
    let rows: Vec<_> = report
        .lines()
        .skip_while(|line| *line != "## Rows")
        .skip(1)
        .take_while(|line| !line.is_empty() && *line != "(none)")
        .filter(|line| line.contains("z_serving"))
        .collect();
    assert!(
        rows.contains(&"- z_serving y≈260: search_ui --60-- q_rewrite --90-- blender"),
        "missing left serving row: {rows:?}"
    );
    assert!(
        rows.contains(&"- z_serving y≈260: l1_ranker --60-- l2_reranker"),
        "missing right serving row: {rows:?}"
    );
    assert!(
        !rows
            .iter()
            .any(|line| line.contains("blender") && line.contains("l1_ranker")),
        "blender must not share a row with l1_ranker: {rows:?}"
    );

    let dir = std::env::temp_dir().join(format!(
        "excal-t41-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("f21.excalidraw");
    std::fs::write(&path, fixture_f21()).expect("write F21");
    let (code, stdout, stderr) = run(&["inspect", path.to_str().expect("path")]);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(
        stdout.contains("- z_serving y≈260: search_ui --60-- q_rewrite --90-- blender"),
        "cli inspect missing left row:\n{stdout}"
    );
    assert!(
        stdout.contains("- z_serving y≈260: l1_ranker --60-- l2_reranker"),
        "cli inspect missing right row:\n{stdout}"
    );
}

#[test]
fn b24_multiline_labels_stay_on_one_inspect_bullet_and_json_keeps_newlines() {
    let dir = std::env::temp_dir().join(format!(
        "excal-t43-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("b24.excalidraw");
    let path_s = path.to_str().expect("utf8 path").to_string();

    let (code, _stdout, stderr) = run(&[
        "draw",
        "node",
        &path_s,
        "--id",
        "cdn_waf",
        "--label",
        "Cloudflare WAF\\n& CDN",
        "--x",
        "100",
        "--y",
        "100",
        "--in-place",
    ]);
    assert_eq!(code, 0, "draw cdn_waf stderr={stderr}");

    let (code, _stdout, stderr) = run(&[
        "draw",
        "node",
        &path_s,
        "--id",
        "api_gw",
        "--label",
        "GraphQL\\nAPI Gateway",
        "--right-of",
        "cdn_waf",
        "--gap",
        "120",
        "--in-place",
    ]);
    assert_eq!(code, 0, "draw api_gw stderr={stderr}");

    let (code, _stdout, stderr) = run(&[
        "draw",
        "zone",
        &path_s,
        "--id",
        "z1",
        "--title",
        "1. Client &\\nEdge Tier",
        "--children",
        "cdn_waf,api_gw",
        "--in-place",
    ]);
    assert_eq!(code, 0, "draw zone stderr={stderr}");

    let (code, _stdout, stderr) = run(&[
        "draw",
        "connect",
        &path_s,
        "--from",
        "cdn_waf",
        "--to",
        "api_gw",
        "--id",
        "a1",
        "--label",
        "TLS 1.3\\nmTLS",
        "--in-place",
    ]);
    assert_eq!(code, 0, "draw connect stderr={stderr}");

    let (code, stdout, stderr) = run(&["inspect", &path_s]);
    assert_eq!(code, 0, "inspect stderr={stderr}");

    let bullets: Vec<_> = stdout
        .lines()
        .filter(|line| line.starts_with("- "))
        .collect();
    for line in &bullets {
        assert!(
            !line.contains('\n'),
            "inspect bullet must be a single line: {line:?}"
        );
    }
    let expected = [
        ("cdn_waf", "Cloudflare WAF & CDN"),
        ("api_gw", "GraphQL API Gateway"),
        ("z1", "1. Client & Edge Tier"),
        ("a1", "TLS 1.3 mTLS"),
    ];
    for (id, label) in expected {
        let line = bullets
            .iter()
            .find(|line| line.starts_with(&format!("- {id} ")))
            .unwrap_or_else(|| panic!("missing inspect bullet for {id}:\\n{stdout}"));
        assert!(
            line.contains(&format!("\"{label}\"")),
            "bullet for {id} should quote {label:?}, got {line}"
        );
        assert!(
            !line.contains('\\'),
            "collapsed label must not keep a backslash: {line}"
        );
    }

    let (code, json_out, stderr) = run(&["inspect", &path_s, "--format", "json"]);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(code, 0, "inspect json stderr={stderr}");
    let v: serde_json::Value =
        serde_json::from_str(&json_out).unwrap_or_else(|err| panic!("json: {err}\\n{json_out}"));
    let component_label = |id: &str| {
        v["components"]
            .as_array()
            .expect("components")
            .iter()
            .find(|c| c["id"] == id)
            .unwrap_or_else(|| panic!("missing component {id}"))["label"]
            .as_str()
            .unwrap_or_else(|| panic!("{id} label is not a string"))
            .to_string()
    };
    assert_eq!(component_label("cdn_waf"), "Cloudflare WAF\n& CDN");
    assert_eq!(component_label("api_gw"), "GraphQL\nAPI Gateway");
    let zone_name = v["containers"]
        .as_array()
        .expect("containers")
        .iter()
        .find(|c| c["id"] == "z1")
        .expect("zone z1")["name"]
        .as_str()
        .expect("zone name");
    assert_eq!(zone_name, "1. Client &\nEdge Tier");
    let arrow_label = v["connectors"]
        .as_array()
        .expect("connectors")
        .iter()
        .find(|c| c["id"] == "a1")
        .expect("connector a1")["label"]
        .as_str()
        .expect("arrow label");
    assert_eq!(arrow_label, "TLS 1.3\nmTLS");
}
