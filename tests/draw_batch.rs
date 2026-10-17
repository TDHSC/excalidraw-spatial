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
    std::env::temp_dir().join(format!("excal-t39-{}-{}-{tag}", std::process::id(), nanos))
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

const MIXED_SCRIPT: &str = "\
# Build a 3-node backend flow
node --id client --label \"Client\" --x 100 --y 100
node --id api --label \"API Gateway\" --x 360 --y 105
node --id db --label \"Database\" --x 620 --y 100
layout align --ids client,api,db --axis middle
zone --id lane_be --title \"Backend\" --children api,db --bg \"#ebfbee\"
connect --from client --to api --label \"HTTPS\"
connect --from api --to db --label \"SQL\"
";

const CLEAN_STDERR: &str = "\
excal: added 6 item(s), updated 1 item(s), removed 0 item(s)
excal: lint after change: errors: 0, warnings: 0, info: 0
";

#[test]
fn batch_aborts_on_unknown_id_without_writing_the_file() {
    let path = temp_path("atomic-fail.excalidraw");
    let path_s = path.to_str().expect("utf8 path");
    let script = temp_path("atomic-fail.excal");
    let script_s = script.to_str().expect("utf8 path");
    std::fs::write(
        &script,
        "\
node --id client --label \"Client\" --x 100 --y 100
node --id api --label \"API\" --x 360 --y 100
connect --from client --to missing
",
    )
    .expect("write script");
    let _ = std::fs::remove_file(&path);

    let (code, stdout, stderr) =
        run(&["draw", "batch", path_s, "--script", script_s, "--in-place"]);
    assert_eq!(code, 2, "stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(stdout, "");
    assert!(
        stderr.contains("excal: error: line 3: unknown id \"missing\""),
        "stderr={stderr:?}"
    );
    assert!(!path.exists(), "failed batch must not create {path_s}");

    let _ = std::fs::remove_file(&script);
}

#[test]
fn batch_rejects_output_flags_init_and_nested_batch_inside_the_script() {
    let path = temp_path("forbidden.excalidraw");
    let path_s = path.to_str().expect("utf8 path");

    let cases: &[(&str, &str)] = &[
        ("node --id a --label A --x 0 --y 0 --in-place\n", "in-place"),
        (
            "node --id a --label A --x 0 --y 0 -o /tmp/nope.excalidraw\n",
            "-o",
        ),
        ("init\n", "init"),
        ("draw init\n", "draw init"),
        ("batch\n", "batch"),
        ("draw batch\n", "draw batch"),
    ];

    for (body, label) in cases {
        let script = temp_path(&format!("forbidden-{label}.excal"));
        let script_s = script.to_str().expect("utf8 path");
        std::fs::write(&script, body).expect("write script");
        let _ = std::fs::remove_file(&path);

        let (code, stdout, stderr) =
            run(&["draw", "batch", path_s, "--script", script_s, "--in-place"]);
        assert_eq!(code, 2, "{label}: stdout={stdout:?} stderr={stderr:?}");
        assert_eq!(stdout, "", "{label}");
        assert!(
            stderr.starts_with("excal: error: line "),
            "{label}: stderr={stderr:?}"
        );
        assert!(
            !path.exists(),
            "{label}: failed batch must not create the file"
        );
        let _ = std::fs::remove_file(&script);
    }
}

#[test]
fn batch_refuses_to_read_document_and_script_from_the_same_stdin() {
    let (code, stdout, stderr) = run_stdin(&["draw", "batch", "-"], "");
    assert_eq!(code, 2, "stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(stdout, "");
    assert_eq!(
        stderr,
        "excal: error: cannot read both document and batch script from stdin; pass --script <PATH>\n"
    );

    let (code, stdout, stderr) = run_stdin(&["draw", "batch", "-", "--script", "-"], "");
    assert_eq!(code, 2, "stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(stdout, "");
    assert_eq!(
        stderr,
        "excal: error: cannot read both document and batch script from stdin; pass --script <PATH>\n"
    );
}

#[test]
fn batch_mixes_draw_and_layout_from_script_and_from_stdin() {
    let path = temp_path("mixed.excalidraw");
    let path_s = path.to_str().expect("utf8 path");
    let script = temp_path("mixed.excal");
    let script_s = script.to_str().expect("utf8 path");
    std::fs::write(&script, MIXED_SCRIPT).expect("write script");
    let _ = std::fs::remove_file(&path);

    let (code, stdout, stderr) =
        run(&["draw", "batch", path_s, "--script", script_s, "--in-place"]);
    assert_eq!(code, 0, "stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(stdout, "");
    assert_eq!(stderr, CLEAN_STDERR);
    assert!(path.exists(), "successful batch must write {path_s}");

    let (lint_code, lint_out, lint_err) = run(&["lint", path_s]);
    assert_eq!(lint_code, 0, "stdout={lint_out:?} stderr={lint_err:?}");
    assert!(
        lint_out.contains("errors: 0, warnings: 0, info: 0"),
        "lint stdout={lint_out:?}"
    );

    // basic_flow already contains client/api/svc. The script must modify that
    // document (new node + layout of existing ids) and write JSON to stdout.
    let stdin_script = temp_path("stdin-script.excal");
    let stdin_script_s = stdin_script.to_str().expect("utf8 path");
    std::fs::write(
        &stdin_script,
        "\
node --id db --label \"Database\" --x 880 --y 140
layout align --ids client,api,svc,db --axis middle
connect --from svc --to db --label \"SQL\"
",
    )
    .expect("write stdin script");
    let fixture = std::fs::read_to_string("tests/fixtures/basic_flow.excalidraw").expect("fixture");
    let (code, stdout, stderr) = run_stdin(
        &["draw", "batch", "-", "--script", stdin_script_s],
        &fixture,
    );
    assert_eq!(code, 0, "stdout-len={} stderr={stderr:?}", stdout.len());
    assert_eq!(
        stderr,
        "excal: added 2 item(s), updated 1 item(s), removed 0 item(s)\nexcal: lint after change: errors: 0, warnings: 0, info: 0\n"
    );
    assert!(
        stdout.starts_with('{'),
        "stdout must be JSON, got {stdout:?}"
    );
    let doc: serde_json::Value = serde_json::from_str(&stdout).expect("stdout json");
    let ids: Vec<&str> = doc["elements"]
        .as_array()
        .expect("elements")
        .iter()
        .filter_map(|el| el["id"].as_str())
        .collect();
    for id in ["client", "api", "svc", "db"] {
        assert!(ids.contains(&id), "stdin batch missing {id} in {ids:?}");
    }
    let _ = std::fs::remove_file(&stdin_script);

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&script);
}
