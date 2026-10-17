use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use excalidraw_spatial::linter::run_lint;
use excalidraw_spatial::scene_graph::{ContainerKind, Scene};
use excalidraw_spatial::schema::Document;
use excalidraw_spatial::text_metrics::TextSize;

const FIXED_NOW_MS: &str = "1700000000000";

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_excal")
}

fn fixture(name: &str) -> String {
    format!("tests/fixtures/{name}.excalidraw")
}

fn run_layout(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(bin())
        .args(args)
        .env("EXCAL_FIXED_NOW_MS", FIXED_NOW_MS)
        .output()
        .expect("failed to execute excal");
    let code = output.status.code().expect("process terminated by signal");
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr utf-8");
    (code, stdout, stderr)
}

fn parse_json(s: &str) -> serde_json::Value {
    serde_json::from_str(s).unwrap_or_else(|e| panic!("parse json: {e}; src={s}"))
}

fn element<'a>(doc: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    doc["elements"]
        .as_array()
        .expect("elements array")
        .iter()
        .find(|el| el["id"] == id)
        .unwrap_or_else(|| panic!("missing element {id}"))
}

fn num(el: &serde_json::Value, key: &str) -> f64 {
    el[key]
        .as_f64()
        .unwrap_or_else(|| panic!("{key} is not a number: {}", el[key]))
}

fn approx(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b} (tol 1e-6)");
}

fn temp_path(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("excal-t20-{}-{}-{tag}", std::process::id(), nanos))
}

fn cleanup(path: &Path) {
    let _ = fs::remove_file(path);
}

fn lint_json(doc_json: &str) -> serde_json::Value {
    let output = run_lint_json(doc_json);
    assert_eq!(
        output.status.code().expect("lint code"),
        0,
        "lint stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    parse_json(&String::from_utf8(output.stdout).expect("lint stdout utf-8"))
}

fn lint_issues(doc_json: &str) -> Vec<serde_json::Value> {
    let output = run_lint_json(doc_json);
    parse_json(&String::from_utf8(output.stdout).expect("lint stdout utf-8"))["issues"]
        .as_array()
        .expect("issues")
        .clone()
}

fn run_lint_json(doc_json: &str) -> std::process::Output {
    let mut child = Command::new(bin())
        .args(["lint", "-", "--format", "json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn lint");
    child
        .stdin
        .take()
        .expect("lint stdin")
        .write_all(doc_json.as_bytes())
        .expect("write lint stdin");
    child.wait_with_output().expect("lint")
}

#[test]
fn align_text_layout_middle_moves_n2_and_clears_that_near_miss() {
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "align",
        &fixture("text_layout"),
        "--ids",
        "n1,n2",
        "--axis",
        "middle",
    ]);
    assert_eq!(code, 0, "stderr={stderr}");
    let out = parse_json(&stdout);

    approx(num(element(&out, "n2"), "y"), 100.0);
    approx(num(element(&out, "n2-label"), "y"), 127.5);

    let issues = lint_json(&stdout)["issues"]
        .as_array()
        .expect("issues")
        .clone();
    assert!(
        issues.iter().all(|issue| {
            !(issue["rule"] == "NEAR_MISS_ALIGNMENT"
                && issue["targets"] == serde_json::json!(["n1", "n2"]))
        }),
        "still has NEAR_MISS_ALIGNMENT for [n1, n2]: {issues:?}"
    );
}

#[test]
fn place_svc_below_api_resyncs_arrow_and_lints_clean() {
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "place",
        &fixture("basic_flow"),
        "--id",
        "svc",
        "--below",
        "api",
        "--gap",
        "60",
    ]);
    assert_eq!(code, 0, "stderr={stderr}");
    let out = parse_json(&stdout);

    approx(num(element(&out, "svc"), "x"), 360.0);
    approx(num(element(&out, "svc"), "y"), 240.0);

    let a2 = element(&out, "a2");
    approx(num(a2, "x"), 467.43);
    approx(num(a2, "y"), 186.0);
    assert_eq!(a2["points"], serde_json::json!([[0, 0], [-54.86, 48]]));

    approx(num(element(&out, "a2-label"), "x"), 423.36);
    approx(num(element(&out, "a2-label"), "y"), 200.0);

    let lint = lint_json(&stdout);
    assert_eq!(
        lint["issues"].as_array().expect("issues").len(),
        0,
        "lint={lint}"
    );
}

#[test]
fn distribute_text_layout_horizontal_gap_spaces_n3_and_clears_uneven_spacing() {
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "distribute",
        &fixture("text_layout"),
        "--ids",
        "n1,n2,n3",
        "--direction",
        "horizontal",
        "--gap",
        "60",
    ]);
    assert_eq!(code, 0, "stderr={stderr}");
    let out = parse_json(&stdout);

    approx(num(element(&out, "n2"), "x"), 220.0);
    approx(num(element(&out, "n3"), "x"), 440.0);
    approx(num(element(&out, "n3-label"), "x"), 386.0);

    let issues = lint_json(&stdout)["issues"]
        .as_array()
        .expect("issues")
        .clone();
    assert!(
        issues.iter().all(|issue| issue["rule"] != "UNEVEN_SPACING"),
        "still has UNEVEN_SPACING: {issues:?}"
    );
}

#[test]
fn distribute_text_layout_horizontal_without_gap_keeps_ends_fixed() {
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "distribute",
        &fixture("text_layout"),
        "--ids",
        "n1,n2,n3",
        "--direction",
        "horizontal",
    ]);
    assert_eq!(code, 0, "stderr={stderr}");
    let out = parse_json(&stdout);

    approx(num(element(&out, "n2"), "x"), 200.0);
    approx(num(element(&out, "n2-label"), "x"), 212.4);
    approx(num(element(&out, "n3"), "x"), 400.0);
}

#[test]
fn distribute_arrows_vertical_gap_moves_d_and_shrinks_near_elbow() {
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "distribute",
        &fixture("arrows"),
        "--ids",
        "c,d",
        "--direction",
        "vertical",
        "--gap",
        "60",
    ]);
    assert_eq!(code, 0, "stderr={stderr}");
    let out = parse_json(&stdout);

    approx(num(element(&out, "d"), "y"), 240.0);
    approx(num(element(&out, "d-label"), "y"), 267.5);
    assert_eq!(
        element(&out, "arr_near")["points"],
        serde_json::json!([[0, 0], [0, 94], [142, 94]])
    );
}

#[test]
fn distribute_two_ids_without_gap_exits_2() {
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "distribute",
        &fixture("overlap"),
        "--ids",
        "api,auth",
        "--direction",
        "horizontal",
    ]);
    assert_eq!(code, 2, "stdout={stdout} stderr={stderr}");
    assert_eq!(
        stderr, "excal: error: distribute without --gap needs at least 3 ids\n",
        "stdout={stdout}"
    );
}

#[test]
fn place_auth_right_of_api_uses_default_gap_and_center() {
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "place",
        &fixture("overlap"),
        "--id",
        "auth",
        "--right-of",
        "api",
    ]);
    assert_eq!(code, 0, "stderr={stderr}");
    let out = parse_json(&stdout);

    approx(num(element(&out, "auth"), "x"), 300.0);
    approx(num(element(&out, "auth"), "y"), 100.0);

    let lint = lint_json(&stdout);
    assert_eq!(
        lint["issues"].as_array().expect("issues").len(),
        0,
        "lint={lint}"
    );
}

