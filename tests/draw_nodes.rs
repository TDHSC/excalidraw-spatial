use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
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
        "excal-t34-{}-{}-{tag}.excalidraw",
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

fn run_stdin(args: &[&str], stdin: &str) -> (i32, String, String) {
    let mut child = Command::new(bin())
        .args(args)
        .env("EXCAL_FIXED_NOW_MS", FIXED_NOW_MS)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn excal");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(stdin.as_bytes())
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait excal");
    let code = output.status.code().expect("process terminated by signal");
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr utf-8");
    (code, stdout, stderr)
}

fn expected_empty_document() -> serde_json::Value {
    serde_json::json!({
        "type": "excalidraw",
        "version": 2,
        "source": "https://excalidraw.com",
        "elements": [],
        "appState": {
            "gridSize": 20,
            "gridStep": 5,
            "gridModeEnabled": false,
            "viewBackgroundColor": "#ffffff",
            "lockedMultiSelections": {}
        },
        "files": {}
    })
}

const INIT_STDERR: &str = "\
excal: added 0 item(s), updated 0 item(s), removed 0 item(s)
excal: lint after change: errors: 0, warnings: 0, info: 0
";

#[test]
fn draw_init_creates_empty_document_and_inspects_as_empty_canvas() {
    let path = temp_path("init-new");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);

    let (code, stdout, stderr) = run(&["draw", "init", path_s]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert_eq!(stdout, "");
    assert_eq!(stderr, INIT_STDERR);

    let written = std::fs::read_to_string(&path).expect("written file");
    assert!(
        written.ends_with('\n'),
        "empty document must end with a trailing newline"
    );
    let doc: serde_json::Value = serde_json::from_str(&written).expect("json");
    assert_eq!(doc, expected_empty_document());

    let (code, stdout, stderr) = run(&["inspect", path_s]);
    assert_eq!(code, 0, "stderr={stderr:?}");
    assert!(
        stdout.contains("canvas (empty)"),
        "inspect must report an empty canvas, got {stdout}"
    );

    let _ = std::fs::remove_file(&path);
}

