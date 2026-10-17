use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use excalidraw_spatial::scene_graph::{ContainerKind, Scene};
use excalidraw_spatial::schema::Document;
use excalidraw_spatial::spatial;

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

fn index_of(doc: &serde_json::Value, id: &str) -> usize {
    doc["elements"]
        .as_array()
        .expect("elements")
        .iter()
        .position(|el| el["id"] == id)
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

fn assert_clean_lint(path: &str) {
    let (code, stdout, stderr) = run(&["lint", path]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert!(
        stdout.contains("errors: 0, warnings: 0, info: 0"),
        "lint must be clean, got {stdout}"
    );
}

fn draw_node(path: &str, args: &[&str]) {
    let mut cmd = vec!["draw", "node", path];
    cmd.extend_from_slice(args);
    cmd.push("--in-place");
    let (code, stdout, stderr) = run(&cmd);
    assert_eq!(code, 0, "args={args:?} stderr={stderr:?} stdout={stdout:?}");
    assert_eq!(stdout, "");
}

#[test]
fn draw_zone_children_wraps_siblings_behind_them_with_top_title() {
    let path = temp_path("lane-be");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);

    draw_node(
        path_s,
        &[
            "--id", "api", "--x", "100", "--y", "100", "--width", "160", "--height", "80",
        ],
    );
    draw_node(
        path_s,
        &[
            "--id",
            "db",
            "--right-of",
            "api",
            "--width",
            "160",
            "--height",
            "80",
        ],
    );

    let (code, stdout, stderr) = run(&[
        "draw",
        "zone",
        path_s,
        "--id",
        "lane_be",
        "--title",
        "Backend",
        "--children",
        "api,db",
        "--bg",
        "#ebfbee",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert_eq!(stdout, "");
    assert!(
        stderr.contains("added 1 item(s), updated 0 item(s)"),
        "wrapping existing siblings must not count as updates, got {stderr:?}"
    );

    let doc = load(&path);
    let zone = element(&doc, "lane_be");
    assert_eq!(num(zone, "x"), 60.0);
    assert_eq!(num(zone, "y"), 25.0);
    assert_eq!(num(zone, "width"), 460.0);
    assert_eq!(num(zone, "height"), 195.0);
    assert_eq!(zone["type"], "rectangle");
    assert_eq!(zone["roundness"], serde_json::Value::Null);
    assert_eq!(zone["backgroundColor"], "#ebfbee");
    assert_eq!(
        zone["boundElements"],
        serde_json::json!([{"type": "text", "id": "lane_be-title"}])
    );

    let title = element(&doc, "lane_be-title");
    assert_eq!(num(title, "width"), 84.0);
    assert_eq!(num(title, "height"), 25.0);
    assert_eq!(num(title, "x"), 65.0);
    assert_eq!(num(title, "y"), 30.0);
    assert_eq!(title["text"], "Backend");
    assert_eq!(title["textAlign"], "left");
    assert_eq!(title["verticalAlign"], "top");
    assert_eq!(title["containerId"], "lane_be");
    assert!(index_of(&doc, "lane_be") < index_of(&doc, "api"));
    assert_eq!(
        index_of(&doc, "lane_be-title"),
        index_of(&doc, "lane_be") + 1
    );

    let scene = scene_of(&path);
    let lane = scene.container("lane_be").expect("lane_be is a container");
    assert_eq!(lane.kind, ContainerKind::Zone);
    assert_eq!(lane.name.as_deref(), Some("Backend"));
    let kids: Vec<String> = spatial::children(&scene, "lane_be")
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(kids, vec!["api".to_string(), "db".to_string()]);
    assert_clean_lint(path_s);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn draw_zone_single_child_fits_and_large_child_scales_past_area_guard() {
    let small = temp_path("lane-db");
    let small_s = small.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&small);
    draw_node(
        small_s,
        &[
            "--id", "pg", "--x", "100", "--y", "100", "--width", "160", "--height", "80",
        ],
    );
    let (code, stdout, stderr) = run(&[
        "draw",
        "zone",
        small_s,
        "--id",
        "lane_db",
        "--title",
        "Data Layer",
        "--children",
        "pg",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    let doc = load(&small);
    let zone = element(&doc, "lane_db");
    assert_eq!(num(zone, "x"), 60.0);
    assert_eq!(num(zone, "y"), 25.0);
    assert_eq!(num(zone, "width"), 240.0);
    assert_eq!(num(zone, "height"), 195.0);
    let scene = scene_of(&small);
    assert_eq!(
        scene.container("lane_db").expect("zone").kind,
        ContainerKind::Zone
    );
    assert_clean_lint(small_s);

    let large = temp_path("z-big");
    let large_s = large.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&large);
    draw_node(
        large_s,
        &[
            "--id", "big", "--x", "100", "--y", "100", "--width", "400", "--height", "300",
        ],
    );
    let (code, stdout, stderr) = run(&[
        "draw",
        "zone",
        large_s,
        "--id",
        "z_big",
        "--title",
        " Core ",
        "--children",
        "big",
        "--padding",
        "40",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    let doc = load(&large);
    let zone = element(&doc, "z_big");
    let big = element(&doc, "big");
    let zone_area = num(zone, "width") * num(zone, "height");
    let big_area = num(big, "width") * num(big, "height");
    assert!(
        big_area <= 0.60 * zone_area,
        "area guard must keep big ({big_area}) <= 0.60 * z_big ({zone_area})"
    );
    // Unscaled tight rect is 480x415. The guard scales that box around its center.
    let center_x = 60.0 + 480.0 / 2.0;
    let center_y = 25.0 + 415.0 / 2.0;
    assert!(
        (num(zone, "x") + num(zone, "width") / 2.0 - center_x).abs() < 1.0,
        "z_big must stay centered on the unscaled box, got {:?}",
        zone
    );
    assert!(
        (num(zone, "y") + num(zone, "height") / 2.0 - center_y).abs() < 1.0,
        "z_big must stay centered on the unscaled box, got {:?}",
        zone
    );
    let scene = scene_of(&large);
    assert_eq!(
        scene.container("z_big").expect("zone").kind,
        ContainerKind::Zone
    );
    assert_clean_lint(large_s);

    let _ = std::fs::remove_file(&small);
    let _ = std::fs::remove_file(&large);
}

#[test]
fn draw_node_in_empty_zone_places_below_title_and_expands() {
    let path = temp_path("ingress");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);

    let (code, stdout, stderr) = run(&[
        "draw",
        "zone",
        path_s,
        "--id",
        "z1",
        "--title",
        "Ingress",
        "--x",
        "60",
        "--y",
        "40",
        "--width",
        "260",
        "--height",
        "180",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert_eq!(stdout, "");

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        path_s,
        "--id",
        "gw",
        "--label",
        "Gateway",
        "--in",
        "z1",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert!(
        stderr.contains("added 1 item(s), updated 1 item(s)"),
        "first child must expand the empty zone, got {stderr:?}"
    );
    let doc = load(&path);
    let gw = element(&doc, "gw");
    assert_eq!(num(gw, "x"), 100.0);
    assert_eq!(num(gw, "y"), 115.0);
    assert_eq!(num(gw, "width"), 160.0);
    assert_eq!(num(gw, "height"), 80.0);
    let z1 = element(&doc, "z1");
    assert_eq!(num(z1, "height"), 195.0);
    assert!(index_of(&doc, "gw") > index_of(&doc, "z1"));

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        path_s,
        "--id",
        "auth",
        "--label",
        "Auth Service",
        "--right-of",
        "gw",
        "--in",
        "z1",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    let doc = load(&path);
    let auth = element(&doc, "auth");
    assert_eq!(num(auth, "x"), 320.0);
    assert_eq!(num(auth, "y"), 115.0);
    assert_eq!(num(auth, "width"), 192.0);
    assert_eq!(num(auth, "height"), 80.0);
    let z1 = element(&doc, "z1");
    assert_eq!(num(z1, "x"), 60.0);
    assert_eq!(num(z1, "y"), 40.0);
    assert_eq!(num(z1, "width"), 492.0);
    assert_eq!(num(z1, "height"), 195.0);

    let scene = scene_of(&path);
    assert_eq!(
        scene.component("gw").expect("gw").parent.as_deref(),
        Some("z1")
    );
    assert_eq!(
        scene.component("auth").expect("auth").parent.as_deref(),
        Some("z1")
    );
    assert_clean_lint(path_s);

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        path_s,
        "--id",
        "rate",
        "--label",
        "Rate Limiter",
        "--in",
        "z1",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    let doc = load(&path);
    let rate = element(&doc, "rate");
    // Auth Service is 192 wide at x=320, so right-of with gap 60 is x=572.
    // "Rate Limiter" is 144 wide -> auto width 192, same height, center-aligned.
    assert_eq!(num(rate, "x"), 572.0);
    assert_eq!(num(rate, "y"), 115.0);
    assert_eq!(num(rate, "width"), 192.0);
    assert_eq!(num(rate, "height"), 80.0);
    let scene = scene_of(&path);
    assert_eq!(
        scene.component("rate").expect("rate").parent.as_deref(),
        Some("z1")
    );
    assert_clean_lint(path_s);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn draw_frame_children_tag_members_and_node_in_frame_inserts_before_frame() {
    let path = temp_path("frame");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);

    draw_node(
        path_s,
        &["--id", "a", "--label", "A", "--x", "100", "--y", "100"],
    );
    draw_node(path_s, &["--id", "b", "--label", "B", "--right-of", "a"]);

    let (code, stdout, stderr) = run(&[
        "draw",
        "frame",
        path_s,
        "--id",
        "f1",
        "--name",
        "Storage",
        "--children",
        "a,b",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert_eq!(stdout, "");

    let doc = load(&path);
    for id in ["a", "a-label", "b", "b-label"] {
        assert_eq!(element(&doc, id)["frameId"], "f1", "{id} frameId");
    }
    let frame_at = index_of(&doc, "f1");
    let member_max = ["a", "a-label", "b", "b-label"]
        .into_iter()
        .map(|id| index_of(&doc, id))
        .max()
        .expect("members");
    assert!(frame_at > member_max, "frame must follow its children");
    assert_eq!(element(&doc, "f1")["type"], "frame");
    assert_eq!(element(&doc, "f1")["name"], "Storage");

    let scene = scene_of(&path);
    let frame = scene.container("f1").expect("frame");
    assert_eq!(frame.kind, ContainerKind::Frame);
    assert_eq!(frame.name.as_deref(), Some("Storage"));
    let kids: Vec<String> = spatial::children(&scene, "f1")
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(kids, vec!["a".to_string(), "b".to_string()]);
    assert_clean_lint(path_s);

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        path_s,
        "--id",
        "c",
        "--label",
        "S3",
        "--right-of",
        "b",
        "--in",
        "f1",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    let doc = load(&path);
    assert_eq!(element(&doc, "c")["frameId"], "f1");
    assert_eq!(element(&doc, "c-label")["frameId"], "f1");
    assert!(index_of(&doc, "c") < index_of(&doc, "f1"));
    assert_eq!(index_of(&doc, "c-label"), index_of(&doc, "c") + 1);
    let scene = scene_of(&path);
    assert_eq!(
        scene.component("c").expect("c").parent.as_deref(),
        Some("f1")
    );
    assert_clean_lint(path_s);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn draw_zone_and_node_reject_bad_container_preconditions() {
    let path = temp_path("preconditions");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);

    draw_node(path_s, &["--id", "client", "--label", "Client"]);
    let (code, stdout, stderr) = run(&[
        "draw",
        "zone",
        path_s,
        "--id",
        "z1",
        "--title",
        "Ingress",
        "--x",
        "400",
        "--y",
        "40",
        "--width",
        "260",
        "--height",
        "180",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    draw_node(
        path_s,
        &["--id", "inside", "--label", "Inside", "--in", "z1"],
    );
    draw_node(
        path_s,
        &[
            "--id", "outside", "--label", "Outside", "--x", "20", "--y", "400",
        ],
    );

    let before = std::fs::read_to_string(&path).expect("seed");
    let (code, stdout, stderr) = run(&[
        "draw",
        "zone",
        path_s,
        "--id",
        "bad",
        "--title",
        "Mixed",
        "--children",
        "inside,outside",
        "--in-place",
    ]);
    assert_eq!(code, 2, "stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(stdout, "");
    assert_eq!(
        stderr,
        "excal: error: children must share the same parent container\n"
    );
    assert_eq!(std::fs::read_to_string(&path).expect("unchanged"), before);

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        path_s,
        "--id",
        "nope",
        "--label",
        "Nope",
        "--in",
        "client",
        "--in-place",
    ]);
    assert_eq!(code, 2, "stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(stdout, "");
    assert_eq!(stderr, "excal: error: client is not a container\n");
    assert_eq!(std::fs::read_to_string(&path).expect("unchanged"), before);

    let _ = std::fs::remove_file(&path);
}
