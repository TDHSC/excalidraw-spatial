use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
        "excal-t36-{}-{}-{tag}.excalidraw",
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

fn has_arrow_binding(el: &serde_json::Value, arrow_id: &str) -> bool {
    el["boundElements"].as_array().is_some_and(|bound| {
        bound
            .iter()
            .any(|entry| entry["type"] == "arrow" && entry["id"] == arrow_id)
    })
}

fn assert_clean_lint(path: &str) {
    let (code, stdout, stderr) = run(&["lint", path]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert!(
        stdout.contains("errors: 0, warnings: 0, info: 0"),
        "lint must be clean, got {stdout}"
    );
}

fn seed_nodes(path: &str, nodes: &[(&str, &str, &[&str])]) {
    for (id, label, extra) in nodes {
        let mut args = vec![
            "draw",
            "node",
            path,
            "--id",
            id,
            "--label",
            label,
            "--in-place",
        ];
        args.extend_from_slice(extra);
        let (code, stdout, stderr) = run(&args);
        assert_eq!(code, 0, "seed {id} stderr={stderr:?} stdout={stdout:?}");
    }
}

#[test]
fn draw_connect_authors_straight_labeled_arrow_that_follows_layout_move() {
    let path = temp_path("straight");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);
    seed_nodes(
        path_s,
        &[
            ("client", "Client", &["--x", "100", "--y", "100"]),
            (
                "api",
                "API Gateway",
                &["--right-of", "client", "--gap", "100"],
            ),
        ],
    );

    let (code, stdout, stderr) = run(&[
        "draw",
        "connect",
        path_s,
        "--from",
        "client",
        "--to",
        "api",
        "--id",
        "a1",
        "--label",
        "HTTPS",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert_eq!(stdout, "");
    assert!(
        stderr.starts_with(
            "excal: added 1 item(s), updated 0 item(s), removed 0 item(s)\n\
             excal: lint after change: errors: 0, warnings: 0, info: 0\n"
        ),
        "stderr={stderr:?}"
    );

    let doc = load(&path);
    assert!(has_arrow_binding(element(&doc, "client"), "a1"));
    assert!(has_arrow_binding(element(&doc, "api"), "a1"));
    let arrow = element(&doc, "a1");
    assert_eq!(num(arrow, "x"), 266.0);
    assert_eq!(num(arrow, "y"), 140.0);
    assert_eq!(arrow["points"], serde_json::json!([[0, 0], [88, 0]]));
    assert_eq!(arrow["startBinding"]["elementId"], "client");
    assert_eq!(arrow["endBinding"]["elementId"], "api");
    assert_eq!(
        arrow["startBinding"]["fixedPoint"],
        serde_json::json!([1, 0.5])
    );
    assert_eq!(
        arrow["endBinding"]["fixedPoint"],
        serde_json::json!([0, 0.5])
    );
    assert_eq!(arrow["startBinding"]["mode"], "orbit");
    assert_eq!(arrow["endBinding"]["mode"], "orbit");
    assert_eq!(arrow["elbowed"], false);
    assert_eq!(arrow["roundness"], serde_json::json!({"type": 2}));
    let label = element(&doc, "a1-label");
    assert_eq!(num(label, "width"), 48.0);
    assert_eq!(num(label, "height"), 20.0);
    assert_eq!(num(label, "x"), 286.0);
    assert_eq!(num(label, "y"), 130.0);
    assert_eq!(label["text"], "HTTPS");
    assert_eq!(label["containerId"], "a1");
    assert_eq!(label["fontSize"], 16);
    assert_clean_lint(path_s);

    let (code, stdout, stderr) = run(&[
        "layout",
        "move",
        path_s,
        "--id",
        "api",
        "--dx",
        "40",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    let moved = load(&path);
    assert_eq!(num(element(&moved, "a1"), "width"), 128.0);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn draw_connect_resolves_labels_and_generates_unused_connector_ids() {
    let path = temp_path("ids");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);
    seed_nodes(
        path_s,
        &[
            ("client", "Client", &["--x", "100", "--y", "100"]),
            (
                "api",
                "API Gateway",
                &["--right-of", "client", "--gap", "100"],
            ),
        ],
    );

    let (code, stdout, stderr) = run(&[
        "draw",
        "connect",
        path_s,
        "--from",
        "Client",
        "--to",
        "API Gateway",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    let doc = load(&path);
    assert!(element(&doc, "client-api")["type"] == "arrow");
    assert_eq!(
        element(&doc, "client-api")["startBinding"]["elementId"],
        "client"
    );
    assert_eq!(
        element(&doc, "client-api")["endBinding"]["elementId"],
        "api"
    );

    let (code, stdout, stderr) = run(&[
        "draw",
        "connect",
        path_s,
        "--from",
        "Client",
        "--to",
        "API Gateway",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    let doc = load(&path);
    assert!(element(&doc, "client-api-2")["type"] == "arrow");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn draw_connect_binds_group_to_largest_shape_and_inserts_before_shared_frame() {
    let path = temp_path("group");
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/icon_group.excalidraw");
    std::fs::copy(&fixture, &path).expect("copy fixture");
    let path_s = path.to_str().expect("utf8 path");

    let (code, stdout, stderr) = run(&[
        "draw",
        "connect",
        path_s,
        "--from",
        "Ab1Cd2Ef3Gh4Ij5Kl6MnO",
        "--to",
        "Postgres",
        "--id",
        "img_pg",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    let doc = load(&path);
    let arrow = element(&doc, "img_pg");
    assert_eq!(arrow["endBinding"]["elementId"], "Hn6Bf3Jq8XcV1sMz5LwPe");
    assert_eq!(arrow["startBinding"]["elementId"], "Ab1Cd2Ef3Gh4Ij5Kl6MnO");
    assert_eq!(arrow["frameId"], "Fr8Mw2Nq5Xs1Lp4Kv7RbZ");
    assert!(has_arrow_binding(
        element(&doc, "Hn6Bf3Jq8XcV1sMz5LwPe"),
        "img_pg"
    ));
    assert!(index_of(&doc, "img_pg") < index_of(&doc, "Fr8Mw2Nq5Xs1Lp4Kv7RbZ"));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn draw_connect_spreads_parallel_straight_arrows_across_the_shared_overlap() {
    let path = temp_path("bundle");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);
    seed_nodes(
        path_s,
        &[
            (
                "psp_router",
                "PSP",
                &[
                    "--x", "60", "--y", "420", "--width", "175", "--height", "75",
                ],
            ),
            (
                "card_net",
                "Card",
                &[
                    "--x", "335", "--y", "420", "--width", "175", "--height", "75",
                ],
            ),
        ],
    );

    let (code, stdout, stderr) = run(&[
        "draw",
        "connect",
        path_s,
        "--from",
        "psp_router",
        "--to",
        "card_net",
        "--id",
        "a_req",
        "--label",
        "AUTH req",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");

    let (code, stdout, stderr) = run(&[
        "draw",
        "connect",
        path_s,
        "--from",
        "card_net",
        "--to",
        "psp_router",
        "--id",
        "a_resp",
        "--label",
        "200 OK",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert!(
        stderr.contains("excal: added 1 item(s), updated 1 item(s), removed 0 item(s)"),
        "stderr={stderr:?}"
    );

    let doc = load(&path);
    let req = element(&doc, "a_req");
    assert_eq!(
        req["startBinding"]["fixedPoint"],
        serde_json::json!([1, 0.33])
    );
    assert_eq!(
        req["endBinding"]["fixedPoint"],
        serde_json::json!([0, 0.33])
    );
    assert_eq!(num(req, "x"), 241.0);
    assert_eq!(num(req, "y"), 444.75);
    assert_eq!(req["points"], serde_json::json!([[0, 0], [88, 0]]));

    let resp = element(&doc, "a_resp");
    assert_eq!(
        resp["startBinding"]["fixedPoint"],
        serde_json::json!([0, 0.67])
    );
    assert_eq!(
        resp["endBinding"]["fixedPoint"],
        serde_json::json!([1, 0.67])
    );
    assert_eq!(num(resp, "x"), 329.0);
    assert_eq!(num(resp, "y"), 470.25);
    assert_eq!(resp["points"], serde_json::json!([[0, 0], [-88, 0]]));

    let (code, stdout, stderr) = run(&["lint", path_s]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert!(
        !stdout.contains("ARROW_LABEL_COLLISION"),
        "labels must not collide, got {stdout}"
    );

    let _ = std::fs::remove_file(&path);
}

#[test]
fn draw_connect_elbow_avoids_intervening_nodes_and_honors_explicit_sides() {
    let path = temp_path("elbow");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);
    seed_nodes(
        path_s,
        &[
            ("a", "A", &["--x", "100", "--y", "100"]),
            ("b", "B", &["--below", "a", "--gap", "60"]),
            ("c", "C", &["--right-of", "b", "--gap", "80"]),
        ],
    );

    let (code, stdout, stderr) = run(&[
        "draw",
        "connect",
        path_s,
        "--from",
        "a",
        "--to",
        "c",
        "--id",
        "ac",
        "--elbow",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    let doc = load(&path);
    let arrow = element(&doc, "ac");
    assert_eq!(arrow["elbowed"], true);
    assert_eq!(arrow["roundness"], serde_json::Value::Null);
    assert_eq!(
        arrow["points"],
        serde_json::json!([[0, 0], [34, 0], [34, 140], [68, 140]])
    );
    assert_eq!(num(arrow, "x"), 266.0);
    assert_eq!(num(arrow, "y"), 140.0);
    let abs = absolute_points(arrow);
    assert_eq!(
        abs,
        vec![
            (266.0, 140.0),
            (300.0, 140.0),
            (300.0, 280.0),
            (334.0, 280.0)
        ]
    );

    let (code, stdout, stderr) = run(&["lint", path_s]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert!(
        !stdout.contains("ARROW_CROSSING_NODE"),
        "elbow must not cross a node, got {stdout}"
    );

    let (code, stdout, stderr) = run(&[
        "draw",
        "connect",
        path_s,
        "--from",
        "a",
        "--to",
        "b",
        "--id",
        "ab_side",
        "--elbow",
        "--from-side",
        "left",
        "--to-side",
        "left",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    let doc = load(&path);
    let side = element(&doc, "ab_side");
    assert_eq!(
        side["startBinding"]["fixedPoint"],
        serde_json::json!([0, 0.5])
    );
    assert_eq!(
        side["endBinding"]["fixedPoint"],
        serde_json::json!([0, 0.5])
    );

    let _ = std::fs::remove_file(&path);
}

fn absolute_points(arrow: &serde_json::Value) -> Vec<(f64, f64)> {
    let x = num(arrow, "x");
    let y = num(arrow, "y");
    arrow["points"]
        .as_array()
        .expect("points")
        .iter()
        .map(|p| {
            (
                x + p[0].as_f64().expect("px"),
                y + p[1].as_f64().expect("py"),
            )
        })
        .collect()
}

#[test]
fn draw_node_from_edge_label_sizes_gap_and_connects_in_one_step() {
    let path = temp_path("inline");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);
    seed_nodes(
        path_s,
        &[("client", "Client", &["--x", "100", "--y", "100"])],
    );

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        path_s,
        "--id",
        "auth",
        "--label",
        "Auth",
        "--right-of",
        "client",
        "--from",
        "client",
        "--edge-label",
        "verifyJwtTokenAndSession()",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert!(
        stderr.contains("excal: added 2 item(s), updated 0 item(s), removed 0 item(s)"),
        "stderr={stderr:?}"
    );
    let doc = load(&path);
    assert_eq!(num(element(&doc, "auth"), "x"), 534.0);
    let arrow = element(&doc, "client-auth");
    assert_eq!(arrow["type"], "arrow");
    assert_eq!(arrow["startBinding"]["elementId"], "client");
    assert_eq!(arrow["endBinding"]["elementId"], "auth");
    assert_eq!(
        element(&doc, "client-auth-label")["text"],
        "verifyJwtTokenAndSession()"
    );
    assert_clean_lint(path_s);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn draw_connect_rejects_self_loops_and_text_endpoints() {
    let path = temp_path("errors");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);
    seed_nodes(
        path_s,
        &[("client", "Client", &["--x", "100", "--y", "100"])],
    );
    let (code, stdout, stderr) = run(&[
        "draw",
        "text",
        path_s,
        "--id",
        "note",
        "--text",
        "free",
        "--x",
        "400",
        "--y",
        "100",
        "--in-place",
    ]);
    assert_eq!(code, 0, "seed text stderr={stderr:?} stdout={stdout:?}");

    let (code, stdout, stderr) = run(&[
        "draw",
        "connect",
        path_s,
        "--from",
        "client",
        "--to",
        "client",
        "--in-place",
    ]);
    assert_eq!(code, 2, "stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(stdout, "");
    assert!(
        stderr.contains("excal: error: cannot connect client to itself"),
        "stderr={stderr:?}"
    );

    let (code, stdout, stderr) = run(&[
        "draw",
        "connect",
        path_s,
        "--from",
        "client",
        "--to",
        "note",
        "--in-place",
    ]);
    assert_eq!(code, 2, "stdout={stdout:?} stderr={stderr:?}");
    assert!(
        stderr.contains(
            "excal: error: cannot connect note: endpoint must be a shape, group, or container"
        ),
        "stderr={stderr:?}"
    );

    let _ = std::fs::remove_file(&path);
}
