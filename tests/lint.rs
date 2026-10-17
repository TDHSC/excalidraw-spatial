use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use excalidraw_spatial::linter::run_lint;
use excalidraw_spatial::scene_graph::Scene;
use excalidraw_spatial::schema::Document;
use excalidraw_spatial::text_metrics::TextSize;

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

fn make_temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("excal-t24-{}-{}-{tag}", std::process::id(), nanos));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn run_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_excal"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("failed to execute excal");
    let code = output.status.code().expect("process terminated by signal");
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr utf-8");
    (code, stdout, stderr)
}

fn lint_json_in(dir: &Path) -> serde_json::Value {
    let (code, stdout, stderr) = run_in(dir, &["lint", "f.excalidraw", "--format", "json"]);
    assert!(
        code == 0 || code == 1,
        "lint should parse the fixture; code={code} stdout={stdout} stderr={stderr}"
    );
    serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("lint json parse failed: {e}; code={code} stdout={stdout} stderr={stderr}")
    })
}

fn issues_with_rule<'a>(v: &'a serde_json::Value, rule: &str) -> Vec<&'a serde_json::Value> {
    v["issues"]
        .as_array()
        .expect("issues array")
        .iter()
        .filter(|issue| issue["rule"] == rule)
        .collect()
}

#[test]
fn lint_overlap_matches_golden_and_exits_1() {
    let (code, stdout, _stderr) = run(&["lint", "tests/fixtures/overlap.excalidraw"]);
    assert_eq!(code, 1);
    assert_eq!(stdout, include_str!("expected/overlap.lint.txt"));
}

#[test]
fn lint_swimlanes_matches_golden_and_exits_1() {
    let (code, stdout, _stderr) = run(&["lint", "tests/fixtures/swimlanes.excalidraw"]);
    assert_eq!(code, 1);
    assert_eq!(stdout, include_str!("expected/swimlanes.lint.txt"));
}

#[test]
fn lint_basic_flow_matches_golden_and_exits_0() {
    let (code, stdout, _stderr) = run(&["lint", "tests/fixtures/basic_flow.excalidraw"]);
    assert_eq!(code, 0);
    assert_eq!(stdout, include_str!("expected/basic_flow.lint.txt"));
}

#[test]
fn lint_text_layout_matches_golden_and_exits_0() {
    let (code, stdout, _stderr) = run(&["lint", "tests/fixtures/text_layout.excalidraw"]);
    assert_eq!(code, 0);
    assert_eq!(stdout, include_str!("expected/text_layout.lint.txt"));
}

#[test]
fn lint_arrows_matches_golden_and_exits_1() {
    let (code, stdout, _stderr) = run(&["lint", "tests/fixtures/arrows.excalidraw"]);
    assert_eq!(code, 1);
    assert_eq!(stdout, include_str!("expected/arrows.lint.txt"));
}

#[test]
fn lint_icon_group_matches_golden_and_exits_0() {
    let (code, stdout, _stderr) = run(&["lint", "tests/fixtures/icon_group.excalidraw"]);
    assert_eq!(code, 0);
    assert_eq!(stdout, include_str!("expected/icon_group.lint.txt"));
}

#[test]
fn lint_all_fixtures_match_golden() {
    for (name, code) in [
        ("arrows", 1),
        ("basic_flow", 0),
        ("icon_group", 0),
        ("overlap", 1),
        ("swimlanes", 1),
        ("text_layout", 0),
    ] {
        let (got, stdout, _) = run(&["lint", &format!("tests/fixtures/{name}.excalidraw")]);
        let expected = std::fs::read_to_string(format!("tests/expected/{name}.lint.txt")).unwrap();
        assert_eq!(got, code, "{name}");
        assert_eq!(stdout, expected, "{name}");
    }
}

#[test]
fn lint_text_layout_strict_exits_1_with_same_stdout() {
    let (code, stdout, _stderr) =
        run(&["lint", "tests/fixtures/text_layout.excalidraw", "--strict"]);
    assert_eq!(code, 1);
    assert_eq!(stdout, include_str!("expected/text_layout.lint.txt"));
}

