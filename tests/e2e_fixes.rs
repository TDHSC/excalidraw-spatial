use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const FIXED_NOW_MS: &str = "1700000000000";

const FIXTURES: &[(&str, usize)] = &[
    ("basic_flow", 0),
    ("overlap", 1),
    ("swimlanes", 1),
    ("arrows", 2),
    ("text_layout", 4),
    ("icon_group", 0),
];

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_excal")
}

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn make_temp_dir() -> TempDir {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("excal-t19-{}-{}", std::process::id(), nanos));
    fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("create {}: {e}", dir.display()));
    TempDir(dir)
}

fn run_lint_json(path: &str) -> serde_json::Value {
    let output = Command::new(bin())
        .args(["lint", path, "--format", "json"])
        .output()
        .expect("failed to spawn excal lint");
    let stdout = String::from_utf8(output.stdout).expect("lint stdout utf-8");
    serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "lint json parse failed: {e}; stdout={stdout}; stderr={}",
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn lint_fix_commands_are_executable_and_clear_the_issue() {
    let mut total = 0usize;
    for &(name, expected) in FIXTURES {
        let tmp = make_temp_dir();
        let fixture = format!("tests/fixtures/{name}.excalidraw");
        let copy_path = tmp.0.join(format!("{name}.excalidraw"));
        fs::copy(&fixture, &copy_path)
            .unwrap_or_else(|e| panic!("copy {fixture} -> {}: {e}", copy_path.display()));
        let copy_str = copy_path.to_str().expect("utf8 copy path");
        assert!(
            !copy_str.contains(' '),
            "temp path must not contain spaces: {copy_str}"
        );

        let lint = run_lint_json(copy_str);
        let issues = lint["issues"]
            .as_array()
            .unwrap_or_else(|| panic!("{name}: issues is not an array: {lint}"));

        let mut count = 0usize;
        for issue in issues {
            let Some(command) = issue["command"].as_str() else {
                continue;
            };
            count += 1;

            let mut argv: Vec<String> = command.split_whitespace().map(str::to_string).collect();
            assert_eq!(
                argv.first().map(String::as_str),
                Some("excal"),
                "{name}: first token must be excal: {command}"
            );
            argv[0] = bin().to_string();
            argv.push("--in-place".to_string());

            fs::copy(&fixture, &copy_path)
                .unwrap_or_else(|e| panic!("fresh copy {fixture} -> {}: {e}", copy_path.display()));

            let output = Command::new(&argv[0])
                .args(&argv[1..])
                .env("EXCAL_FIXED_NOW_MS", FIXED_NOW_MS)
                .output()
                .unwrap_or_else(|e| panic!("failed to spawn fix command {argv:?}: {e}"));
            let code = output
                .status
                .code()
                .unwrap_or_else(|| panic!("fix command terminated by signal: {argv:?}"));
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(
                code, 0,
                "{name}: fix command failed\ncommand={argv:?}\nstdout={stdout}\nstderr={stderr}"
            );

            let after = run_lint_json(copy_str);
            let after_issues = after["issues"]
                .as_array()
                .unwrap_or_else(|| panic!("{name}: post-fix issues is not an array: {after}"));
            let rule = issue["rule"].as_str().expect("rule is string");
            let targets = &issue["targets"];
            let still = after_issues
                .iter()
                .any(|i| i["rule"].as_str() == Some(rule) && &i["targets"] == targets);
            assert!(
                !still,
                "{name}: issue still present after fix\ncommand={command}\noriginal={issue}\nnew_issues={after_issues:?}"
            );
        }

        assert_eq!(
            count, expected,
            "{name}: expected {expected} fix commands, got {count}"
        );
        total += count;
    }
    assert_eq!(
        total, 8,
        "expected 8 fix commands across all fixtures, got {total}"
    );
}

fn run_layout_in_place(path: &str, args: &[&str]) -> (i32, String, String) {
    let mut argv = vec!["layout"];
    argv.extend_from_slice(args);
    argv.push(path);
    argv.push("--in-place");
    let output = Command::new(bin())
        .args(&argv)
        .env("EXCAL_FIXED_NOW_MS", FIXED_NOW_MS)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn {argv:?}: {e}"));
    let code = output.status.code().expect("process terminated by signal");
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr utf-8");
    (code, stdout, stderr)
}

fn approx(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b} (tol 1e-6)");
}

