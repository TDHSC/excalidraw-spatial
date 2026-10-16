use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

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

fn load_fixture_json(name: &str) -> serde_json::Value {
    let src = fs::read_to_string(fixture(name)).expect("read fixture");
    serde_json::from_str(&src).expect("parse fixture")
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
    std::env::temp_dir().join(format!("excal-t15-{}-{}-{tag}", std::process::id(), nanos))
}

fn cleanup(path: &Path) {
    let _ = fs::remove_file(path);
}

#[test]
fn move_auth_right_updates_coords_versions_and_lints_clean() {
    let orig = load_fixture_json("overlap");
    let (code, stdout, _stderr) = run_layout(&[
        "layout",
        "move",
        &fixture("overlap"),
        "--id",
        "auth",
        "--dx",
        "64",
    ]);
    assert_eq!(code, 0, "stdout={stdout}");
    let out = parse_json(&stdout);

    let auth = element(&out, "auth");
    approx(num(auth, "x"), 300.0);
    assert_eq!(auth["version"], 6);
    assert_eq!(auth["versionNonce"], 189302496);
    assert_eq!(auth["updated"], 1_700_000_000_000i64);

    let auth_label = element(&out, "auth-label");
    approx(num(auth_label, "x"), 359.2);
    assert_eq!(auth_label["version"], 7);

    assert_eq!(element(&out, "api"), element(&orig, "api"));
    assert_eq!(element(&out, "api-label"), element(&orig, "api-label"));

    let tmp = temp_path("overlap-moved");
    fs::write(&tmp, stdout).expect("write temp");
    let lint = Command::new(bin())
        .args(["lint", tmp.to_str().expect("utf8 path")])
        .output()
        .expect("lint");
    cleanup(&tmp);
    assert_eq!(
        lint.status.code().expect("lint code"),
        0,
        "lint stderr={}",
        String::from_utf8_lossy(&lint.stderr)
    );
}

#[test]
fn move_c_syncs_bound_near_and_dangling_arrows() {
    let (code, stdout, _stderr) = run_layout(&[
        "layout",
        "move",
        &fixture("arrows"),
        "--id",
        "c",
        "--dx",
        "65",
    ]);
    assert_eq!(code, 0, "stdout={stdout}");
    let out = parse_json(&stdout);

    approx(num(element(&out, "c"), "x"), 505.0);
    approx(num(element(&out, "c-label"), "x"), 543.8);

    let arr_label = element(&out, "arr_label");
    approx(num(arr_label, "x"), 366.0);
    approx(num(arr_label, "y"), 160.0);
    assert_eq!(arr_label["points"], serde_json::json!([[0, 0], [133, 0]]));
    approx(num(arr_label, "width"), 133.0);

    let arr_label_text = element(&out, "arr_label-label");
    approx(num(arr_label_text, "x"), 370.1);
    approx(num(arr_label_text, "y"), 150.0);

    assert_eq!(
        element(&out, "arr_cross")["points"],
        serde_json::json!([[0, 0], [353, 0]])
    );

    let dangling = element(&out, "arr_dangling");
    approx(num(dangling, "x"), 575.0);
    approx(num(dangling, "y"), 186.0);
    assert_eq!(dangling["points"], serde_json::json!([[0, 0], [-65, 74]]));
    approx(num(dangling, "width"), 65.0);
    approx(num(dangling, "height"), 74.0);
}

#[test]
fn move_b_down_updates_label_arrow_and_elbow() {
    let (code, stdout, _stderr) = run_layout(&[
        "layout",
        "move",
        &fixture("arrows"),
        "--id",
        "b",
        "--dy",
        "40",
    ]);
    assert_eq!(code, 0, "stdout={stdout}");
    let out = parse_json(&stdout);

    let arr_label = element(&out, "arr_label");
    approx(num(arr_label, "x"), 366.0);
    approx(num(arr_label, "y"), 197.0);
    assert_eq!(arr_label["points"], serde_json::json!([[0, 0], [68, -34]]));

    let arr_near = element(&out, "arr_near");
    approx(num(arr_near, "x"), 290.0);
    approx(num(arr_near, "y"), 226.0);
    assert_eq!(
        arr_near["points"],
        serde_json::json!([[0, 0], [0, 114], [142, 114]])
    );
}