#[test]
fn lint_stdin_still_reports_text_overflow() {
    let fixture = File::open("tests/fixtures/text_layout.excalidraw").expect("open fixture");
    let output = Command::new(env!("CARGO_BIN_EXE_excal"))
        .args(["lint", "-"])
        .stdin(Stdio::from(fixture))
        .output()
        .expect("failed to execute excal");
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let expected = include_str!("expected/text_layout.lint.txt")
        .replace("tests/fixtures/text_layout.excalidraw", "<file>");
    assert_eq!(stdout, expected);
}

#[test]
fn lint_overlap_json_matches_spec_sample() {
    let (code, stdout, _stderr) = run(&[
        "lint",
        "tests/fixtures/overlap.excalidraw",
        "--format",
        "json",
    ]);
    assert_eq!(code, 1);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("lint json");
    assert_eq!(
        v["summary"],
        serde_json::json!({"errors": 1, "warnings": 0, "info": 0})
    );
    let issue = &v["issues"][0];
    assert_eq!(issue["rule"], "OVERLAP_COMPONENT");
    assert_eq!(issue["severity"], "error");
    assert_eq!(issue["targets"], serde_json::json!(["api", "auth"]));
    assert_eq!(
        issue["message"],
        "api \"API\" overlaps auth \"Auth\" by 24x60px"
    );
    assert_eq!(issue["fix"], "move auth right by 64px to leave a 40px gap");
    assert_eq!(
        issue["command"],
        "excal layout move tests/fixtures/overlap.excalidraw --id auth --dx 64"
    );
}

#[test]
fn lint_missing_file_exits_2_with_error_prefix() {
    let (code, _stdout, stderr) = run(&["lint", "does/not/exist.excalidraw"]);
    assert_eq!(code, 2);
    assert!(
        stderr.starts_with("excal: error:"),
        "stderr should start with 'excal: error:', got {stderr:?}"
    );
}

#[test]
fn lint_stdin_uses_placeholder_file_in_command() {
    let fixture = File::open("tests/fixtures/overlap.excalidraw").expect("open fixture");
    let output = Command::new(env!("CARGO_BIN_EXE_excal"))
        .args(["lint", "-"])
        .stdin(Stdio::from(fixture))
        .output()
        .expect("failed to execute excal");
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    assert!(
        stdout.contains("excal layout move <file> --id auth --dx 64"),
        "stdout missing stdin command placeholder:\n{stdout}"
    );
}

#[test]
fn lint_zero_width_arrow_label_reports_estimated_collision() {
    let path = temp_path("arrow-label.excalidraw");
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
    fs::write(&path, src).expect("write temp lint file");
    let path_str = path.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run(&["lint", path_str]);
    cleanup(&path);
    assert_eq!(code, 1, "stdout={stdout} stderr={stderr}");
    let collisions: Vec<&str> = stdout
        .lines()
        .filter(|line| line.contains("ARROW_LABEL_COLLISION"))
        .collect();
    assert_eq!(
        collisions.len(),
        2,
        "expected 2 ARROW_LABEL_COLLISION errors, got {}:\n{stdout}",
        collisions.len()
    );
    for line in &collisions {
        assert!(
            line.contains("42x20px"),
            "collision message missing 42x20px: {line}"
        );
    }
    assert!(
        stdout.contains(&format!("excal layout move {path_str} --id b --dx 104")),
        "stdout missing lengthen command:\n{stdout}"
    );
}