#[test]
fn fit_container_swimlanes_lane_be_grows_around_children_and_title() {
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "fit-container",
        &fixture("swimlanes"),
        "--id",
        "lane_be",
    ]);
    assert_eq!(code, 0, "stderr={stderr}");
    let out = parse_json(&stdout);

    let lane_be = element(&out, "lane_be");
    approx(num(lane_be, "x"), 440.0);
    approx(num(lane_be, "y"), 35.0);
    approx(num(lane_be, "width"), 460.0);
    approx(num(lane_be, "height"), 195.0);

    approx(num(element(&out, "lane_be_title"), "x"), 456.0);
    approx(num(element(&out, "lane_be_title"), "y"), 47.0);

    let lint = lint_json(&stdout);
    assert_eq!(
        lint["issues"].as_array().expect("issues").len(),
        0,
        "lint={lint}"
    );
}

#[test]
fn fit_container_swimlanes_lane_be_expand_only_keeps_aligned_edges() {
    // T18 without --expand-only still shrinks y/height (see test above). This flag
    // unions the tight rect with the current rect so lane_be stays aligned with lane_fe.
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "fit-container",
        &fixture("swimlanes"),
        "--id",
        "lane_be",
        "--padding",
        "40",
        "--expand-only",
    ]);
    assert_eq!(code, 0, "stderr={stderr}");
    let out = parse_json(&stdout);

    let lane_be = element(&out, "lane_be");
    approx(num(lane_be, "x"), 440.0);
    approx(num(lane_be, "y"), 0.0);
    approx(num(lane_be, "width"), 460.0);
    approx(num(lane_be, "height"), 300.0);
}

#[test]
fn fit_container_icon_group_storage_uses_padding_without_title_inset() {
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "fit-container",
        &fixture("icon_group"),
        "--id",
        "Storage",
        "--padding",
        "20",
    ]);
    assert_eq!(code, 0, "stderr={stderr}");
    let out = parse_json(&stdout);

    let frame = element(&out, "Fr8Mw2Nq5Xs1Lp4Kv7RbZ");
    approx(num(frame, "x"), 38.4);
    approx(num(frame, "y"), 60.0);
    approx(num(frame, "width"), 441.6);
    approx(num(frame, "height"), 200.0);

    let lint = lint_json(&stdout);
    assert_eq!(
        lint["issues"].as_array().expect("issues").len(),
        0,
        "lint={lint}"
    );
}

#[test]
fn fit_text_text_layout_n3_grows_width_and_keeps_center() {
    let (code, stdout, stderr) =
        run_layout(&["layout", "fit-text", &fixture("text_layout"), "--id", "n3"]);
    assert_eq!(code, 0, "stderr={stderr}");
    let out = parse_json(&stdout);

    let n3 = element(&out, "n3");
    approx(num(n3, "x"), 322.0);
    approx(num(n3, "y"), 100.0);
    approx(num(n3, "width"), 256.0);
    approx(num(n3, "height"), 80.0);
    approx(num(element(&out, "n3-label"), "x"), 346.0);

    let issues = lint_issues(&stdout);
    assert!(
        issues.iter().all(|issue| issue["rule"] != "TEXT_OVERFLOW"),
        "still has TEXT_OVERFLOW: {issues:?}"
    );
}

#[test]
fn fit_text_text_layout_all_only_changes_overflowing_n3() {
    let original = std::fs::read_to_string(fixture("text_layout")).expect("read fixture");
    let (code, stdout, stderr) =
        run_layout(&["layout", "fit-text", &fixture("text_layout"), "--all"]);
    assert_eq!(code, 0, "stderr={stderr}");
    let input = parse_json(&original);
    let out = parse_json(&stdout);
    let input_els = input["elements"].as_array().expect("input elements");
    let out_els = out["elements"].as_array().expect("output elements");
    assert_eq!(out_els.len(), input_els.len());
    for (before, after) in input_els.iter().zip(out_els) {
        let id = before["id"].as_str().expect("id");
        if id == "n3" || id == "n3-label" {
            continue;
        }
        assert_eq!(after, before, "element {id} changed");
    }
}

#[test]
fn fit_text_text_layout_n1_already_fits() {
    let original = std::fs::read_to_string(fixture("text_layout")).expect("read fixture");
    let (code, stdout, stderr) =
        run_layout(&["layout", "fit-text", &fixture("text_layout"), "--id", "n1"]);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert_eq!(stdout, original);
    assert_eq!(stderr, "excal: nothing to change\n");
}

#[test]
fn fit_text_icon_group_postgres_group_exits_2() {
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "fit-text",
        &fixture("icon_group"),
        "--id",
        "Postgres",
    ]);
    assert_eq!(code, 2, "stdout={stdout} stderr={stderr}");
}

#[test]
fn fit_container_basic_flow_api_is_not_a_container() {
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "fit-container",
        &fixture("basic_flow"),
        "--id",
        "api",
    ]);
    assert_eq!(code, 2, "stdout={stdout} stderr={stderr}");
}

#[test]
fn fit_text_writes_estimated_bound_label_size() {
    let path = temp_path("fit-text.excalidraw");
    let src = r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "box", "x": 100, "y": 100, "width": 120, "height": 80},
    {
      "type": "text",
      "id": "lbl",
      "x": 140,
      "y": 127.5,
      "width": 40,
      "height": 25,
      "text": "Order Fulfillment Service",
      "fontSize": 20,
      "textAlign": "center",
      "containerId": "box"
    }
  ]
}"#;
    fs::write(&path, src).expect("write temp fit-text file");
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run_layout(&["layout", "fit-text", path_str, "--id", "box"]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    let out = parse_json(&stdout);
    let box_el = element(&out, "box");
    approx(num(box_el, "x"), -14.0);
    approx(num(box_el, "width"), 348.0);
    approx(num(box_el, "height"), 80.0);
    let lbl = element(&out, "lbl");
    approx(num(lbl, "width"), 300.0);
    approx(num(lbl, "height"), 25.0);
    approx(num(lbl, "x"), 10.0);
    approx(num(lbl, "y"), 127.5);
}

#[test]
fn move_arrow_endpoint_writes_estimated_label_size() {
    let path = temp_path("place-label.excalidraw");
    let src = r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "a", "x": 0, "y": 100, "width": 140, "height": 80},
    {"type": "rectangle", "id": "b", "x": 200, "y": 100, "width": 140, "height": 80},
    {
      "type": "arrow",
      "id": "arr",
      "x": 146,
      "y": 140,
      "width": 48,
      "height": 0,
      "points": [[0, 0], [48, 0]],
      "startBinding": {"elementId": "a"},
      "endBinding": {"elementId": "b"}
    },
    {
      "type": "text",
      "id": "arr-lbl",
      "x": 170,
      "y": 130,
      "width": 0,
      "height": 0,
      "text": "validateToken()",
      "fontSize": 16,
      "textAlign": "center",
      "containerId": "arr"
    }
  ]
}"#;
    fs::write(&path, src).expect("write temp place-label file");
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) =
        run_layout(&["layout", "move", path_str, "--id", "b", "--dx", "10"]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    let out = parse_json(&stdout);
    let lbl = element(&out, "arr-lbl");
    approx(num(lbl, "width"), 144.0);
    approx(num(lbl, "height"), 20.0);
}