#[test]
fn move_d_down_repairs_only_adjacent_elbow_segment() {
    let (code, stdout, _stderr) = run_layout(&[
        "layout",
        "move",
        &fixture("arrows"),
        "--id",
        "d",
        "--dy",
        "30",
    ]);
    assert_eq!(code, 0, "stdout={stdout}");
    let out = parse_json(&stdout);
    assert_eq!(
        element(&out, "arr_near")["points"],
        serde_json::json!([[0, 0], [0, 184], [142, 184]])
    );
}

#[test]
fn move_container_carries_descendants_and_syncs_connector() {
    let (code, stdout, _stderr) = run_layout(&[
        "layout",
        "move",
        &fixture("swimlanes"),
        "--id",
        "lane_fe",
        "--dx",
        "-20",
    ]);
    assert_eq!(code, 0, "stdout={stdout}");
    let out = parse_json(&stdout);

    approx(num(element(&out, "lane_fe"), "x"), -20.0);
    approx(num(element(&out, "lane_fe_title"), "x"), -4.0);
    approx(num(element(&out, "web"), "x"), 20.0);
    approx(num(element(&out, "mobile"), "x"), 200.0);

    let mob_api = element(&out, "mob_api");
    approx(num(mob_api, "x"), 346.0);
    assert_eq!(mob_api["points"], serde_json::json!([[0, 0], [128, 0]]));
}

#[test]
fn move_frame_translates_group_arrow_as_a_whole() {
    let (code, stdout, _stderr) = run_layout(&[
        "layout",
        "move",
        &fixture("icon_group"),
        "--id",
        "Fr8Mw2Nq5Xs1Lp4Kv7RbZ",
        "--dx",
        "50",
    ]);
    assert_eq!(code, 0, "stdout={stdout}");
    let out = parse_json(&stdout);

    let arrow = element(&out, "Lg7Hs4Kd1Fp8Wq5Er2TyU");
    approx(num(arrow, "x"), 344.0);
    assert_eq!(arrow["points"], serde_json::json!([[0, 0], [-148, 0]]));
    approx(num(element(&out, "Hn6Bf3Jq8XcV1sMz5LwPe"), "x"), 110.0);
}

#[test]
fn move_postgres_group_uses_binding_element_center() {
    let (code, stdout, _stderr) = run_layout(&[
        "layout",
        "move",
        &fixture("icon_group"),
        "--id",
        "Postgres",
        "--dy",
        "20",
    ]);
    assert_eq!(code, 0, "stdout={stdout}");
    let out = parse_json(&stdout);

    approx(num(element(&out, "Hn6Bf3Jq8XcV1sMz5LwPe"), "y"), 112.0);
    approx(num(element(&out, "Qw5Er8Ty1Ui4Op7As0DfG"), "y"), 204.0);

    let arrow = element(&out, "Lg7Hs4Kd1Fp8Wq5Er2TyU");
    approx(num(arrow, "x"), 294.0);
    approx(num(arrow, "y"), 135.22);
    assert_eq!(arrow["points"], serde_json::json!([[0, 0], [-148, 8.99]]));
}

#[test]
fn move_connector_itself_skips_arrow_sync() {
    let orig = load_fixture_json("arrows");
    let (code, stdout, _stderr) = run_layout(&[
        "layout",
        "move",
        &fixture("arrows"),
        "--id",
        "arr_dangling",
        "--dx",
        "10",
    ]);
    assert_eq!(code, 0, "stdout={stdout}");
    let out = parse_json(&stdout);

    approx(num(element(&out, "arr_dangling"), "x"), 520.0);
    let orig_els = orig["elements"].as_array().expect("orig elements");
    let out_els = out["elements"].as_array().expect("out elements");
    assert_eq!(orig_els.len(), out_els.len());
    for (orig_el, out_el) in orig_els.iter().zip(out_els) {
        if orig_el["id"] == "arr_dangling" {
            continue;
        }
        assert_eq!(out_el, orig_el, "element {} changed", orig_el["id"]);
    }
}