#[test]
fn unbound_title_and_subtitle_accumulate_envelope_size() {
    // Title sits inside the box; the subtitle starts left of the box and its
    // vertical span is not symmetric about the box center.
    let scene = Scene::build(
        &Document::parse(
            r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "box", "x": 100, "y": 100, "width": 160, "height": 80},
    {
      "type": "text",
      "id": "title",
      "x": 150,
      "y": 112,
      "width": 60,
      "height": 25,
      "text": "Redis",
      "fontSize": 20
    },
    {
      "type": "text",
      "id": "sub",
      "x": 70,
      "y": 145,
      "width": 220,
      "height": 20,
      "text": "cluster-01.prod.internal:6379",
      "fontSize": 16
    }
  ]
}"#,
        )
        .expect("parse"),
    );

    let box_shape = scene.component("box").expect("box");
    assert_eq!(box_shape.label.as_deref(), Some("Redis"));
    assert_eq!(scene.notes.len(), 1);
    assert_eq!(
        box_shape.label_size,
        Some(TextSize {
            width: 220.0,
            height: 50.0,
            estimated: false,
        })
    );

    let issues = run_lint(&scene, "b08.excalidraw");
    let overflow = issues
        .iter()
        .find(|issue| issue.rule == "TEXT_OVERFLOW")
        .expect("TEXT_OVERFLOW");
    assert_eq!(
        overflow.severity,
        excalidraw_spatial::linter::Severity::Warn
    );
    assert_eq!(
        overflow.message,
        "label of box \"Redis\" needs 220x50px but the rectangle is 160x80px"
    );
}

#[test]
fn corner_badge_inside_usable_inner_box_does_not_overflow() {
    // Bound "Main" plus an unbound badge that sits inside the padded inner box
    // (x10..190, y8..92) of a 200x100 rectangle.
    let scene = Scene::build(
        &Document::parse(
            r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "box", "x": 0, "y": 0, "width": 200, "height": 100},
    {
      "type": "text",
      "id": "main",
      "x": 70,
      "y": 37.5,
      "width": 60,
      "height": 25,
      "text": "Main",
      "fontSize": 20,
      "containerId": "box"
    },
    {
      "type": "text",
      "id": "tag",
      "x": 15,
      "y": 12,
      "width": 30,
      "height": 16,
      "text": "v2",
      "fontSize": 14
    }
  ]
}"#,
        )
        .expect("parse"),
    );

    let issues = run_lint(&scene, "badge.excalidraw");
    assert!(
        issues.iter().all(|issue| issue.rule != "TEXT_OVERFLOW"),
        "corner badge should not overflow: {issues:?}"
    );
}

#[test]
fn b06_sibling_zones_emit_overlap_component() {
    let dir = make_temp_dir("b06-zones");
    let path = dir.join("f.excalidraw");
    let src = r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "lane_fe", "x": 0, "y": 35, "width": 460, "height": 195},
    {"type": "text", "id": "fe_title", "x": 16, "y": 40, "width": 80, "height": 20, "text": "Frontend", "fontSize": 16, "textAlign": "left"},
    {"type": "rectangle", "id": "fe_child", "x": 40, "y": 90, "width": 80, "height": 40},
    {"type": "rectangle", "id": "lane_be", "x": 440, "y": 0, "width": 400, "height": 300},
    {"type": "text", "id": "be_title", "x": 456, "y": 8, "width": 80, "height": 20, "text": "Backend", "fontSize": 16, "textAlign": "left"},
    {"type": "rectangle", "id": "be_child", "x": 500, "y": 80, "width": 80, "height": 40}
  ]
}"#;
    fs::write(&path, src).expect("write temp lint file");
    let v = lint_json_in(&dir);
    let _ = fs::remove_dir_all(&dir);
    let hits: Vec<_> = issues_with_rule(&v, "OVERLAP_COMPONENT")
        .into_iter()
        .filter(|issue| {
            issue["message"] == r#"lane_fe "Frontend" overlaps lane_be "Backend" by 20x195px"#
                && issue["command"] == "excal layout move f.excalidraw --id lane_be --dx 60"
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected OVERLAP_COMPONENT for sibling zones, got {}",
        serde_json::to_string_pretty(&v["issues"]).unwrap()
    );
    assert_eq!(hits[0]["severity"], "error");
    assert_eq!(
        hits[0]["targets"],
        serde_json::json!(["lane_fe", "lane_be"])
    );
    assert_eq!(
        hits[0]["fix"],
        "move lane_be right by 60px to leave a 40px gap"
    );
}

