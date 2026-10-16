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