#[test]
fn fit_text_all_reclips_orbit_arrow_after_resize() {
    let path = temp_path("b01-fit-text.excalidraw");
    let src = r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {
      "type": "rectangle",
      "id": "s1",
      "x": 100,
      "y": 100,
      "width": 100,
      "height": 80,
      "boundElements": [
        {"type": "text", "id": "s1-label"},
        {"type": "arrow", "id": "arr"}
      ]
    },
    {
      "type": "text",
      "id": "s1-label",
      "x": 60,
      "y": 127.5,
      "width": 180,
      "height": 25,
      "text": "Payment Gateway",
      "fontSize": 20,
      "textAlign": "center",
      "containerId": "s1"
    },
    {
      "type": "rectangle",
      "id": "s2",
      "x": 400,
      "y": 100,
      "width": 100,
      "height": 80,
      "boundElements": [
        {"type": "text", "id": "s2-label"},
        {"type": "arrow", "id": "arr"}
      ]
    },
    {
      "type": "text",
      "id": "s2-label",
      "x": 360,
      "y": 127.5,
      "width": 180,
      "height": 25,
      "text": "Payment Gateway",
      "fontSize": 20,
      "textAlign": "center",
      "containerId": "s2"
    },
    {
      "type": "arrow",
      "id": "arr",
      "x": 206,
      "y": 140,
      "width": 188,
      "height": 0,
      "points": [[0, 0], [188, 0]],
      "startBinding": {"elementId": "s1", "mode": "orbit"},
      "endBinding": {"elementId": "s2", "mode": "orbit"}
    }
  ]
}"#;
    fs::write(&path, src).expect("write temp b01 file");
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run_layout(&["layout", "fit-text", path_str, "--all"]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    let out = parse_json(&stdout);

    let s1 = element(&out, "s1");
    approx(num(s1, "x"), 36.0);
    approx(num(s1, "width"), 228.0);

    let s2 = element(&out, "s2");
    approx(num(s2, "x"), 336.0);
    approx(num(s2, "width"), 228.0);

    let arr = element(&out, "arr");
    approx(num(arr, "x"), 270.0);
    approx(num(arr, "y"), 140.0);
    approx(num(arr, "width"), 60.0);
    assert_eq!(arr["points"], serde_json::json!([[0, 0], [60, 0]]));
}

#[test]
fn place_b_below_a_routes_elbow_vertically_to_avoid_self_crossing() {
    let path = temp_path("b09a-place.excalidraw");
    let src = r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {
      "type": "rectangle",
      "id": "A",
      "x": 100,
      "y": 100,
      "width": 140,
      "height": 80,
      "boundElements": [{"type": "arrow", "id": "arr"}]
    },
    {
      "type": "rectangle",
      "id": "B",
      "x": 340,
      "y": 100,
      "width": 140,
      "height": 80,
      "boundElements": [{"type": "arrow", "id": "arr"}]
    },
    {
      "type": "arrow",
      "id": "arr",
      "x": 246,
      "y": 140,
      "width": 88,
      "height": 0,
      "points": [[0, 0], [88, 0]],
      "elbowed": true,
      "startBinding": {"elementId": "A", "fixedPoint": [1, 0.5]},
      "endBinding": {"elementId": "B", "fixedPoint": [0, 0.5]}
    }
  ]
}"#;
    fs::write(&path, src).expect("write temp b09a file");
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) =
        run_layout(&["layout", "place", path_str, "--id", "B", "--below", "A"]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    let out = parse_json(&stdout);

    let arr = element(&out, "arr");
    approx(num(arr, "x"), 246.0);
    approx(num(arr, "y"), 140.0);
    assert_eq!(
        arr["points"],
        serde_json::json!([[0, 0], [0, 60], [-152, 60], [-152, 120]])
    );

    let issues = lint_issues(&stdout);
    assert!(
        issues
            .iter()
            .all(|issue| issue["rule"] != "ARROW_CROSSING_NODE"),
        "ARROW_CROSSING_NODE: {issues:?}"
    );
}

#[test]
fn move_fetcher_toward_parser_clips_forward_at_4px_gap() {
    let path = temp_path("b12a-move.excalidraw");
    let src = r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {
      "type": "rectangle",
      "id": "fetcher",
      "x": 275,
      "y": 65,
      "width": 175,
      "height": 75,
      "boundElements": [{"type": "arrow", "id": "a_fetch_parse"}]
    },
    {
      "type": "rectangle",
      "id": "parser",
      "x": 510,
      "y": 65,
      "width": 175,
      "height": 75,
      "boundElements": [{"type": "arrow", "id": "a_fetch_parse"}]
    },
    {
      "type": "arrow",
      "id": "a_fetch_parse",
      "x": 456,
      "y": 102.5,
      "width": 48,
      "height": 0,
      "points": [[0, 0], [48, 0]],
      "startBinding": {"elementId": "fetcher", "mode": "orbit"},
      "endBinding": {"elementId": "parser", "mode": "orbit"}
    }
  ]
}"#;
    fs::write(&path, src).expect("write temp b12a file");
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) =
        run_layout(&["layout", "move", path_str, "--id", "fetcher", "--dx", "56"]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    let out = parse_json(&stdout);

    let arr = element(&out, "a_fetch_parse");
    approx(num(arr, "x"), 506.0);
    approx(num(arr, "y"), 102.5);
    approx(num(arr, "width"), 4.0);
    assert_eq!(arr["points"], serde_json::json!([[0, 0], [4, 0]]));
    let points = arr["points"].as_array().expect("points");
    let end_x = points[1][0].as_f64().expect("end x");
    assert!(end_x > 0.0, "arrow flipped backwards: {}", arr["points"]);
}

#[test]
fn fit_text_absorbs_left_anchored_overflow_and_clears_warnings() {
    // Box 100..220 x 100..180. Unbound "Notification Service" starts in the
    // left half (x=115) and runs past the right edge (x2=355), so its center
    // (235, 140.5) is outside the box.
    let src = r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "box", "x": 100, "y": 100, "width": 120, "height": 80},
    {
      "type": "text",
      "id": "txt",
      "x": 115,
      "y": 128,
      "width": 240,
      "height": 25,
      "text": "Notification Service",
      "fontSize": 20
    }
  ]
}"#;
    let doc = Document::parse(src).expect("parse");
    let scene = Scene::build(&doc);
    assert_eq!(scene.components.len(), 1);
    let box_shape = scene.component("box").expect("box");
    assert_eq!(box_shape.label.as_deref(), Some("Notification Service"));
    assert_eq!(
        box_shape.label_size,
        Some(TextSize {
            width: 390.0,
            height: 26.0,
            estimated: false,
        })
    );

    let issues = run_lint(&scene, "b07.excalidraw");
    let rules: Vec<_> = issues.iter().map(|issue| issue.rule).collect();
    assert!(
        rules.contains(&"TEXT_OVERFLOW"),
        "missing TEXT_OVERFLOW: {issues:?}"
    );
    assert!(
        rules.contains(&"UNBOUND_TEXT"),
        "missing UNBOUND_TEXT: {issues:?}"
    );
    assert!(
        !rules.contains(&"OVERLAP_COMPONENT"),
        "unbound label was treated as a separate component: {issues:?}"
    );
    let overflow = issues
        .iter()
        .find(|issue| issue.rule == "TEXT_OVERFLOW")
        .expect("TEXT_OVERFLOW");
    assert_eq!(
        overflow.severity,
        excalidraw_spatial::linter::Severity::Warn
    );
    assert!(
        overflow.message.contains("needs 390x26px"),
        "overflow message: {}",
        overflow.message
    );

    let path = temp_path("b07-fit-text.excalidraw");
    fs::write(&path, src).expect("write temp b07 file");
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run_layout(&["layout", "fit-text", path_str, "--id", "box"]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    let out = parse_json(&stdout);
    let box_el = element(&out, "box");
    approx(num(box_el, "x"), -59.0);
    approx(num(box_el, "width"), 438.0);
    approx(num(box_el, "height"), 80.0);
    assert!(
        num(box_el, "x") <= 115.0 && num(box_el, "x") + num(box_el, "width") >= 355.0,
        "resized box does not enclose txt x=115..355: {box_el}"
    );

    let after = lint_json(&stdout);
    assert_eq!(after["summary"]["errors"], 0, "lint={after}");
    assert_eq!(after["summary"]["warnings"], 0, "lint={after}");
}