#[test]
fn system_design_pipeline_converges_cleanly() {
    let tmp = make_temp_dir();
    let path = tmp.0.join("system_design.excalidraw");
    let src = r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "z_ingress", "x": 30, "y": 55, "width": 480, "height": 250},
    {"type": "text", "id": "z_ingress_title", "x": 46, "y": 62, "width": 228, "height": 25, "text": "1. Edge & Risk Gate", "fontSize": 20, "lineHeight": 1.25},
    {
      "type": "rectangle",
      "id": "pay_gw",
      "x": 60,
      "y": 120,
      "width": 175,
      "height": 75,
      "boundElements": [{"type": "text", "id": "pay_gw-label"}]
    },
    {
      "type": "text",
      "id": "pay_gw-label",
      "x": 92.5,
      "y": 145,
      "width": 110,
      "height": 25,
      "text": "Pay GW",
      "fontSize": 20,
      "textAlign": "center",
      "verticalAlign": "middle",
      "containerId": "pay_gw"
    },
    {
      "type": "diamond",
      "id": "risk_decision",
      "x": 310,
      "y": 105,
      "width": 160,
      "height": 105,
      "boundElements": [{"type": "text", "id": "risk_decision-label"}]
    },
    {
      "type": "text",
      "id": "risk_decision-label",
      "x": 338,
      "y": 132.5,
      "width": 104,
      "height": 50,
      "text": "Fraud / 3DS\nCheck",
      "fontSize": 20,
      "lineHeight": 1.25,
      "textAlign": "center",
      "verticalAlign": "middle",
      "containerId": "risk_decision"
    },
    {"type": "rectangle", "id": "z_settle", "x": 30, "y": 340, "width": 1020, "height": 250},
    {"type": "text", "id": "z_settle_title", "x": 46, "y": 352, "width": 156, "height": 25, "text": "2. Settlement", "fontSize": 20, "lineHeight": 1.25},
    {
      "type": "rectangle",
      "id": "psp_router",
      "x": 60,
      "y": 420,
      "width": 175,
      "height": 75,
      "boundElements": [
        {"type": "arrow", "id": "a_req"},
        {"type": "arrow", "id": "a_resp"}
      ]
    },
    {
      "type": "rectangle",
      "id": "card_net",
      "x": 335,
      "y": 420,
      "width": 175,
      "height": 75,
      "boundElements": [
        {"type": "arrow", "id": "a_req"},
        {"type": "arrow", "id": "a_resp"}
      ]
    },
    {
      "type": "rectangle",
      "id": "clearing",
      "x": 610,
      "y": 420,
      "width": 175,
      "height": 75
    },
    {
      "type": "arrow",
      "id": "a_req",
      "x": 241,
      "y": 440,
      "width": 88,
      "height": 0,
      "points": [[0, 0], [88, 0]],
      "startBinding": {"elementId": "psp_router", "mode": "orbit"},
      "endBinding": {"elementId": "card_net", "mode": "orbit"},
      "boundElements": [{"type": "text", "id": "a_req-label"}]
    },
    {
      "type": "text",
      "id": "a_req-label",
      "x": 246.6,
      "y": 430,
      "width": 76.8,
      "height": 20,
      "text": "AUTH req",
      "fontSize": 16,
      "lineHeight": 1.25,
      "textAlign": "center",
      "containerId": "a_req"
    },
    {
      "type": "arrow",
      "id": "a_resp",
      "x": 329,
      "y": 475,
      "width": 88,
      "height": 0,
      "points": [[0, 0], [-88, 0]],
      "startBinding": {"elementId": "card_net", "mode": "orbit"},
      "endBinding": {"elementId": "psp_router", "mode": "orbit"},
      "boundElements": [{"type": "text", "id": "a_resp-label"}]
    },
    {
      "type": "text",
      "id": "a_resp-label",
      "x": 256.2,
      "y": 465,
      "width": 57.6,
      "height": 20,
      "text": "200 OK",
      "fontSize": 16,
      "lineHeight": 1.25,
      "textAlign": "center",
      "containerId": "a_resp"
    }
  ]
}"#;
    fs::write(&path, src).expect("write temp system design");
    let path_str = path.to_str().expect("utf8 path");

    let steps: &[(&[&str], &str)] = &[
        (&["fit-text", "--all"], "fit-text --all"),
        (
            &[
                "distribute",
                "--ids",
                "pay_gw,risk_decision",
                "--direction",
                "horizontal",
                "--gap",
                "60",
            ],
            "distribute ingress",
        ),
        (
            &[
                "distribute",
                "--ids",
                "psp_router,card_net,clearing",
                "--direction",
                "horizontal",
                "--gap",
                "90",
            ],
            "distribute settle",
        ),
        (
            &[
                "fit-container",
                "--id",
                "z_ingress",
                "--padding",
                "40",
                "--expand-only",
            ],
            "fit-container z_ingress",
        ),
        (
            &[
                "fit-container",
                "--id",
                "z_settle",
                "--padding",
                "40",
                "--expand-only",
            ],
            "fit-container z_settle",
        ),
    ];
    for (args, name) in steps {
        let (code, stdout, stderr) = run_layout_in_place(path_str, args);
        assert_eq!(code, 0, "{name} failed\nstdout={stdout}\nstderr={stderr}");
    }

    let after = fs::read_to_string(&path).expect("read result");
    let doc = excalidraw_spatial::schema::Document::parse(&after).expect("parse result");
    let scene = excalidraw_spatial::scene_graph::Scene::build(&doc);
    assert_eq!(
        scene.containers.len(),
        2,
        "containers={:?}",
        scene.containers.iter().map(|c| &c.id).collect::<Vec<_>>()
    );
    let mut ids: Vec<_> = scene.containers.iter().map(|c| c.id.as_str()).collect();
    ids.sort();
    assert_eq!(ids, vec!["z_ingress", "z_settle"]);

    let a_req = doc.element("a_req").expect("a_req");
    let a_resp = doc.element("a_resp").expect("a_resp");
    approx(a_req.y(), 440.0);
    approx(a_resp.y(), 475.0);

    let lint = Command::new(bin())
        .args(["lint", path_str, "--format", "json"])
        .output()
        .expect("lint");
    let lint_code = lint.status.code().expect("lint code");
    let lint_stdout = String::from_utf8(lint.stdout).expect("lint stdout");
    let lint_stderr = String::from_utf8_lossy(&lint.stderr);
    assert_eq!(
        lint_code, 0,
        "lint exit {lint_code}\nstdout={lint_stdout}\nstderr={lint_stderr}"
    );
    let lint_json: serde_json::Value = serde_json::from_str(&lint_stdout).expect("lint json");
    assert_eq!(lint_json["summary"]["errors"], 0, "lint={lint_json}");
    assert_eq!(lint_json["summary"]["warnings"], 0, "lint={lint_json}");
    assert_eq!(lint_json["summary"]["info"], 0, "lint={lint_json}");
}

