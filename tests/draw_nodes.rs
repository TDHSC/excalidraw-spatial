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
        "excal-t33-{}-{}-{tag}.excalidraw",
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