fn scatter_gather_pipeline(shard_2_x: f64) -> String {
    format!(
        r#"{{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {{"type": "rectangle", "id": "q_gw", "x": 40, "y": 370, "width": 175, "height": 75}},
    {{"type": "rectangle", "id": "q_rewrite", "x": 275, "y": 370, "width": 175, "height": 75}},
    {{"type": "rectangle", "id": "blender", "x": 510, "y": 370, "width": 175, "height": 75}},
    {{"type": "rectangle", "id": "shard_1", "x": 745, "y": 265, "width": 175, "height": 75}},
    {{"type": "rectangle", "id": "shard_2", "x": {shard_2_x}, "y": 370, "width": 175, "height": 75}},
    {{"type": "rectangle", "id": "shard_3", "x": 745, "y": 475, "width": 175, "height": 75}},
    {{"type": "rectangle", "id": "l1_ranker", "x": 1010, "y": 370, "width": 175, "height": 75}},
    {{"type": "rectangle", "id": "l2_reranker", "x": 1245, "y": 370, "width": 175, "height": 75}}
  ]
}}"#
    )
}

const SCATTER_IDS: &str = "q_gw,q_rewrite,blender,shard_2,l1_ranker,l2_reranker";

fn distribute_scatter(src: &str, tag: &str) -> (i32, String, String) {
    let path = temp_path(tag);
    fs::write(&path, src).expect("write scatter-gather");
    let path_str = path.to_str().expect("utf-8 path");
    let result = run_layout(&[
        "layout",
        "distribute",
        path_str,
        "--ids",
        SCATTER_IDS,
        "--direction",
        "horizontal",
        "--gap",
        "90",
    ]);
    cleanup(&path);
    result
}

#[test]
fn distribute_scatter_gather_stack_moves_as_a_unit() {
    let src = scatter_gather_pipeline(745.0);
    let path = temp_path("b17-lint.excalidraw");
    fs::write(&path, &src).expect("write scatter-gather");
    let path_str = path.to_str().expect("utf-8 path");
    let lint = Command::new(bin())
        .args(["lint", path_str, "--format", "json"])
        .output()
        .expect("lint");
    cleanup(&path);
    let lint_stdout = String::from_utf8(lint.stdout).expect("lint stdout");
    let lint_json = parse_json(&lint_stdout);
    let recommended = format!(
        "excal layout distribute {path_str} --ids {SCATTER_IDS} --direction horizontal --gap 90"
    );
    let issues = lint_json["issues"].as_array().expect("issues");
    assert!(
        issues.iter().any(|issue| {
            issue["rule"] == "UNEVEN_SPACING"
                && issue["severity"] == "info"
                && issue["command"] == recommended
        }),
        "missing UNEVEN_SPACING recommending {recommended}: {issues:?}"
    );

    let (code, stdout, stderr) = distribute_scatter(&src, "b17-aligned.excalidraw");
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(
        stderr.contains("excal: moved 7 item(s), resized 0 item(s), updated 0 connector(s)"),
        "stderr={stderr}"
    );
    assert!(
        stderr.contains("excal: lint after change: errors: 0, warnings: 0, info: 0"),
        "stderr={stderr}"
    );

    let out = parse_json(&stdout);
    approx(num(element(&out, "q_rewrite"), "x"), 305.0);
    approx(num(element(&out, "blender"), "x"), 570.0);
    for id in ["shard_1", "shard_2", "shard_3"] {
        approx(num(element(&out, id), "x"), 835.0);
    }
    approx(num(element(&out, "shard_1"), "y"), 265.0);
    approx(num(element(&out, "shard_2"), "y"), 370.0);
    approx(num(element(&out, "shard_3"), "y"), 475.0);
    approx(num(element(&out, "l1_ranker"), "x"), 1100.0);
    approx(num(element(&out, "l2_reranker"), "x"), 1365.0);
}

#[test]
fn distribute_near_miss_shard_stack_cotranslates_then_aligns() {
    let (code, stdout, stderr) =
        distribute_scatter(&scatter_gather_pipeline(749.0), "b17-near.excalidraw");
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    let out = parse_json(&stdout);
    approx(num(element(&out, "shard_2"), "x"), 835.0);
    approx(num(element(&out, "shard_1"), "x"), 831.0);
    approx(num(element(&out, "shard_3"), "x"), 831.0);

    let path = temp_path("b17-align.excalidraw");
    fs::write(&path, &stdout).expect("write near-miss result");
    let path_str = path.to_str().expect("utf-8 path");
    let (code, aligned, stderr) = run_layout(&[
        "layout",
        "align",
        path_str,
        "--ids",
        "shard_2,shard_1,shard_3",
        "--axis",
        "center",
    ]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={aligned} stderr={stderr}");
    assert!(
        stderr.contains("excal: lint after change: errors: 0, warnings: 0, info: 0"),
        "stderr={stderr}"
    );
    let out = parse_json(&aligned);
    approx(num(element(&out, "shard_1"), "x"), 835.0);
    approx(num(element(&out, "shard_3"), "x"), 835.0);
}

fn grid_3x3() -> String {
    let mut elements = Vec::new();
    for (row, y) in [(1, 0.0), (2, 160.0), (3, 320.0)] {
        for (col, x) in [(1, 0.0), (2, 200.0), (3, 400.0)] {
            elements.push(format!(
                r#"{{"type": "rectangle", "id": "r{row}c{col}", "x": {x}, "y": {y}, "width": 140, "height": 80}}"#
            ));
        }
    }
    format!(
        r#"{{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {}
  ]
}}"#,
        elements.join(",\n    ")
    )
}

#[test]
fn distribute_middle_row_of_grid_does_not_cotranslate_other_rows() {
    let path = temp_path("b17-grid.excalidraw");
    fs::write(&path, grid_3x3()).expect("write grid");
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "distribute",
        path_str,
        "--ids",
        "r2c1,r2c2,r2c3",
        "--direction",
        "horizontal",
        "--gap",
        "100",
    ]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(stderr.contains("excal: moved 2 item(s)"), "stderr={stderr}");

    let out = parse_json(&stdout);
    for (id, x, y) in [
        ("r1c1", 0.0, 0.0),
        ("r1c2", 200.0, 0.0),
        ("r1c3", 400.0, 0.0),
        ("r3c1", 0.0, 320.0),
        ("r3c2", 200.0, 320.0),
        ("r3c3", 400.0, 320.0),
    ] {
        approx(num(element(&out, id), "x"), x);
        approx(num(element(&out, id), "y"), y);
    }
}

fn lane_with_three_boxes() -> String {
    r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "z", "x": 0, "y": 0, "width": 600, "height": 300},
    {"type": "text", "id": "t", "x": 16, "y": 12, "width": 80, "height": 25, "text": "Lane", "fontSize": 20},
    {"type": "rectangle", "id": "a", "x": 40, "y": 110, "width": 140, "height": 80},
    {"type": "rectangle", "id": "b", "x": 220, "y": 110, "width": 140, "height": 80},
    {"type": "rectangle", "id": "c", "x": 400, "y": 110, "width": 140, "height": 80}
  ]
}"#
    .to_string()
}