#[test]
fn scatter_gather_in_tight_zone_cotranslates_and_grows() {
    let tmp = make_temp_dir();
    let path = tmp.0.join("scatter_zone.excalidraw");
    // Compressed so l2_reranker's center starts inside z_query; distribute then
    // pushes that center past the right edge and T28 grows the zone to keep it.
    let src = r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "z_query", "x": 20, "y": 220, "width": 1250, "height": 360},
    {"type": "text", "id": "z_query_title", "x": 36, "y": 232, "width": 80, "height": 25, "text": "Query", "fontSize": 20},
    {"type": "rectangle", "id": "q_gw", "x": 40, "y": 370, "width": 175, "height": 75},
    {"type": "rectangle", "id": "q_rewrite", "x": 255, "y": 370, "width": 175, "height": 75},
    {"type": "rectangle", "id": "blender", "x": 470, "y": 370, "width": 175, "height": 75},
    {"type": "rectangle", "id": "shard_1", "x": 685, "y": 265, "width": 175, "height": 75},
    {"type": "rectangle", "id": "shard_2", "x": 685, "y": 370, "width": 175, "height": 75},
    {"type": "rectangle", "id": "shard_3", "x": 685, "y": 475, "width": 175, "height": 75},
    {"type": "rectangle", "id": "l1_ranker", "x": 900, "y": 370, "width": 175, "height": 75},
    {"type": "rectangle", "id": "l2_reranker", "x": 1085, "y": 370, "width": 175, "height": 75}
  ]
}"#;
    fs::write(&path, src).expect("write scatter zone");
    let path_str = path.to_str().expect("utf8 path");
    let (code, stdout, stderr) = run_layout_in_place(
        path_str,
        &[
            "distribute",
            "--ids",
            "q_gw,q_rewrite,blender,shard_2,l1_ranker,l2_reranker",
            "--direction",
            "horizontal",
            "--gap",
            "90",
        ],
    );
    assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
    assert!(
        stderr.contains("excal: lint after change: errors: 0, warnings: 0, info: 0"),
        "stderr={stderr}"
    );

    let after = fs::read_to_string(&path).expect("read result");
    let doc = excalidraw_spatial::schema::Document::parse(&after).expect("parse result");
    let scene = excalidraw_spatial::scene_graph::Scene::build(&doc);
    for id in ["shard_1", "shard_2", "shard_3"] {
        approx(scene.component(id).unwrap().rect.x1, 835.0);
    }
    let l2 = scene.component("l2_reranker").unwrap();
    approx(l2.rect.x1, 1365.0);
    assert_eq!(l2.parent.as_deref(), Some("z_query"));
    let zone = scene.container("z_query").expect("zone");
    assert_eq!(
        zone.kind,
        excalidraw_spatial::scene_graph::ContainerKind::Zone
    );
    approx(zone.rect.x2, 1580.0);
    approx(zone.rect.width(), 1560.0);
}