#[test]
fn draw_init_refuses_existing_file_unless_force() {
    let path = temp_path("init-exists");
    let path_s = path.to_str().expect("utf8 path");
    std::fs::write(&path, "keep-me\n").expect("seed file");

    let (code, stdout, stderr) = run(&["draw", "init", path_s]);
    assert_eq!(code, 2, "stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(stdout, "");
    assert_eq!(
        stderr,
        format!("excal: error: file \"{path_s}\" already exists; pass --force to overwrite\n")
    );
    assert_eq!(std::fs::read_to_string(&path).expect("kept"), "keep-me\n");

    let (code, stdout, stderr) = run(&["draw", "init", path_s, "--force"]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert_eq!(stderr, INIT_STDERR);
    let written = std::fs::read_to_string(&path).expect("overwritten");
    let doc: serde_json::Value = serde_json::from_str(&written).expect("json");
    assert_eq!(doc["elements"], serde_json::json!([]));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn draw_init_stdout_and_rejects_in_place_on_stdin() {
    let (code, stdout, stderr) = run_stdin(&["draw", "init", "-"], "");
    assert_eq!(code, 0, "stderr={stderr:?}");
    assert_eq!(stderr, INIT_STDERR);
    assert!(stdout.ends_with('\n'));
    let doc: serde_json::Value = serde_json::from_str(&stdout).expect("json");
    assert_eq!(doc, expected_empty_document());

    let (code, stdout, stderr) = run_stdin(&["draw", "init", "-", "--in-place"], "");
    assert_eq!(code, 2, "stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(stdout, "");
    assert!(
        stderr.contains("cannot use --in-place with stdin"),
        "stderr={stderr:?}"
    );
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

const CLEAN_STDERR: &str = "\
excal: added 1 item(s), updated 0 item(s), removed 0 item(s)\n\
excal: lint after change: errors: 0, warnings: 0, info: 0\n\
";

#[test]
fn draw_node_auto_inits_missing_file_with_deterministic_bound_label() {
    let path = temp_path("client");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        path_s,
        "--id",
        "client",
        "--label",
        "Client",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert_eq!(stdout, "");
    assert_eq!(stderr, CLEAN_STDERR);

    let written = std::fs::read_to_string(&path).expect("written");
    let doc: serde_json::Value = serde_json::from_str(&written).expect("json");
    let client = element(&doc, "client");
    assert_eq!(num(client, "x"), 100.0);
    assert_eq!(num(client, "y"), 100.0);
    assert_eq!(num(client, "width"), 160.0);
    assert_eq!(num(client, "height"), 80.0);
    assert_eq!(client["seed"], 2_060_559_059);
    assert_eq!(client["version"], 1);
    assert_eq!(client["versionNonce"], 491_086_952);
    assert_eq!(client["updated"], 1_700_000_000_000_i64);
    assert_eq!(client["index"], serde_json::Value::Null);
    assert_eq!(
        client["boundElements"],
        serde_json::json!([{"type": "text", "id": "client-label"}])
    );

    let label = element(&doc, "client-label");
    assert_eq!(num(label, "width"), 72.0);
    assert_eq!(num(label, "height"), 25.0);
    assert_eq!(num(label, "x"), 144.0);
    assert_eq!(num(label, "y"), 127.5);
    assert_eq!(label["seed"], 1_838_114_862);
    assert_eq!(label["versionNonce"], 290_245_772);
    assert_eq!(label["containerId"], "client");

    let (code, stdout, stderr) = run(&["lint", path_s]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert!(
        stdout.contains("errors: 0, warnings: 0, info: 0"),
        "lint must be clean, got {stdout}"
    );

    let _ = std::fs::remove_file(&path);
}

#[test]
fn draw_node_preserves_untouched_fixture_elements() {
    let out = temp_path("preserve");
    let out_s = out.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&out);

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        "tests/fixtures/basic_flow.excalidraw",
        "--id",
        "cache",
        "--label",
        "Redis",
        "--below",
        "api",
        "-o",
        out_s,
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert_eq!(stdout, "");

    let original: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/basic_flow.excalidraw")).expect("fixture");
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).expect("written")).expect("json");
    let original_ids = [
        "client",
        "client-label",
        "api",
        "api-label",
        "svc",
        "svc-label",
        "a1",
        "a1-label",
        "a2",
        "a2-label",
    ];
    for id in original_ids {
        assert_eq!(
            element(&written, id),
            element(&original, id),
            "untouched element {id} changed"
        );
    }
    assert!(
        written["elements"]
            .as_array()
            .expect("elements")
            .iter()
            .any(|el| el["id"] == "cache"),
        "cache node must be added"
    );

    let _ = std::fs::remove_file(&out);
}

#[test]
fn draw_node_autosizes_long_labels_and_shape_factors() {
    let path = temp_path("shapes");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        path_s,
        "--id",
        "client",
        "--label",
        "Client",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        path_s,
        "--id",
        "svc",
        "--label",
        "Order Fulfillment Service",
        "--right-of",
        "client",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("written")).expect("json");
    let svc = element(&doc, "svc");
    assert_eq!(num(svc, "x"), 320.0);
    assert_eq!(num(svc, "y"), 100.0);
    assert_eq!(num(svc, "width"), 348.0);
    assert_eq!(num(svc, "height"), 80.0);

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        path_s,
        "--id",
        "dec",
        "--shape",
        "diamond",
        "--label",
        "Fraud / 3DS\\nCheck",
        "--right-of",
        "svc",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("written")).expect("json");
    let dec = element(&doc, "dec");
    assert_eq!(num(dec, "x"), 728.0);
    assert_eq!(num(dec, "y"), 58.0);
    assert_eq!(num(dec, "width"), 360.0);
    assert_eq!(num(dec, "height"), 164.0);
    assert_eq!(dec["roundness"], serde_json::json!({"type": 3}));
    let dec_label = element(&doc, "dec-label");
    assert_eq!(num(dec_label, "width"), 132.0);
    assert_eq!(num(dec_label, "height"), 50.0);
    assert_eq!(num(dec_label, "x"), 842.0);
    assert_eq!(num(dec_label, "y"), 115.0);
    assert_eq!(dec_label["text"], "Fraud / 3DS\nCheck");

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        path_s,
        "--id",
        "cache",
        "--shape",
        "ellipse",
        "--label",
        "Cache Cluster",
        "--below",
        "svc",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("written")).expect("json");
    let cache = element(&doc, "cache");
    // tw=156, th=25, f=0.707 => width 289, height 81 so fit-text is a no-op.
    assert_eq!(num(cache, "width"), 289.0);
    assert_eq!(num(cache, "height"), 81.0);
    assert_eq!(cache["roundness"], serde_json::json!({"type": 2}));

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        path_s,
        "--id",
        "note_card",
        "--shape",
        "stickynote",
        "--label",
        "TODO",
        "--below",
        "dec",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("written")).expect("json");
    let note = element(&doc, "note_card");
    assert_eq!(note["roundness"], serde_json::Value::Null);
    assert_eq!(note["backgroundColor"], "#fff3bf");
    assert_eq!(num(note, "baseHeight"), 80.0);

    let before = std::fs::read_to_string(&path).expect("before fit-text");
    let (code, stdout, stderr) = run(&["layout", "fit-text", path_s, "--all"]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert_eq!(stderr, "excal: nothing to change\n");
    assert_eq!(stdout, before, "fit-text --all must be a no-op");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn draw_node_on_non_empty_canvas_requires_placement() {
    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        "tests/fixtures/basic_flow.excalidraw",
        "--id",
        "x",
        "--label",
        "X",
    ]);
    assert_eq!(code, 2, "stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(stdout, "");
    assert_eq!(
        stderr,
        "excal: error: node placement requires --x and --y or one of --right-of, --left-of, --below, --above\n"
    );
}