#[test]
fn b06_external_node_intruding_into_container() {
    let dir = make_temp_dir("b06-ext");
    let path = dir.join("f.excalidraw");
    let src = r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "lane_be", "x": 440, "y": 0, "width": 400, "height": 300},
    {"type": "rectangle", "id": "c1", "x": 480, "y": 110, "width": 80, "height": 40},
    {"type": "rectangle", "id": "c2", "x": 600, "y": 110, "width": 80, "height": 40},
    {"type": "rectangle", "id": "ext", "x": 380, "y": 110, "width": 100, "height": 80}
  ]
}"#;
    fs::write(&path, src).expect("write temp lint file");
    let v = lint_json_in(&dir);
    let _ = fs::remove_dir_all(&dir);
    let hits: Vec<_> = issues_with_rule(&v, "OVERLAP_COMPONENT")
        .into_iter()
        .filter(|issue| issue["targets"] == serde_json::json!(["lane_be", "ext"]))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected OVERLAP_COMPONENT targeting lane_be,ext, got {}",
        serde_json::to_string_pretty(&v["issues"]).unwrap()
    );
    assert_eq!(hits[0]["severity"], "error");
    assert_eq!(hits[0]["fix"], "move ext left by 80px to leave a 40px gap");
    assert_eq!(
        hits[0]["command"],
        "excal layout move f.excalidraw --id ext --dx -80"
    );
}

#[test]
fn b09b_elbow_arrow_crosses_own_endpoints() {
    let dir = make_temp_dir("b09b");
    let path = dir.join("f.excalidraw");
    let src = r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "A", "x": 100, "y": 100, "width": 140, "height": 80},
    {"type": "rectangle", "id": "B", "x": 100, "y": 220, "width": 140, "height": 80},
    {
      "type": "arrow",
      "id": "arr",
      "x": 246,
      "y": 140,
      "width": 152,
      "height": 120,
      "points": [[0, 0], [-76, 0], [-76, 120], [-152, 120]],
      "startBinding": {"elementId": "A"},
      "endBinding": {"elementId": "B"}
    }
  ]
}"#;
    fs::write(&path, src).expect("write temp lint file");
    let v = lint_json_in(&dir);
    let _ = fs::remove_dir_all(&dir);
    let hits = issues_with_rule(&v, "ARROW_CROSSING_NODE");
    let pairs: Vec<_> = hits.iter().map(|issue| &issue["targets"]).collect();
    assert!(
        pairs.iter().any(|t| **t == serde_json::json!(["arr", "A"])),
        "missing targets [arr, A]: {}",
        serde_json::to_string_pretty(&v["issues"]).unwrap()
    );
    assert!(
        pairs.iter().any(|t| **t == serde_json::json!(["arr", "B"])),
        "missing targets [arr, B]: {}",
        serde_json::to_string_pretty(&v["issues"]).unwrap()
    );
    assert!(hits.iter().all(|issue| issue["severity"] == "error"));
}

#[test]
fn b12b_inverted_arrow_label_collision_uses_positive_dx() {
    let dir = make_temp_dir("b12b");
    let path = dir.join("f.excalidraw");
    let src = r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "fetcher", "x": 331, "y": 65, "width": 175, "height": 75},
    {"type": "rectangle", "id": "parser", "x": 510, "y": 65, "width": 175, "height": 75},
    {
      "type": "arrow",
      "id": "a_fp",
      "x": 512,
      "y": 102,
      "width": 8,
      "height": 0,
      "points": [[0, 0], [-8, 0]],
      "startBinding": {"elementId": "fetcher"},
      "endBinding": {"elementId": "parser"}
    },
    {
      "type": "text",
      "id": "lbl",
      "x": 469.6,
      "y": 92,
      "width": 76.8,
      "height": 20,
      "text": "raw HTML",
      "fontSize": 16,
      "containerId": "a_fp"
    }
  ]
}"#;
    fs::write(&path, src).expect("write temp lint file");
    let v = lint_json_in(&dir);
    let _ = fs::remove_dir_all(&dir);
    let hits = issues_with_rule(&v, "ARROW_LABEL_COLLISION");
    assert!(
        hits.iter()
            .any(|issue| issue["command"] == "excal layout move f.excalidraw --id parser --dx 93"),
        "expected positive --dx 93 command, got {}",
        serde_json::to_string_pretty(&v["issues"]).unwrap()
    );
}

