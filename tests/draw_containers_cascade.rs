use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use excalidraw_spatial::scene_graph::Scene;
use excalidraw_spatial::schema::Document;

const FIXED_NOW_MS: &str = "1700000000000";

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_excal")
}

fn temp_path(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "excal-t35-{}-{}-{tag}.excalidraw",
        std::process::id(),
        nanos
    ))
}

fn run(args: &[&str]) -> (i32, String, String) {
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

fn element<'a>(doc: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    doc["elements"]
        .as_array()
        .expect("elements")
        .iter()
        .find(|el| el["id"] == id)
        .unwrap_or_else(|| panic!("missing element {id}"))
}

fn num(el: &serde_json::Value, key: &str) -> f64 {
    el[key]
        .as_f64()
        .unwrap_or_else(|| panic!("{key} not a number: {el}"))
}

fn load(path: &PathBuf) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("written")).expect("json")
}

fn scene_of(path: &PathBuf) -> Scene {
    let src = std::fs::read_to_string(path).expect("written");
    let doc = Document::parse(&src).expect("parse");
    Scene::build(&doc)
}

fn draw_node(path: &str, args: &[&str]) {
    let mut cmd = vec!["draw", "node", path];
    cmd.extend_from_slice(args);
    cmd.push("--in-place");
    let (code, stdout, stderr) = run(&cmd);
    assert_eq!(code, 0, "args={args:?} stderr={stderr:?} stdout={stdout:?}");
}

/// Nested `--in` expansion must resize both the inner zone and its outer parent.
/// This file is separate so the cascade is the only update counted in `updated`.
#[test]
fn draw_node_in_nested_zone_expands_inner_and_outer() {
    let path = temp_path("cascade");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);

    draw_node(
        path_s,
        &[
            "--id", "gw", "--label", "Gateway", "--x", "100", "--y", "100",
        ],
    );
    draw_node(
        path_s,
        &["--id", "auth", "--label", "Auth", "--right-of", "gw"],
    );
    draw_node(
        path_s,
        &[
            "--id", "audit", "--label", "Audit", "--x", "100", "--y", "280",
        ],
    );

    let (code, stdout, stderr) = run(&[
        "draw",
        "zone",
        path_s,
        "--id",
        "z_inner",
        "--title",
        "Inner",
        "--children",
        "gw,auth",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");

    let (code, stdout, stderr) = run(&[
        "draw",
        "zone",
        path_s,
        "--id",
        "z_outer",
        "--title",
        "Outer",
        "--children",
        "z_inner,audit",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");

    let before = load(&path);
    let inner_before = (
        num(element(&before, "z_inner"), "width"),
        num(element(&before, "z_inner"), "height"),
    );
    let outer_before = (
        num(element(&before, "z_outer"), "width"),
        num(element(&before, "z_outer"), "height"),
    );

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        path_s,
        "--id",
        "extra",
        "--label",
        "Extra",
        "--in",
        "z_inner",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert_eq!(stdout, "");
    assert!(
        stderr.contains("added 1 item(s), updated 2 item(s)"),
        "both ancestor zones must resize, got {stderr:?}"
    );

    let after = load(&path);
    let inner = element(&after, "z_inner");
    let outer = element(&after, "z_outer");
    assert!(
        num(inner, "width") > inner_before.0 || num(inner, "height") > inner_before.1,
        "z_inner must grow, before={inner_before:?} after=({}, {})",
        num(inner, "width"),
        num(inner, "height")
    );
    assert!(
        num(outer, "width") > outer_before.0 || num(outer, "height") > outer_before.1,
        "z_outer must grow, before={outer_before:?} after=({}, {})",
        num(outer, "width"),
        num(outer, "height")
    );

    let scene = scene_of(&path);
    assert_eq!(
        scene.container("z_inner").expect("inner").parent.as_deref(),
        Some("z_outer")
    );
    assert_eq!(
        scene.component("extra").expect("extra").parent.as_deref(),
        Some("z_inner")
    );

    let (code, stdout, stderr) = run(&["lint", path_s]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert!(
        stdout.contains("errors: 0, warnings: 0, info: 0"),
        "lint must be clean, got {stdout}"
    );

    let _ = std::fs::remove_file(&path);
}
