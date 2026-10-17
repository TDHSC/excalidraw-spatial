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
        "excal-t37-{}-{}-{tag}.excalidraw",
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

fn assert_report(stderr: &str, added: usize, updated: usize, removed: usize) {
    assert!(
        stderr.contains(&format!(
            "excal: added {added} item(s), updated {updated} item(s), removed {removed} item(s)"
        )),
        "stderr={stderr:?}"
    );
}

fn lint_rules(path: &str) -> Vec<String> {
    let (code, stdout, stderr) = run(&["lint", path, "--format", "json"]);
    assert!(
        code == 0 || code == 1,
        "stderr={stderr:?} stdout={stdout:?}"
    );
    let doc: serde_json::Value = serde_json::from_str(&stdout).expect("lint json");
    doc["issues"]
        .as_array()
        .expect("issues")
        .iter()
        .filter_map(|issue| issue["rule"].as_str().map(str::to_string))
        .collect()
}

#[test]
fn draw_route_lifts_crossing_arrow_above_the_node_it_cuts() {
    let path = copy_fixture("arrows", "route-cross");
    let path_s = path.to_str().expect("utf8 path");

    let (code, stdout, stderr) = run(&[
        "draw",
        "route",
        path_s,
        "--id",
        "arr_cross",
        "--via",
        "y=60",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert_report(&stderr, 0, 1, 0);

    let doc = load(&path);
    let arrow = element(&doc, "arr_cross");
    assert_eq!(arrow["elbowed"], true);
    assert!(arrow["roundness"].is_null());
    assert_eq!(
        arrow["startBinding"]["fixedPoint"],
        serde_json::json!([0.5, 0])
    );
    assert_eq!(
        arrow["endBinding"]["fixedPoint"],
        serde_json::json!([0.5, 0])
    );
    assert_eq!(num(arrow, "x"), 70.0);
    assert_eq!(num(arrow, "y"), 94.0);
    assert_eq!(
        arrow["points"],
        serde_json::json!([[0, 0], [0, -34], [440, -34], [440, 0]])
    );
    assert!(
        !lint_rules(path_s)
            .iter()
            .any(|rule| rule == "ARROW_CROSSING_NODE"),
        "crossing should be gone: {:?}",
        lint_rules(path_s)
    );

    let _ = std::fs::remove_file(&path);
}

#[test]
fn draw_route_bends_a_near_attachment_below_its_target() {
    let path = copy_fixture("arrows", "route-near");
    let path_s = path.to_str().expect("utf8 path");

    let (code, stdout, stderr) = run(&[
        "draw",
        "route",
        path_s,
        "--id",
        "arr_near",
        "--via",
        "y=420",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert_report(&stderr, 0, 1, 0);

    let near_doc = load(&path);
    let arrow = element(&near_doc, "arr_near");
    assert_eq!(
        absolute_points(arrow),
        vec![
            (290.0, 186.0),
            (290.0, 420.0),
            (510.0, 420.0),
            (510.0, 386.0)
        ]
    );
    assert_eq!(arrow["elbowed"], true);
    assert!(arrow["roundness"].is_null());
    assert_eq!(
        arrow["startBinding"]["fixedPoint"],
        serde_json::json!([0.5, 1])
    );
    assert!(arrow["endBinding"].is_null());

    let _ = std::fs::remove_file(&path);
}

#[test]
fn draw_connect_via_routes_around_aligned_nodes_and_places_the_label() {
    let path = temp_path("via-routes");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);
    seed_nodes(
        path_s,
        &[
            (
                "a",
                "A",
                &["--x", "0", "--y", "100", "--width", "140", "--height", "80"],
            ),
            (
                "b",
                "B",
                &[
                    "--x", "220", "--y", "100", "--width", "140", "--height", "80",
                ],
            ),
            (
                "c",
                "C",
                &[
                    "--x", "440", "--y", "100", "--width", "140", "--height", "80",
                ],
            ),
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
        "--via",
        "y=220",
        "--label",
        "async",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    let doc = load(&path);
    let arrow = element(&doc, "ac");
    assert_eq!(
        absolute_points(arrow),
        vec![(70.0, 186.0), (70.0, 220.0), (510.0, 220.0), (510.0, 186.0)]
    );
    assert_eq!(arrow["elbowed"], true);
    assert!(arrow["roundness"].is_null());
    let label = element(&doc, "ac-label");
    assert_eq!(num(label, "width"), 48.0);
    assert_eq!(num(label, "height"), 20.0);
    assert_eq!(num(label, "x"), 266.0);
    assert_eq!(num(label, "y"), 210.0);
    let rules = lint_rules(path_s);
    assert!(
        !rules
            .iter()
            .any(|rule| rule == "ARROW_CROSSING_NODE" || rule == "ARROW_LABEL_COLLISION"),
        "rules={rules:?}"
    );

    let stacked = temp_path("via-x");
    let stacked_s = stacked.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&stacked);
    seed_nodes(
        stacked_s,
        &[
            (
                "n1",
                "N1",
                &[
                    "--x", "100", "--y", "100", "--width", "140", "--height", "80",
                ],
            ),
            (
                "n2",
                "N2",
                &[
                    "--x", "100", "--y", "240", "--width", "140", "--height", "80",
                ],
            ),
            (
                "n3",
                "N3",
                &[
                    "--x", "100", "--y", "380", "--width", "140", "--height", "80",
                ],
            ),
        ],
    );
    let (code, stdout, stderr) = run(&[
        "draw",
        "connect",
        stacked_s,
        "--from",
        "n1",
        "--to",
        "n3",
        "--id",
        "n13",
        "--via",
        "x=40",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    let stacked_doc = load(&stacked);
    let arrow = element(&stacked_doc, "n13");
    assert_eq!(
        absolute_points(arrow),
        vec![(94.0, 140.0), (40.0, 140.0), (40.0, 420.0), (94.0, 420.0)]
    );
    assert!(
        !lint_rules(stacked_s)
            .iter()
            .any(|rule| rule == "ARROW_CROSSING_NODE"),
        "rules={:?}",
        lint_rules(stacked_s)
    );

    let channel = temp_path("via-z");
    let channel_s = channel.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&channel);
    seed_nodes(
        channel_s,
        &[
            (
                "top_box",
                "Top",
                &[
                    "--x", "100", "--y", "60", "--width", "140", "--height", "80",
                ],
            ),
            (
                "bot_box",
                "Bot",
                &[
                    "--x", "400", "--y", "260", "--width", "140", "--height", "80",
                ],
            ),
        ],
    );
    let (code, stdout, stderr) = run(&[
        "draw",
        "connect",
        channel_s,
        "--from",
        "top_box",
        "--to",
        "bot_box",
        "--id",
        "tb",
        "--via",
        "y=200",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    let channel_doc = load(&channel);
    let arrow = element(&channel_doc, "tb");
    assert_eq!(
        arrow["startBinding"]["fixedPoint"],
        serde_json::json!([0.5, 1])
    );
    assert_eq!(
        arrow["endBinding"]["fixedPoint"],
        serde_json::json!([0.5, 0])
    );
    assert_eq!(
        absolute_points(arrow),
        vec![
            (170.0, 146.0),
            (170.0, 200.0),
            (470.0, 200.0),
            (470.0, 254.0)
        ]
    );

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&stacked);
    let _ = std::fs::remove_file(&channel);
}

#[test]
fn draw_via_and_route_reject_bad_waypoints_and_non_arrows() {
    let path = temp_path("via-errors");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);
    seed_nodes(
        path_s,
        &[
            (
                "a",
                "A",
                &["--x", "0", "--y", "100", "--width", "140", "--height", "80"],
            ),
            (
                "c",
                "C",
                &[
                    "--x", "440", "--y", "100", "--width", "140", "--height", "80",
                ],
            ),
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
        "--via",
        "z=10",
        "--in-place",
    ]);
    assert_eq!(code, 2, "stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(stdout, "");
    assert!(
        stderr.contains("excal: error: invalid --via \"z=10\": expected x=<N> or y=<N>"),
        "stderr={stderr:?}"
    );

    let (code, stdout, stderr) = run(&[
        "draw",
        "connect",
        path_s,
        "--from",
        "a",
        "--to",
        "c",
        "--via",
        "y=140",
        "--in-place",
    ]);
    assert_eq!(code, 2, "stdout={stdout:?} stderr={stderr:?}");
    assert!(
        stderr.contains("excal: error: --via y=140 passes through endpoint bounds"),
        "stderr={stderr:?}"
    );

    let arrows = fixture("arrows");
    let arrows_s = arrows.to_str().expect("utf8 path");
    let (code, stdout, stderr) = run(&[
        "draw",
        "route",
        arrows_s,
        "--id",
        "arr_dangling",
        "--via",
        "y=60",
    ]);
    assert_eq!(code, 2, "stdout={stdout:?} stderr={stderr:?}");
    assert!(
        stderr.contains(
            "excal: error: arrow arr_dangling must be attached at both ends to route with --via"
        ),
        "stderr={stderr:?}"
    );

    let flow = fixture("basic_flow");
    let flow_s = flow.to_str().expect("utf8 path");
    let (code, stdout, stderr) = run(&["draw", "route", flow_s, "--id", "client", "--via", "y=60"]);
    assert_eq!(code, 2, "stdout={stdout:?} stderr={stderr:?}");
    assert!(
        stderr.contains("excal: error: client is not an arrow connector"),
        "stderr={stderr:?}"
    );

    let _ = std::fs::remove_file(&path);
}

#[test]
fn draw_node_from_edge_label_clamps_explicit_gap_to_label_floor() {
    let path = temp_path("gap-floor");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);
    seed_nodes(
        path_s,
        &[("client", "Client", &["--x", "100", "--y", "100"])],
    );

    // "POST /v1/charges" is 16 chars * 16 * 0.6 = 153.6; floor = ceil(153.6 + 24) = 178 > 120.
    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        path_s,
        "--id",
        "api_gw",
        "--label",
        "API",
        "--right-of",
        "client",
        "--gap",
        "120",
        "--from",
        "client",
        "--edge-label",
        "POST /v1/charges",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    let doc = load(&path);
    assert_eq!(num(element(&doc, "api_gw"), "x"), 438.0);
    assert!(
        !lint_rules(path_s)
            .iter()
            .any(|rule| rule == "ARROW_LABEL_COLLISION"),
        "clamped gap must not collide the edge label"
    );

    let path = temp_path("gap-floor-wide");
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
        "api_gw",
        "--label",
        "API",
        "--right-of",
        "client",
        "--gap",
        "300",
        "--from",
        "client",
        "--edge-label",
        "POST /v1/charges",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert_eq!(num(element(&load(&path), "api_gw"), "x"), 560.0);

    let path = temp_path("gap-no-edge");
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
        "api_gw",
        "--label",
        "API",
        "--right-of",
        "client",
        "--gap",
        "120",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert_eq!(num(element(&load(&path), "api_gw"), "x"), 380.0);

    // "ok" height is 16 * 1.25 = 20; floor = ceil(20 + 24) = 44 > 30.
    let path = temp_path("gap-floor-below");
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
        "api_gw",
        "--label",
        "API",
        "--below",
        "client",
        "--gap",
        "30",
        "--from",
        "client",
        "--edge-label",
        "ok",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert_eq!(num(element(&load(&path), "api_gw"), "y"), 224.0);
}

#[test]
fn draw_connect_elbow_uses_vertical_ports_when_boxes_overlap_horizontally() {
    let path = temp_path("axis-overlap");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);
    seed_nodes(
        path_s,
        &[
            (
                "a",
                "A",
                &[
                    "--x", "100", "--y", "100", "--width", "400", "--height", "80",
                ],
            ),
            (
                "b",
                "B",
                &["--x", "0", "--y", "220", "--width", "300", "--height", "80"],
            ),
        ],
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
        "ab",
        "--elbow",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    let doc = load(&path);
    let arrow = element(&doc, "ab");
    assert_eq!(
        arrow["startBinding"]["fixedPoint"],
        serde_json::json!([0.5, 1])
    );
    assert_eq!(
        arrow["endBinding"]["fixedPoint"],
        serde_json::json!([0.5, 0])
    );
    assert_eq!(num(arrow, "x"), 300.0);
    assert_eq!(num(arrow, "y"), 186.0);
    assert_eq!(
        arrow["points"],
        serde_json::json!([[0, 0], [0, 14], [-150, 14], [-150, 28]])
    );

    let _ = std::fs::remove_file(&path);
}

#[test]
fn draw_connect_elbow_spreads_fan_out_and_fan_in_ports() {
    let path = temp_path("fan");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);
    seed_nodes(
        path_s,
        &[
            (
                "blender",
                "Blender",
                &[
                    "--x", "100", "--y", "300", "--width", "160", "--height", "80",
                ],
            ),
            (
                "shard_1",
                "Shard 1",
                &[
                    "--x", "400", "--y", "180", "--width", "160", "--height", "80",
                ],
            ),
            (
                "shard_2",
                "Shard 2",
                &[
                    "--x", "400", "--y", "300", "--width", "160", "--height", "80",
                ],
            ),
            (
                "shard_3",
                "Shard 3",
                &[
                    "--x", "400", "--y", "420", "--width", "160", "--height", "80",
                ],
            ),
            (
                "ranker",
                "Ranker",
                &[
                    "--x", "700", "--y", "300", "--width", "160", "--height", "80",
                ],
            ),
        ],
    );

    let pairs = [
        ("blender", "shard_1", 0),
        ("blender", "shard_2", 1),
        ("blender", "shard_3", 2),
        ("shard_1", "ranker", 0),
        ("shard_2", "ranker", 1),
        ("shard_3", "ranker", 2),
    ];
    for (from, to, updated) in pairs {
        let (code, stdout, stderr) = run(&[
            "draw",
            "connect",
            path_s,
            "--from",
            from,
            "--to",
            to,
            "--elbow",
            "--in-place",
        ]);
        assert_eq!(code, 0, "{from}->{to} stderr={stderr:?} stdout={stdout:?}");
        assert_report(&stderr, 1, updated, 0);
    }

    let doc = load(&path);
    let blender_shard_1 = element(&doc, "blender-shard_1");
    assert_eq!(
        blender_shard_1["startBinding"]["fixedPoint"],
        serde_json::json!([1, 0.35])
    );
    assert_eq!(
        absolute_points(blender_shard_1),
        vec![
            (266.0, 328.0),
            (330.0, 328.0),
            (330.0, 220.0),
            (394.0, 220.0)
        ]
    );
    let blender_shard_2 = element(&doc, "blender-shard_2");
    assert_eq!(
        blender_shard_2["startBinding"]["fixedPoint"],
        serde_json::json!([1, 0.5])
    );
    assert_eq!(
        absolute_points(blender_shard_2),
        vec![(266.0, 340.0), (394.0, 340.0)]
    );
    let blender_shard_3 = element(&doc, "blender-shard_3");
    assert_eq!(
        blender_shard_3["startBinding"]["fixedPoint"],
        serde_json::json!([1, 0.65])
    );
    assert_eq!(
        absolute_points(blender_shard_3),
        vec![
            (266.0, 352.0),
            (330.0, 352.0),
            (330.0, 460.0),
            (394.0, 460.0)
        ]
    );

    for (id, y) in [
        ("shard_1-ranker", 328.0),
        ("shard_2-ranker", 340.0),
        ("shard_3-ranker", 352.0),
    ] {
        let arrow = element(&doc, id);
        let u = if (y - 340.0_f64).abs() < 0.5 {
            0.5
        } else if y < 340.0 {
            0.35
        } else {
            0.65
        };
        assert_eq!(
            arrow["endBinding"]["fixedPoint"],
            serde_json::json!([0, u]),
            "{id} end fixedPoint"
        );
        let abs = absolute_points(arrow);
        assert_eq!(
            abs.last().copied(),
            Some((694.0, y)),
            "{id} end point {abs:?}"
        );
    }
    assert_clean_lint(path_s);

    let _ = std::fs::remove_file(&path);
}
