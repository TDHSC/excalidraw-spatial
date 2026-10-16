use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

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