fn scene_of(doc_json: &str) -> Scene {
    Scene::build(&Document::parse(doc_json).expect("parse layout output"))
}

#[test]
fn distribute_straddling_child_grows_zone_on_crossed_side_only() {
    let path = temp_path("b16-straddle.excalidraw");
    fs::write(&path, lane_with_three_boxes()).expect("write lane");
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "distribute",
        path_str,
        "--ids",
        "a,b,c",
        "--direction",
        "horizontal",
        "--gap",
        "120",
    ]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(
        stderr.contains("excal: moved 2 item(s), resized 1 item(s), updated 0 connector(s)"),
        "stderr={stderr}"
    );
    assert!(
        stderr.contains("excal: lint after change: errors: 0, warnings: 0, info: 0"),
        "stderr={stderr}"
    );

    let out = parse_json(&stdout);
    approx(num(element(&out, "b"), "x"), 300.0);
    approx(num(element(&out, "c"), "x"), 560.0);
    let zone = element(&out, "z");
    approx(num(zone, "x"), 0.0);
    approx(num(zone, "y"), 0.0);
    approx(num(zone, "width"), 740.0);
    approx(num(zone, "height"), 300.0);
    let title = element(&out, "t");
    approx(num(title, "x"), 16.0);
    approx(num(title, "y"), 12.0);

    let scene = scene_of(&stdout);
    assert_eq!(scene.component("c").unwrap().parent.as_deref(), Some("z"));
}

#[test]
fn distribute_fully_outside_child_retains_zone_membership() {
    let path = temp_path("b16-outside.excalidraw");
    fs::write(&path, lane_with_three_boxes()).expect("write lane");
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "distribute",
        path_str,
        "--ids",
        "a,b,c",
        "--direction",
        "horizontal",
        "--gap",
        "200",
    ]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(
        stderr.contains("excal: lint after change: errors: 0, warnings: 0, info: 0"),
        "stderr={stderr}"
    );

    let out = parse_json(&stdout);
    approx(num(element(&out, "c"), "x"), 720.0);
    approx(num(element(&out, "z"), "width"), 900.0);
    let scene = scene_of(&stdout);
    assert_eq!(scene.component("c").unwrap().parent.as_deref(), Some("z"));
}

#[test]
fn move_straddle_retains_but_full_exit_or_place_below_releases() {
    let path = temp_path("b16-move-straddle.excalidraw");
    fs::write(&path, lane_with_three_boxes()).expect("write lane");
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) =
        run_layout(&["layout", "move", path_str, "--id", "c", "--dx", "180"]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(
        stderr.contains("excal: lint after change: errors: 0"),
        "stderr={stderr}"
    );
    let out = parse_json(&stdout);
    approx(num(element(&out, "z"), "width"), 760.0);
    let scene = scene_of(&stdout);
    assert_eq!(scene.component("c").unwrap().parent.as_deref(), Some("z"));

    let path = temp_path("b16-move-exit.excalidraw");
    fs::write(&path, lane_with_three_boxes()).expect("write lane");
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) =
        run_layout(&["layout", "move", path_str, "--id", "c", "--dx", "400"]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(stderr.contains("resized 0 item(s)"), "stderr={stderr}");
    let out = parse_json(&stdout);
    approx(num(element(&out, "z"), "width"), 600.0);
    approx(num(element(&out, "c"), "x"), 800.0);
    let scene = scene_of(&stdout);
    assert_eq!(scene.component("c").unwrap().parent, None);
    let zone = scene.container("z").expect("zone remains");
    assert_eq!(zone.kind, ContainerKind::Zone);
    let kids: Vec<_> = scene
        .components
        .iter()
        .filter(|component| component.parent.as_deref() == Some("z"))
        .map(|component| component.id.as_str())
        .collect();
    assert_eq!(kids, ["a", "b"]);

    let path = temp_path("b16-place-below.excalidraw");
    fs::write(&path, lane_with_three_boxes()).expect("write lane");
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) =
        run_layout(&["layout", "place", path_str, "--id", "c", "--below", "z"]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(stderr.contains("resized 0 item(s)"), "stderr={stderr}");
    let scene = scene_of(&stdout);
    assert!(scene.component("c").unwrap().rect.y1 >= 300.0);
    assert_eq!(scene.component("c").unwrap().parent, None);
}

fn adjacent_sibling_lanes() -> String {
    // z2 is listed before c, so its z is lower and its center-containment would
    // otherwise steal c after c straddles into z2.
    r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "z", "x": 0, "y": 0, "width": 600, "height": 300},
    {"type": "rectangle", "id": "z2", "x": 640, "y": 0, "width": 400, "height": 300},
    {"type": "rectangle", "id": "a", "x": 40, "y": 110, "width": 140, "height": 80},
    {"type": "rectangle", "id": "b", "x": 220, "y": 110, "width": 140, "height": 80},
    {"type": "rectangle", "id": "c", "x": 400, "y": 110, "width": 140, "height": 80},
    {"type": "rectangle", "id": "d", "x": 680, "y": 110, "width": 140, "height": 80},
    {"type": "rectangle", "id": "e", "x": 860, "y": 110, "width": 140, "height": 80}
  ]
}"#
    .to_string()
}

#[test]
fn adjacent_sibling_zone_does_not_steal_retained_child() {
    let path = temp_path("b16-sibling.excalidraw");
    fs::write(&path, adjacent_sibling_lanes()).expect("write lanes");
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "distribute",
        path_str,
        "--ids",
        "a,b,c",
        "--direction",
        "horizontal",
        "--gap",
        "130",
    ]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");

    let out = parse_json(&stdout);
    approx(num(element(&out, "c"), "x"), 580.0);
    let zone = element(&out, "z");
    approx(num(zone, "x"), 0.0);
    approx(num(zone, "width"), 760.0);
    let scene = scene_of(&stdout);
    assert_eq!(scene.component("c").unwrap().parent.as_deref(), Some("z"));
    assert_eq!(scene.container("z2").unwrap().parent, None);

    let grown = temp_path("b16-sibling-grown.excalidraw");
    fs::write(&grown, &stdout).expect("write grown");
    let grown_str = grown.to_str().expect("utf-8 path");
    let (code, moved, stderr) =
        run_layout(&["layout", "move", grown_str, "--id", "z2", "--dx", "160"]);
    cleanup(&grown);
    assert_eq!(code, 0, "stdout={moved} stderr={stderr}");
    assert!(
        stderr.contains("excal: lint after change: errors: 0, warnings: 0, info: 0"),
        "stderr={stderr}"
    );
    let after = lint_json(&moved);
    let issues = after["issues"].as_array().expect("issues");
    assert!(
        issues
            .iter()
            .all(|issue| issue["rule"] != "OVERLAP_COMPONENT"),
        "remaining overlaps: {after}"
    );
}

fn nested_pci_ingress() -> String {
    // Same nesting as the scene attribution case, with z_ingress already
    // inset so fit-container must grow it without entering the parent title band.
    r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "z_pci", "x": 0, "y": 0, "width": 1080, "height": 620},
    {"type": "text", "id": "z_pci_title", "x": 16, "y": 12, "width": 324, "height": 25, "text": "Global PCI-DSS Payment Mesh", "fontSize": 20, "lineHeight": 1.25},
    {"type": "rectangle", "id": "z_ingress", "x": 30, "y": 55, "width": 480, "height": 250},
    {"type": "text", "id": "z_ingress_title", "x": 46, "y": 67, "width": 228, "height": 25, "text": "1. Edge & Risk Gate", "fontSize": 20, "lineHeight": 1.25},
    {"type": "rectangle", "id": "pay_gw", "x": 60, "y": 120, "width": 175, "height": 75},
    {"type": "diamond", "id": "risk_decision", "x": 300, "y": 86, "width": 180, "height": 144}
  ]
}"#
    .to_string()
}

