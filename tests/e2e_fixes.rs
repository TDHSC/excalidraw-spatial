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