#[test]
fn draw_node_output_modes_stdout_file_and_stdin() {
    let path = temp_path("modes");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);
    std::fs::write(&path, include_str!("fixtures/basic_flow.excalidraw")).expect("seed");

    let (code, stdout, stderr) = run(&[
        "draw", "node", path_s, "--id", "extra", "--label", "Extra", "--x", "900", "--y", "100",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?}");
    assert!(
        stdout.contains("\"extra\""),
        "stdout should be JSON, got {stdout}"
    );
    assert!(stdout.trim_end().ends_with('}'));
    assert_eq!(
        std::fs::read_to_string(&path).expect("unchanged"),
        include_str!("fixtures/basic_flow.excalidraw")
    );

    let out = temp_path("modes-out");
    let out_s = out.to_str().expect("utf8 path");
    let (code, stdout, stderr) = run(&[
        "draw", "node", path_s, "--id", "extra", "--label", "Extra", "--x", "900", "--y", "100",
        "-o", out_s,
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert_eq!(stdout, "");
    let written = std::fs::read_to_string(&out).expect("written");
    assert!(written.contains("\"extra\""));

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        path_s,
        "--id",
        "inplace",
        "--label",
        "In Place",
        "--x",
        "900",
        "--y",
        "300",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert_eq!(stdout, "");
    let overwritten = std::fs::read_to_string(&path).expect("overwritten");
    assert!(overwritten.contains("\"inplace\""));

    let empty = include_str!("fixtures/basic_flow.excalidraw");
    let (code, stdout, stderr) = run_stdin(
        &[
            "draw",
            "node",
            "-",
            "--id",
            "piped",
            "--label",
            "Piped",
            "--x",
            "10",
            "--y",
            "10",
            "--in-place",
        ],
        empty,
    );
    assert_eq!(code, 2, "stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(stdout, "");
    assert!(
        stderr.contains("cannot use --in-place with stdin"),
        "stderr={stderr:?}"
    );

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&out);
}

#[test]
fn draw_node_stderr_reports_overlap_error_and_text_overflow_warning() {
    let path = temp_path("lint");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        path_s,
        "--id",
        "client",
        "--label",
        "Client",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        path_s,
        "--id",
        "overlap_box",
        "--label",
        "Auth",
        "--x",
        "120",
        "--y",
        "100",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(stdout, "");
    let expected = [
        "excal: added 1 item(s), updated 0 item(s), removed 0 item(s)\n",
        "excal: lint after change: errors: 1, warnings: 0, info: 0\n",
        "ERROR OVERLAP_COMPONENT client \"Client\" overlaps overlap_box \"Auth\" by 140x80px\n",
        "      fix: move overlap_box down by 120px to leave a 40px gap\n",
        "      run: excal layout move ",
        path_s,
        " --id overlap_box --dy 120\n",
    ]
    .concat();
    assert_eq!(stderr, expected);

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        path_s,
        "--id",
        "tight",
        "--label",
        "Order Fulfillment Service",
        "--width",
        "80",
        "--x",
        "400",
        "--y",
        "100",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stdout={stdout:?} stderr={stderr:?}");
    assert!(
        stderr.contains("WARN  TEXT_OVERFLOW"),
        "missing TEXT_OVERFLOW warning: {stderr:?}"
    );
    assert!(
        stderr.contains(&format!("run: excal layout fit-text {path_s} --id tight")),
        "missing fit-text run command: {stderr:?}"
    );

    let _ = std::fs::remove_file(&path);
}

#[test]
fn draw_text_above_client_uses_label_alias_and_free_text_metrics() {
    let path = temp_path("text");
    let path_s = path.to_str().expect("utf8 path");
    let _ = std::fs::remove_file(&path);

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        path_s,
        "--id",
        "client",
        "--label",
        "Client",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");

    let (code, stdout, stderr) = run(&[
        "draw",
        "text",
        path_s,
        "--id",
        "note1",
        "--label",
        "v2.0 Architecture",
        "--above",
        "client",
        "--gap",
        "40",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert_eq!(stderr, CLEAN_STDERR);

    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("written")).expect("json");
    let note = element(&doc, "note1");
    assert_eq!(num(note, "width"), 204.0);
    assert_eq!(num(note, "height"), 25.0);
    assert_eq!(num(note, "x"), 78.0);
    assert_eq!(num(note, "y"), 35.0);
    assert_eq!(note["containerId"], serde_json::Value::Null);
    assert_eq!(note["textAlign"], "left");
    assert_eq!(note["verticalAlign"], "top");
    assert_eq!(note["text"], "v2.0 Architecture");

    let (code, stdout, stderr) = run(&["lint", path_s]);
    assert_eq!(code, 0, "stderr={stderr:?} stdout={stdout:?}");
    assert!(
        stdout.contains("errors: 0, warnings: 0, info: 0"),
        "lint must be clean, got {stdout}"
    );

    let _ = std::fs::remove_file(&path);
}