#[test]
fn fit_container_nested_zone_stops_at_parent_title_band() {
    let path = temp_path("b18-fit-ingress.excalidraw");
    fs::write(&path, nested_pci_ingress()).expect("write nested zones");
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "fit-container",
        path_str,
        "--id",
        "z_ingress",
        "--padding",
        "40",
        "--expand-only",
    ]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(
        stderr.contains("excal: lint after change: errors: 0, warnings: 0, info: 0"),
        "stderr={stderr}"
    );

    let out = parse_json(&stdout);
    let ingress = element(&out, "z_ingress");
    approx(num(ingress, "x"), 20.0);
    approx(num(ingress, "y"), 52.5);
    approx(num(ingress, "width"), 500.0);
    approx(num(ingress, "height"), 252.5);
    let title = element(&out, "z_ingress_title");
    approx(num(title, "x"), 36.0);
    approx(num(title, "y"), 55.0);

    let scene = scene_of(&stdout);
    assert_eq!(
        scene.container("z_pci").unwrap().title_element.as_deref(),
        Some("z_pci_title")
    );
    assert_eq!(
        scene
            .container("z_ingress")
            .unwrap()
            .title_element
            .as_deref(),
        Some("z_ingress_title")
    );
}

fn fixture_p() -> String {
    r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "z_prefill", "x": 430, "y": 55, "width": 560, "height": 210},
    {"type": "text", "id": "z_prefill_title", "x": 446, "y": 67, "width": 420, "height": 25, "text": "2a. Prefill Cluster", "fontSize": 20},
    {"type": "rectangle", "id": "p_sched", "x": 460, "y": 115, "width": 160, "height": 75},
    {"type": "rectangle", "id": "p_node1", "x": 600, "y": 115, "width": 160, "height": 75},
    {"type": "rectangle", "id": "p_node2", "x": 810, "y": 115, "width": 160, "height": 75}
  ]
}"#
    .to_string()
}

fn fixture_p_plus_r() -> String {
    r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "z_prefill", "x": 430, "y": 55, "width": 560, "height": 210},
    {"type": "text", "id": "z_prefill_title", "x": 446, "y": 67, "width": 420, "height": 25, "text": "2a. Prefill Cluster", "fontSize": 20},
    {"type": "rectangle", "id": "z_rag", "x": 1030, "y": 55, "width": 610, "height": 210},
    {"type": "text", "id": "z_rag_title", "x": 1046, "y": 67, "width": 200, "height": 25, "text": "2b. RAG", "fontSize": 20},
    {"type": "rectangle", "id": "p_sched", "x": 460, "y": 115, "width": 160, "height": 75},
    {"type": "rectangle", "id": "p_node1", "x": 600, "y": 115, "width": 160, "height": 75},
    {"type": "rectangle", "id": "p_node2", "x": 810, "y": 115, "width": 160, "height": 75},
    {"type": "rectangle", "id": "embed_svc", "x": 1250, "y": 115, "width": 160, "height": 75},
    {"type": "rectangle", "id": "milvus", "x": 1450, "y": 115, "width": 160, "height": 75}
  ]
}"#
    .to_string()
}

fn fixture_n() -> String {
    r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "z_pci", "x": 0, "y": 0, "width": 1080, "height": 620},
    {"type": "text", "id": "z_pci_title", "x": 16, "y": 12, "width": 324, "height": 25, "text": "Global PCI-DSS Payment Mesh", "fontSize": 20},
    {"type": "rectangle", "id": "z_ingress", "x": 20, "y": 150, "width": 500, "height": 260},
    {"type": "text", "id": "z_ingress_title", "x": 36, "y": 162, "width": 228, "height": 25, "text": "1. Edge & Risk Gate", "fontSize": 20},
    {"type": "rectangle", "id": "pay_gw", "x": 60, "y": 240, "width": 175, "height": 75},
    {"type": "rectangle", "id": "risk", "x": 300, "y": 240, "width": 175, "height": 75}
  ]
}"#
    .to_string()
}

fn write_fixture(tag: &str, body: &str) -> PathBuf {
    let path = temp_path(tag);
    fs::write(&path, body).expect("write fixture");
    path
}

#[test]
fn same_parent_place_beside_sibling_zone_retains_and_overlap_is_movable() {
    let path = write_fixture("b19-place-sibling.excalidraw", &fixture_p_plus_r());
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "place",
        path_str,
        "--id",
        "p_node2",
        "--right-of",
        "p_node1",
        "--gap",
        "210",
    ]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(
        stderr.contains("excal: moved 1 item(s), resized 1 item(s), updated 0 connector(s)"),
        "stderr={stderr}"
    );

    let out = parse_json(&stdout);
    approx(num(element(&out, "p_node2"), "x"), 970.0);
    let prefill = element(&out, "z_prefill");
    approx(num(prefill, "x"), 430.0);
    approx(num(prefill, "y"), 55.0);
    approx(num(prefill, "width"), 740.0);
    approx(num(prefill, "height"), 210.0);
    let scene = scene_of(&stdout);
    assert_eq!(
        scene.component("p_node2").unwrap().parent.as_deref(),
        Some("z_prefill")
    );

    let issues = lint_issues(&stdout);
    let overlap = issues
        .iter()
        .find(|issue| {
            issue["rule"] == "OVERLAP_COMPONENT"
                && issue["targets"] == serde_json::json!(["z_prefill", "z_rag"])
        })
        .unwrap_or_else(|| panic!("expected z_prefill/z_rag overlap, issues={issues:?}"));
    let command = overlap["command"].as_str().expect("command");
    assert!(
        command.ends_with(" --id z_rag --dx 180"),
        "command={command}"
    );

    let grown = write_fixture("b19-place-sibling-grown.excalidraw", &stdout);
    let grown_str = grown.to_str().expect("utf-8 path");
    let (code, moved, stderr) =
        run_layout(&["layout", "move", grown_str, "--id", "z_rag", "--dx", "180"]);
    cleanup(&grown);
    assert_eq!(code, 0, "stdout={moved} stderr={stderr}");
    let moved_doc = parse_json(&moved);
    let rag = element(&moved_doc, "z_rag");
    approx(num(rag, "x"), 1210.0);
    let scene = scene_of(&moved);
    assert_eq!(
        scene.component("p_node2").unwrap().parent.as_deref(),
        Some("z_prefill")
    );
    assert!(
        lint_issues(&moved).iter().all(|issue| {
            !(issue["rule"] == "OVERLAP_COMPONENT"
                && issue["targets"]
                    .as_array()
                    .is_some_and(|targets| targets.iter().any(|target| target == "z_rag")))
        }),
        "z_rag still overlaps: {}",
        serde_json::to_string(&lint_issues(&moved)).expect("issues json")
    );
}