#[test]
fn fit_text_then_distribute_pushes_disjoint_sibling_zone() {
    let tmp = make_temp_dir();
    let path = tmp.0.join("llm_cluster.excalidraw");
    let src = r#"{
  "type": "excalidraw",
  "version": 2,
  "elements": [
    {"type": "rectangle", "id": "z_fabric", "x": 0, "y": 0, "width": 1680, "height": 300},
    {"type": "rectangle", "id": "z_prefill", "x": 400, "y": 55, "width": 590, "height": 210},
    {"type": "rectangle", "id": "z_rag", "x": 1030, "y": 55, "width": 610, "height": 210},
    {"type": "rectangle", "id": "p_sched", "x": 460, "y": 115, "width": 160, "height": 75, "boundElements": [{"type": "text", "id": "p_sched-label"}]},
    {"type": "rectangle", "id": "p_node1", "x": 600, "y": 115, "width": 160, "height": 75, "boundElements": [{"type": "text", "id": "p_node1-label"}]},
    {"type": "rectangle", "id": "p_node2", "x": 810, "y": 115, "width": 160, "height": 75, "boundElements": [{"type": "text", "id": "p_node2-label"}]},
    {"type": "rectangle", "id": "embed_svc", "x": 1070, "y": 115, "width": 175, "height": 75, "boundElements": [{"type": "text", "id": "embed_svc-label"}]},
    {"type": "rectangle", "id": "milvus", "x": 1330, "y": 115, "width": 250, "height": 75, "boundElements": [{"type": "text", "id": "milvus-label"}]},
    {"type": "text", "id": "p_sched-label", "x": 460, "y": 115, "width": 0, "height": 0, "text": "KV-Aware Prefill\nRouter", "fontSize": 20, "lineHeight": 1.25, "containerId": "p_sched"},
    {"type": "text", "id": "p_node1-label", "x": 600, "y": 115, "width": 0, "height": 0, "text": "Prefill Worker #1\n(H100 TP=8 FP8)", "fontSize": 20, "lineHeight": 1.25, "containerId": "p_node1"},
    {"type": "text", "id": "p_node2-label", "x": 810, "y": 115, "width": 0, "height": 0, "text": "Prefill Worker #2\n(H100 TP=8 FP8)", "fontSize": 20, "lineHeight": 1.25, "containerId": "p_node2"},
    {"type": "text", "id": "embed_svc-label", "x": 1070, "y": 115, "width": 0, "height": 0, "text": "BGE-M3 Embedding\nService", "fontSize": 20, "lineHeight": 1.25, "containerId": "embed_svc"},
    {"type": "text", "id": "milvus-label", "x": 1330, "y": 115, "width": 0, "height": 0, "text": "Milvus HNSW\nShard", "fontSize": 20, "lineHeight": 1.25, "containerId": "milvus"}
  ]
}"#;
    fs::write(&path, src).expect("write llm cluster");
    let path_str = path.to_str().expect("utf8 path");
    let (code, stdout, stderr) = run_layout_in_place(path_str, &["fit-text", "--all"]);
    assert_eq!(code, 0, "fit-text failed\nstdout={stdout}\nstderr={stderr}");
    let (code, stdout, stderr) = run_layout_in_place(
        path_str,
        &[
            "distribute",
            "--ids",
            "p_sched,p_node1,p_node2",
            "--direction",
            "horizontal",
            "--gap",
            "95",
        ],
    );
    assert_eq!(
        code, 0,
        "distribute failed\nstdout={stdout}\nstderr={stderr}"
    );

    let after = fs::read_to_string(&path).expect("read result");
    let doc = excalidraw_spatial::schema::Document::parse(&after).expect("parse result");
    let scene = excalidraw_spatial::scene_graph::Scene::build(&doc);
    approx(scene.component("p_node1").unwrap().rect.x1, 755.0);
    approx(scene.component("p_node2").unwrap().rect.x1, 1102.0);
    let prefill = scene.container("z_prefill").unwrap();
    approx(prefill.rect.x1, 400.0);
    approx(prefill.rect.y1, 55.0);
    approx(prefill.rect.width(), 994.0);
    approx(prefill.rect.height(), 210.0);
    approx(scene.container("z_rag").unwrap().rect.x1, 1434.0);
    approx(scene.component("embed_svc").unwrap().rect.x1, 1441.5);
    approx(scene.component("milvus").unwrap().rect.x1, 1734.0);
    let fabric = scene.container("z_fabric").unwrap();
    approx(fabric.rect.x1, 0.0);
    approx(fabric.rect.y1, 0.0);
    approx(fabric.rect.width(), 2084.0);
    approx(fabric.rect.height(), 300.0);
    assert_eq!(
        scene.component("p_node2").unwrap().parent.as_deref(),
        Some("z_prefill")
    );
    assert_eq!(
        scene.component("embed_svc").unwrap().parent.as_deref(),
        Some("z_rag")
    );
    assert_eq!(
        scene.component("milvus").unwrap().parent.as_deref(),
        Some("z_rag")
    );

    let lint_json = run_lint_json(path_str);
    assert_eq!(lint_json["summary"]["errors"], 0, "lint={lint_json}");
    let issues = lint_json["issues"].as_array().expect("issues");
    assert!(
        issues
            .iter()
            .all(|issue| issue["rule"] != "OVERLAP_COMPONENT"),
        "overlaps: {lint_json}"
    );
}
