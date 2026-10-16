use std::io::Write;
use std::process::{Command, Stdio};

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

fn lint_json(doc_json: &str) -> serde_json::Value {
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
    let output = child.wait_with_output().expect("lint");
    assert_eq!(
        output.status.code().expect("lint code"),
        0,
        "lint stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    parse_json(&String::from_utf8(output.stdout).expect("lint stdout utf-8"))
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