#[test]
fn same_parent_place_disjoint_from_old_zone_still_retains() {
    let path = write_fixture("b19-place-disjoint.excalidraw", &fixture_p());
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "place",
        path_str,
        "--id",
        "p_node2",
        "--right-of",
        "p_node1",
        "--gap",
        "300",
    ]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(stderr.contains("resized 1 item(s)"), "stderr={stderr}");

    let out = parse_json(&stdout);
    approx(num(element(&out, "p_node2"), "x"), 1060.0);
    approx(num(element(&out, "z_prefill"), "width"), 830.0);
    let scene = scene_of(&stdout);
    assert_eq!(
        scene.component("p_node2").unwrap().parent.as_deref(),
        Some("z_prefill")
    );
}

#[test]
fn cross_parent_place_and_move_exit_do_not_grow_zone() {
    let path = write_fixture("b19-place-cross.excalidraw", &fixture_p());
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "place",
        path_str,
        "--id",
        "p_node2",
        "--right-of",
        "z_prefill",
    ]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(stderr.contains("resized 0 item(s)"), "stderr={stderr}");
    let out = parse_json(&stdout);
    let node = element(&out, "p_node2");
    approx(num(node, "x"), 1030.0);
    approx(num(node, "y"), 122.5);
    let scene = scene_of(&stdout);
    assert_eq!(scene.component("p_node2").unwrap().parent, None);

    let path = write_fixture("b19-move-exit.excalidraw", &fixture_p());
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) =
        run_layout(&["layout", "move", path_str, "--id", "p_node2", "--dx", "500"]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(stderr.contains("resized 0 item(s)"), "stderr={stderr}");
    approx(
        num(element(&parse_json(&stdout), "z_prefill"), "width"),
        560.0,
    );
}

#[test]
fn upward_place_reserves_title_extra_above_child() {
    let path = write_fixture("b19-title-extra.excalidraw", &lane_with_three_boxes());
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run_layout(&[
        "layout", "place", path_str, "--id", "c", "--above", "b", "--gap", "80",
    ]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(stderr.contains("resized 1 item(s)"), "stderr={stderr}");
    assert!(
        stderr.contains("excal: lint after change: errors: 0, warnings: 0, info: 0"),
        "stderr={stderr}"
    );

    let out = parse_json(&stdout);
    let child = element(&out, "c");
    approx(num(child, "x"), 220.0);
    approx(num(child, "y"), -50.0);
    let zone = element(&out, "z");
    approx(num(zone, "x"), 0.0);
    approx(num(zone, "y"), -125.0);
    approx(num(zone, "width"), 600.0);
    approx(num(zone, "height"), 425.0);
    let title = element(&out, "t");
    approx(num(title, "x"), 16.0);
    approx(num(title, "y"), -113.0);

    let scene = scene_of(&stdout);
    assert_eq!(scene.component("c").unwrap().parent.as_deref(), Some("z"));
    assert_eq!(
        scene.container("z").unwrap().title_element.as_deref(),
        Some("t")
    );
}

#[test]
fn nested_upward_place_cascades_when_child_enters_parent_title_band() {
    let path = write_fixture("b19-nested-cascade.excalidraw", &fixture_n());
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run_layout(&[
        "layout", "place", path_str, "--id", "risk", "--above", "pay_gw", "--gap", "75",
    ]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(
        stderr.contains("excal: moved 1 item(s), resized 2 item(s), updated 0 connector(s)"),
        "stderr={stderr}"
    );
    assert!(
        stderr.contains("excal: lint after change: errors: 0, warnings: 0, info: 0"),
        "stderr={stderr}"
    );

    let out = parse_json(&stdout);
    let risk = element(&out, "risk");
    approx(num(risk, "x"), 60.0);
    approx(num(risk, "y"), 90.0);
    let ingress = element(&out, "z_ingress");
    approx(num(ingress, "x"), 20.0);
    approx(num(ingress, "y"), 15.0);
    approx(num(ingress, "width"), 500.0);
    approx(num(ingress, "height"), 395.0);
    let ingress_title = element(&out, "z_ingress_title");
    approx(num(ingress_title, "x"), 36.0);
    approx(num(ingress_title, "y"), 27.0);
    let pci = element(&out, "z_pci");
    approx(num(pci, "x"), 0.0);
    approx(num(pci, "y"), -60.0);
    approx(num(pci, "width"), 1080.0);
    approx(num(pci, "height"), 680.0);
    let pci_title = element(&out, "z_pci_title");
    approx(num(pci_title, "x"), 16.0);
    approx(num(pci_title, "y"), -48.0);

    let scene = scene_of(&stdout);
    assert_eq!(
        scene
            .container("z_ingress")
            .unwrap()
            .title_element
            .as_deref(),
        Some("z_ingress_title")
    );
    assert_eq!(
        scene.container("z_pci").unwrap().title_element.as_deref(),
        Some("z_pci_title")
    );
    assert_eq!(
        scene.container("z_ingress").unwrap().parent.as_deref(),
        Some("z_pci")
    );
    assert_eq!(
        scene.component("risk").unwrap().parent.as_deref(),
        Some("z_ingress")
    );
}

#[test]
fn distribute_on_fixture_p_plus_r_retains_then_clears_overlap() {
    let path = write_fixture("b19-distribute.excalidraw", &fixture_p_plus_r());
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "distribute",
        path_str,
        "--ids",
        "p_sched,p_node1,p_node2",
        "--direction",
        "horizontal",
        "--gap",
        "95",
    ]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");

    let out = parse_json(&stdout);
    approx(num(element(&out, "p_node1"), "x"), 715.0);
    approx(num(element(&out, "p_node2"), "x"), 970.0);
    approx(num(element(&out, "z_prefill"), "width"), 740.0);
    let scene = scene_of(&stdout);
    assert_eq!(
        scene.component("p_node2").unwrap().parent.as_deref(),
        Some("z_prefill")
    );

    let grown = write_fixture("b19-distribute-grown.excalidraw", &stdout);
    let grown_str = grown.to_str().expect("utf-8 path");
    let (code, moved, stderr) =
        run_layout(&["layout", "move", grown_str, "--id", "z_rag", "--dx", "180"]);
    cleanup(&grown);
    assert_eq!(code, 0, "stdout={moved} stderr={stderr}");
    assert!(
        stderr.contains("excal: lint after change: errors: 0"),
        "stderr={stderr}"
    );
    let after = lint_json(&moved);
    assert_eq!(after["summary"]["errors"], 0, "lint={after}");
}

fn assert_no_overlap(doc_json: &str) {
    let issues = lint_issues(doc_json);
    assert!(
        issues
            .iter()
            .all(|issue| issue["rule"] != "OVERLAP_COMPONENT"),
        "overlaps: {}",
        serde_json::to_string(&issues).expect("issues json")
    );
}

fn assert_errors_zero(doc_json: &str) {
    let lint = lint_json(doc_json);
    assert_eq!(lint["summary"]["errors"], 0, "lint={lint}");
    assert_no_overlap(doc_json);
}

#[test]
fn distribute_full_containment_pushes_sibling_zone_and_keeps_parents() {
    let path = write_fixture("b20-push-sibling.excalidraw", &fixture_p_plus_r());
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "distribute",
        path_str,
        "--ids",
        "p_sched,p_node1,p_node2",
        "--direction",
        "horizontal",
        "--gap",
        "200",
    ]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(
        stderr.contains("excal: moved 5 item(s), resized 1 item(s), updated 0 connector(s)"),
        "stderr={stderr}"
    );

    let out = parse_json(&stdout);
    approx(num(element(&out, "p_node1"), "x"), 820.0);
    approx(num(element(&out, "p_node2"), "x"), 1180.0);
    let prefill = element(&out, "z_prefill");
    approx(num(prefill, "x"), 430.0);
    approx(num(prefill, "y"), 55.0);
    approx(num(prefill, "width"), 950.0);
    approx(num(prefill, "height"), 210.0);
    approx(num(element(&out, "z_rag"), "x"), 1420.0);
    approx(num(element(&out, "z_rag_title"), "x"), 1436.0);
    approx(num(element(&out, "embed_svc"), "x"), 1640.0);
    approx(num(element(&out, "milvus"), "x"), 1840.0);
    approx(num(element(&out, "z_rag"), "y"), 55.0);
    approx(num(element(&out, "z_rag_title"), "y"), 67.0);
    approx(num(element(&out, "embed_svc"), "y"), 115.0);
    approx(num(element(&out, "milvus"), "y"), 115.0);

    let scene = scene_of(&stdout);
    assert_eq!(
        scene.component("p_node2").unwrap().parent.as_deref(),
        Some("z_prefill")
    );
    assert_eq!(
        scene.component("embed_svc").unwrap().parent.as_deref(),
        Some("z_rag")
    );
    assert_eq!(
        scene.component("milvus").unwrap().parent.as_deref(),
        Some("z_rag")
    );
    assert_errors_zero(&stdout);
}

