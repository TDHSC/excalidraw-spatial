use std::fs::File;
use std::process::{Command, Stdio};

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