#[test]
fn zero_delta_is_byte_identical_and_prints_nothing_to_change() {
    let orig = fs::read_to_string(fixture("overlap")).expect("read fixture");
    let (code, stdout, stderr) =
        run_layout(&["layout", "move", &fixture("overlap"), "--id", "auth"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert_eq!(stdout, orig);
    assert_eq!(stderr, "excal: nothing to change\n");
}

#[test]
fn output_destinations() {
    let (code, stdout, _stderr) = run_layout(&[
        "layout",
        "move",
        &fixture("overlap"),
        "--id",
        "auth",
        "--dx",
        "64",
    ]);
    assert_eq!(code, 0);
    let parsed = parse_json(&stdout);
    approx(num(element(&parsed, "auth"), "x"), 300.0);

    let out_file = temp_path("out.json");
    let (code, stdout, _stderr) = run_layout(&[
        "layout",
        "move",
        &fixture("overlap"),
        "--id",
        "auth",
        "--dx",
        "64",
        "-o",
        out_file.to_str().expect("utf8"),
    ]);
    assert_eq!(code, 0);
    assert_eq!(stdout, "");
    let written = fs::read_to_string(&out_file).expect("read -o file");
    cleanup(&out_file);
    let parsed = parse_json(&written);
    approx(num(element(&parsed, "auth"), "x"), 300.0);

    let copy = temp_path("inplace.excalidraw");
    fs::copy(fixture("overlap"), &copy).expect("copy fixture");
    let (code, stdout, _stderr) = run_layout(&[
        "layout",
        "move",
        copy.to_str().expect("utf8"),
        "--id",
        "auth",
        "--dx",
        "64",
        "--in-place",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stdout, "");
    let written = fs::read_to_string(&copy).expect("read in-place");
    cleanup(&copy);
    let parsed = parse_json(&written);
    approx(num(element(&parsed, "auth"), "x"), 300.0);

    let output = Command::new(bin())
        .args(["layout", "move", "-", "--id", "auth", "--in-place"])
        .env("EXCAL_FIXED_NOW_MS", FIXED_NOW_MS)
        .stdin(Stdio::null())
        .output()
        .expect("stdin in-place");
    assert_eq!(output.status.code().expect("code"), 2);
    let stderr = String::from_utf8(output.stderr).expect("stderr utf-8");
    assert!(stderr.starts_with("excal: error:"), "stderr={stderr:?}");
    assert!(
        stderr.contains("in-place") && stderr.to_lowercase().contains("stdin"),
        "expected early in-place/stdin error, got {stderr:?}"
    );
}

#[test]
fn move_auth_stderr_is_exactly_two_summary_lines() {
    let (code, _stdout, stderr) = run_layout(&[
        "layout",
        "move",
        &fixture("overlap"),
        "--id",
        "auth",
        "--dx",
        "64",
    ]);
    assert_eq!(code, 0);
    assert_eq!(
        stderr,
        "excal: moved 1 item(s), resized 0 item(s), updated 0 connector(s)\n\
         excal: lint after change: errors: 0, warnings: 0, info: 0\n"
    );
}

#[test]
fn unknown_id_is_exit_2_with_resolve_message() {
    let (code, _stdout, stderr) = run_layout(&[
        "layout",
        "move",
        &fixture("overlap"),
        "--id",
        "zzz",
        "--dx",
        "1",
    ]);
    assert_eq!(code, 2);
    assert_eq!(stderr.trim_end(), "excal: error: unknown id \"zzz\"");
}
