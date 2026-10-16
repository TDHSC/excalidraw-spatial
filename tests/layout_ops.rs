use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use excalidraw_spatial::linter::run_lint;
use excalidraw_spatial::scene_graph::Scene;
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