#[test]
fn b15c_cross_swimlane_near_miss_alignment() {
    let dir = make_temp_dir("b15c");
    let path = dir.join("f.excalidraw");
    let src = r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "z1", "x": 0, "y": 0, "width": 300, "height": 300},
    {"type": "text", "id": "z1_title", "x": 8, "y": 8, "width": 40, "height": 20, "text": "Z1", "fontSize": 16, "textAlign": "left"},
    {"type": "rectangle", "id": "a", "x": 40, "y": 100, "width": 140, "height": 80},
    {"type": "rectangle", "id": "z2", "x": 400, "y": 0, "width": 300, "height": 300},
    {"type": "text", "id": "z2_title", "x": 408, "y": 8, "width": 40, "height": 20, "text": "Z2", "fontSize": 16, "textAlign": "left"},
    {"type": "rectangle", "id": "b", "x": 480, "y": 105, "width": 140, "height": 80},
    {
      "type": "arrow",
      "id": "arr",
      "x": 186,
      "y": 140,
      "width": 288,
      "height": 5,
      "points": [[0, 0], [288, 5]],
      "startBinding": {"elementId": "a"},
      "endBinding": {"elementId": "b"}
    }
  ]
}"#;
    fs::write(&path, src).expect("write temp lint file");
    let v = lint_json_in(&dir);
    let _ = fs::remove_dir_all(&dir);
    let hits: Vec<_> = issues_with_rule(&v, "NEAR_MISS_ALIGNMENT")
        .into_iter()
        .filter(|issue| issue["message"] == "a and b are misaligned by 5px (middle)")
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected NEAR_MISS_ALIGNMENT for a and b, got {}",
        serde_json::to_string_pretty(&v["issues"]).unwrap()
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
fn b21_fixture_f21_uneven_spacing_does_not_cross_nested_zone() {
    let dir = make_temp_dir("b21");
    let path = dir.join("f.excalidraw");
    fs::write(&path, fixture_f21()).expect("write F21");
    let v = lint_json_in(&dir);
    let _ = fs::remove_dir_all(&dir);

    let uneven = issues_with_rule(&v, "UNEVEN_SPACING");
    let left: Vec<_> = uneven
        .iter()
        .filter(|issue| {
            issue["targets"] == serde_json::json!(["search_ui", "q_rewrite", "blender"])
        })
        .collect();
    assert_eq!(
        left.len(),
        1,
        "expected one UNEVEN_SPACING on the left pipeline, got {}",
        serde_json::to_string_pretty(&v["issues"]).unwrap()
    );
    assert_eq!(
        left[0]["command"],
        "excal layout distribute f.excalidraw --ids search_ui,q_rewrite,blender --direction horizontal --gap 90"
    );
    assert!(
        uneven.iter().all(|issue| {
            let targets = issue["targets"].as_array().expect("targets");
            !(targets.iter().any(|id| id == "blender")
                && targets.iter().any(|id| id == "l1_ranker"))
        }),
        "no issue may group blender with l1_ranker: {}",
        serde_json::to_string_pretty(&v["issues"]).unwrap()
    );
    assert_eq!(
        uneven.len(),
        1,
        "exactly one UNEVEN_SPACING, got {uneven:?}"
    );
}