#[test]
fn distribute_pushes_larger_sibling_when_grown_zone_claims_its_child() {
    let body = fixture_p_plus_r()
        .replace(
            r#"{"type": "rectangle", "id": "z_rag", "x": 1030, "y": 55, "width": 610, "height": 210}"#,
            r#"{"type": "rectangle", "id": "z_rag", "x": 1030, "y": 55, "width": 1000, "height": 210}"#,
        )
        .replace(
            r#"{"type": "rectangle", "id": "embed_svc", "x": 1250, "y": 115, "width": 160, "height": 75}"#,
            r#"{"type": "rectangle", "id": "embed_svc", "x": 1050, "y": 115, "width": 160, "height": 75}"#,
        );
    let path = write_fixture("b20-larger-sibling.excalidraw", &body);
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "distribute",
        path_str,
        "--ids",
        "p_sched,p_node1,p_node2",
        "--direction",
        "horizontal",
        "--gap",
        "120",
    ]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(
        stderr.contains("excal: moved 5 item(s), resized 1 item(s), updated 0 connector(s)"),
        "stderr={stderr}"
    );

    let out = parse_json(&stdout);
    approx(num(element(&out, "p_node2"), "x"), 1020.0);
    let prefill = element(&out, "z_prefill");
    approx(num(prefill, "x"), 430.0);
    approx(num(prefill, "y"), 55.0);
    approx(num(prefill, "width"), 790.0);
    approx(num(prefill, "height"), 210.0);
    approx(num(element(&out, "z_rag"), "x"), 1260.0);
    approx(num(element(&out, "embed_svc"), "x"), 1280.0);
    approx(num(element(&out, "milvus"), "x"), 1680.0);

    let scene = scene_of(&stdout);
    assert_eq!(
        scene.component("p_node2").unwrap().parent.as_deref(),
        Some("z_prefill")
    );
    assert_eq!(
        scene.component("embed_svc").unwrap().parent.as_deref(),
        Some("z_rag")
    );
}

#[test]
fn distribute_push_cascades_into_enclosing_parent_zone() {
    let inner = fixture_p_plus_r();
    let elements_start = inner.find("\"elements\": [").expect("elements");
    let body = format!(
        "{}{{\"type\": \"rectangle\", \"id\": \"z_fabric\", \"x\": 0, \"y\": 0, \"width\": 1680, \"height\": 300}},\n    {{\"type\": \"text\", \"id\": \"z_fabric_title\", \"x\": 16, \"y\": 12, \"width\": 240, \"height\": 25, \"text\": \"2. GPU Fabric\", \"fontSize\": 20}},\n    {}",
        &inner[..elements_start + "\"elements\": [".len()],
        &inner[elements_start + "\"elements\": [".len()..]
    );
    let path = write_fixture("b20-nested-cascade.excalidraw", &body);
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "distribute",
        path_str,
        "--ids",
        "p_sched,p_node1,p_node2",
        "--direction",
        "horizontal",
        "--gap",
        "200",
    ]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(
        stderr.contains("excal: moved 5 item(s), resized 2 item(s), updated 0 connector(s)"),
        "stderr={stderr}"
    );

    let out = parse_json(&stdout);
    approx(num(element(&out, "p_node1"), "x"), 820.0);
    approx(num(element(&out, "p_node2"), "x"), 1180.0);
    let prefill = element(&out, "z_prefill");
    approx(num(prefill, "x"), 430.0);
    approx(num(prefill, "y"), 55.0);
    approx(num(prefill, "width"), 950.0);
    approx(num(prefill, "height"), 210.0);
    approx(num(element(&out, "z_rag"), "x"), 1420.0);
    let fabric = element(&out, "z_fabric");
    approx(num(fabric, "x"), 0.0);
    approx(num(fabric, "y"), 0.0);
    approx(num(fabric, "width"), 2070.0);
    approx(num(fabric, "height"), 300.0);

    let scene = scene_of(&stdout);
    assert_eq!(
        scene.container("z_prefill").unwrap().parent.as_deref(),
        Some("z_fabric")
    );
    assert_eq!(
        scene.container("z_rag").unwrap().parent.as_deref(),
        Some("z_fabric")
    );
}

#[test]
fn distribute_push_ripples_same_band_and_leaves_lower_row() {
    let inner = fixture_p_plus_r();
    let close = inner.rfind(']').expect("elements close");
    let body = format!(
        "{},\n    {{\"type\": \"rectangle\", \"id\": \"z_store\", \"x\": 1680, \"y\": 55, \"width\": 400, \"height\": 210}},\n    {{\"type\": \"rectangle\", \"id\": \"z_lower\", \"x\": 1030, \"y\": 305, \"width\": 560, \"height\": 220}},\n    {{\"type\": \"rectangle\", \"id\": \"st_a\", \"x\": 1720, \"y\": 115, \"width\": 140, \"height\": 75}},\n    {{\"type\": \"rectangle\", \"id\": \"st_b\", \"x\": 1900, \"y\": 115, \"width\": 140, \"height\": 75}},\n    {{\"type\": \"rectangle\", \"id\": \"lo_a\", \"x\": 1070, \"y\": 365, \"width\": 140, \"height\": 75}},\n    {{\"type\": \"rectangle\", \"id\": \"lo_b\", \"x\": 1250, \"y\": 365, \"width\": 140, \"height\": 75}}\n  {}",
        &inner[..close],
        &inner[close..]
    );
    let path = write_fixture("b20-ripple.excalidraw", &body);
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run_layout(&[
        "layout",
        "distribute",
        path_str,
        "--ids",
        "p_sched,p_node1,p_node2",
        "--direction",
        "horizontal",
        "--gap",
        "200",
    ]);
    cleanup(&path);
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(
        stderr.contains("excal: moved 8 item(s), resized 1 item(s), updated 0 connector(s)"),
        "stderr={stderr}"
    );

    let out = parse_json(&stdout);
    approx(num(element(&out, "z_rag"), "x"), 1420.0);
    approx(num(element(&out, "z_store"), "x"), 2070.0);
    approx(num(element(&out, "st_a"), "x"), 2110.0);
    approx(num(element(&out, "st_b"), "x"), 2290.0);
    approx(num(element(&out, "z_lower"), "x"), 1030.0);
    approx(num(element(&out, "lo_a"), "x"), 1070.0);
    approx(num(element(&out, "lo_b"), "x"), 1250.0);
}
