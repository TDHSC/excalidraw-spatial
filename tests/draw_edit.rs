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
        "excal-t38-{}-{}-{tag}.excalidraw",
        std::process::id(),
        nanos
    ))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{name}.excalidraw"))
}

fn copy_fixture(name: &str, tag: &str) -> PathBuf {
    let dest = temp_path(tag);
    std::fs::copy(fixture(name), &dest).expect("copy fixture");
    dest
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

fn has_id(doc: &serde_json::Value, id: &str) -> bool {
    doc["elements"]
        .as_array()
        .expect("elements")
        .iter()
        .any(|el| el["id"] == id)
}

fn bound_ids(el: &serde_json::Value) -> Vec<String> {
    el["boundElements"]
        .as_array()
        .map(|bound| {
            bound
                .iter()
                .filter_map(|entry| entry["id"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn assert_clean_lint(path: &str) {
    let (code, stdout, stderr) = run(&["lint", path]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert!(
        stdout.contains("errors: 0, warnings: 0, info: 0"),
        "lint must be clean, got {stdout}"
    );
}

#[test]
fn draw_label_widens_shape_and_reclips_arrows() {
    let path = copy_fixture("basic_flow", "widen");
    let path_s = path.to_str().expect("utf8 path");
    let (code, _stdout, stderr) = run(&[
        "draw",
        "label",
        path_s,
        "--id",
        "api",
        "--text",
        "Edge API Gateway",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?}");
    assert!(
        stderr.contains("excal: added 0 item(s), updated 3 item(s), removed 0 item(s)"),
        "stderr={stderr:?}"
    );

    let doc = load(&path);
    let api = element(&doc, "api");
    assert_eq!(num(api, "x"), 320.0);
    assert_eq!(num(api, "y"), 100.0);
    assert_eq!(num(api, "width"), 240.0);
    assert_eq!(num(api, "height"), 80.0);

    let label = element(&doc, "api-label");
    assert_eq!(label["text"], "Edge API Gateway");
    assert_eq!(label["originalText"], "Edge API Gateway");
    assert_eq!(num(label, "x"), 344.0);
    assert_eq!(num(label, "y"), 127.5);
    assert_eq!(num(label, "width"), 192.0);
    assert_eq!(num(label, "height"), 25.0);

    let a1 = element(&doc, "a1");
    assert_eq!(num(a1, "x"), 266.0);
    assert_eq!(a1["points"], serde_json::json!([[0, 0], [48, 0]]));
    let a2 = element(&doc, "a2");
    assert_eq!(num(a2, "x"), 566.0);
    assert_eq!(a2["points"], serde_json::json!([[0, 0], [48, 0]]));

    assert_clean_lint(path_s);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn draw_label_shorter_text_recenters_without_resizing() {
    let path = copy_fixture("basic_flow", "short");
    let path_s = path.to_str().expect("utf8 path");
    let (code, _stdout, stderr) = run(&[
        "draw",
        "label",
        path_s,
        "--id",
        "api",
        "--text",
        "GW",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?}");
    assert!(
        stderr.contains("excal: added 0 item(s), updated 1 item(s), removed 0 item(s)"),
        "stderr={stderr:?}"
    );

    let doc = load(&path);
    let api = element(&doc, "api");
    assert_eq!(
        (
            num(api, "x"),
            num(api, "y"),
            num(api, "width"),
            num(api, "height")
        ),
        (360.0, 100.0, 160.0, 80.0)
    );
    let label = element(&doc, "api-label");
    assert_eq!(label["text"], "GW");
    assert_eq!(num(label, "width"), 24.0);
    assert_eq!(num(label, "height"), 25.0);
    assert_eq!(num(label, "x"), 428.0);
    assert_eq!(num(label, "y"), 127.5);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn draw_label_inserts_bound_label_on_unlabeled_shape() {
    let path = temp_path("unlabeled");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);
    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        path_s,
        "--id",
        "b1",
        "--x",
        "100",
        "--y",
        "100",
        "--in-place",
    ]);
    assert_eq!(code, 0, "seed stderr={stderr:?} stdout={stdout:?}");

    let (code, _stdout, stderr) = run(&[
        "draw",
        "label",
        path_s,
        "--id",
        "b1",
        "--text",
        "Worker",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?}");

    let doc = load(&path);
    assert_eq!(index_of(&doc, "b1-label"), index_of(&doc, "b1") + 1);
    let label = element(&doc, "b1-label");
    assert_eq!(label["text"], "Worker");
    assert_eq!(label["containerId"], "b1");
    assert_eq!(num(label, "width"), 72.0);
    assert_eq!(num(label, "height"), 25.0);
    assert_eq!(num(label, "x"), 144.0);
    assert_eq!(num(label, "y"), 127.5);
    assert!(bound_ids(element(&doc, "b1")).contains(&"b1-label".to_string()));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn draw_label_updates_arrow_zone_frame_and_rejects_group() {
    let arrows = copy_fixture("basic_flow", "arrow-label");
    let arrows_s = arrows.to_str().expect("utf8 path");
    let (code, _stdout, stderr) = run(&[
        "draw",
        "label",
        arrows_s,
        "--id",
        "a1",
        "--text",
        "mTLS",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?}");
    let doc = load(&arrows);
    let label = element(&doc, "a1-label");
    assert_eq!(label["text"], "mTLS");
    assert_eq!(num(label, "width"), 38.4);
    assert_eq!(num(label, "height"), 20.0);
    assert_eq!(num(label, "x"), 290.8);
    assert_eq!(num(label, "y"), 130.0);

    let lanes = copy_fixture("swimlanes", "zone-label");
    let lanes_s = lanes.to_str().expect("utf8 path");
    let (code, _stdout, stderr) = run(&[
        "draw",
        "label",
        lanes_s,
        "--id",
        "mob_api",
        "--text",
        "REST",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?}");
    let doc = load(&lanes);
    let label = element(&doc, "mob_api-label");
    assert_eq!(label["text"], "REST");
    assert_eq!(label["containerId"], "mob_api");
    assert!(bound_ids(element(&doc, "mob_api")).contains(&"mob_api-label".to_string()));
    assert_eq!(
        index_of(&doc, "mob_api-label"),
        index_of(&doc, "mob_api") + 1
    );

    let (code, _stdout, stderr) = run(&[
        "draw",
        "label",
        lanes_s,
        "--id",
        "lane_fe",
        "--text",
        "Client Tier",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?}");
    let doc = load(&lanes);
    let title = element(&doc, "lane_fe_title");
    assert_eq!(title["text"], "Client Tier");
    assert_eq!(title["originalText"], "Client Tier");

    let icons = copy_fixture("icon_group", "frame-label");
    let icons_s = icons.to_str().expect("utf8 path");
    let (code, _stdout, stderr) = run(&[
        "draw",
        "label",
        icons_s,
        "--id",
        "Storage",
        "--text",
        "Data Frame",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?}");
    let doc = load(&icons);
    assert_eq!(element(&doc, "Fr8Mw2Nq5Xs1Lp4Kv7RbZ")["name"], "Data Frame");

    let (code, _stdout, stderr) = run(&[
        "draw",
        "label",
        icons_s,
        "--id",
        "Postgres",
        "--text",
        "DB",
        "--in-place",
    ]);
    assert_eq!(code, 2, "stderr={stderr:?}");
    assert!(
        stderr.contains("excal: error: cannot relabel group component group:kq3Xv9TzL2mPw8RbN5cYd"),
        "stderr={stderr:?}"
    );

    let _ = std::fs::remove_file(&arrows);
    let _ = std::fs::remove_file(&lanes);
    let _ = std::fs::remove_file(&icons);
}

#[test]
fn draw_remove_cascades_arrows_unless_kept() {
    let path = copy_fixture("basic_flow", "cascade");
    let path_s = path.to_str().expect("utf8 path");
    let (code, _stdout, stderr) = run(&["draw", "remove", path_s, "--id", "api", "--in-place"]);
    assert_eq!(code, 0, "stderr={stderr:?}");
    assert!(stderr.contains("removed 3 item(s)"), "stderr={stderr:?}");
    let doc = load(&path);
    for id in ["api", "api-label", "a1", "a1-label", "a2", "a2-label"] {
        assert!(!has_id(&doc, id), "{id} should be removed");
    }
    assert!(!bound_ids(element(&doc, "client"))
        .iter()
        .any(|id| id == "a1"));
    assert!(!bound_ids(element(&doc, "svc")).iter().any(|id| id == "a2"));

    let kept = copy_fixture("basic_flow", "keep");
    let kept_s = kept.to_str().expect("utf8 path");
    let (code, _stdout, stderr) = run(&[
        "draw",
        "remove",
        kept_s,
        "--id",
        "api",
        "--keep-arrows",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?}");
    assert!(
        stderr.contains("updated 2 item(s), removed 1 item(s)"),
        "stderr={stderr:?}"
    );
    assert_eq!(
        stderr.matches("WARN  DANGLING_ARROW").count(),
        2,
        "stderr={stderr:?}"
    );
    let doc = load(&kept);
    assert!(!has_id(&doc, "api"));
    assert!(!has_id(&doc, "api-label"));
    assert!(element(&doc, "a1")["endBinding"].is_null());
    assert!(element(&doc, "a2")["startBinding"].is_null());

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&kept);
}

#[test]
fn draw_remove_zone_frame_and_connector_keep_unrelated_children() {
    let lanes = copy_fixture("swimlanes", "rm-zone");
    let lanes_s = lanes.to_str().expect("utf8 path");
    let (code, _stdout, stderr) =
        run(&["draw", "remove", lanes_s, "--id", "lane_fe", "--in-place"]);
    assert_eq!(code, 0, "stderr={stderr:?}");
    assert!(stderr.contains("removed 1 item(s)"), "stderr={stderr:?}");
    let doc = load(&lanes);
    assert!(!has_id(&doc, "lane_fe"));
    assert!(!has_id(&doc, "lane_fe_title"));
    assert!(has_id(&doc, "web"));
    assert!(has_id(&doc, "mobile"));
    let parsed = Document::load(lanes_s).expect("reload");
    let scene = Scene::build(&parsed);
    assert!(scene.component("web").unwrap().parent.is_none());
    assert!(scene.component("mobile").unwrap().parent.is_none());

    let icons = copy_fixture("icon_group", "rm-frame");
    let icons_s = icons.to_str().expect("utf8 path");
    let (code, _stdout, stderr) =
        run(&["draw", "remove", icons_s, "--id", "Storage", "--in-place"]);
    assert_eq!(code, 0, "stderr={stderr:?}");
    let doc = load(&icons);
    assert!(!has_id(&doc, "Fr8Mw2Nq5Xs1Lp4Kv7RbZ"));
    for el in doc["elements"].as_array().expect("elements") {
        assert!(el["frameId"].is_null(), "{} still has frameId", el["id"]);
    }

    let flow = copy_fixture("basic_flow", "rm-arrow");
    let flow_s = flow.to_str().expect("utf8 path");
    let (code, _stdout, stderr) = run(&["draw", "remove", flow_s, "--id", "a1", "--in-place"]);
    assert_eq!(code, 0, "stderr={stderr:?}");
    let doc = load(&flow);
    assert!(!has_id(&doc, "a1"));
    assert!(!has_id(&doc, "a1-label"));
    assert!(!bound_ids(element(&doc, "client"))
        .iter()
        .any(|id| id == "a1"));
    assert!(!bound_ids(element(&doc, "api")).iter().any(|id| id == "a1"));

    let _ = std::fs::remove_file(&lanes);
    let _ = std::fs::remove_file(&icons);
    let _ = std::fs::remove_file(&flow);
}
