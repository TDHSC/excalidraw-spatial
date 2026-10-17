use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const FIXED_NOW_MS: &str = "1700000000000";

/// Bottom-up 2-zone payment architecture. Connectors are drawn before zones so
/// wrapping does not move geometry that was already persisted.
const ARCHITECTURE_SCRIPT: &str = "\
node --id pay_gw --label \"Pay GW\" --x 100 --y 100
node --id risk_decision --label \"Risk\" --shape diamond --right-of pay_gw --gap 120
node --id psp_router --label \"PSP Router\" --x 100 --y 420
node --id card_net --label \"Card Net\" --right-of psp_router --gap 200
node --id clearing --label \"Clearing\" --right-of card_net --gap 200
connect --from psp_router --to card_net --id a_req --label \"AUTH req\"
connect --from card_net --to psp_router --id a_resp --label \"200 OK\"
connect --from risk_decision --to psp_router --id a_cross --elbow
connect --from psp_router --to clearing --id a_bypass --via y=560
zone --id z_ingress --title \"Ingress\" --children pay_gw,risk_decision
zone --id z_settle --title \"Settlement\" --children psp_router,card_net,clearing
text --id note --text \"audit trail\" --x 100 --y 640
";

/// Same architecture without the parallel return arrow, so no persisted element
/// is mutated when the commands run one CLI process at a time.
const BOTTOM_UP_SCRIPT: &str = "\
node --id pay_gw --label \"Pay GW\" --x 100 --y 100
node --id risk_decision --label \"Risk\" --shape diamond --right-of pay_gw --gap 120
node --id psp_router --label \"PSP Router\" --x 100 --y 420
node --id card_net --label \"Card Net\" --right-of psp_router --gap 200
node --id clearing --label \"Clearing\" --right-of card_net --gap 200
connect --from psp_router --to card_net --id a_req --label \"AUTH req\"
connect --from risk_decision --to psp_router --id a_cross --elbow
connect --from psp_router --to clearing --id a_bypass --via y=560
zone --id z_ingress --title \"Ingress\" --children pay_gw,risk_decision
zone --id z_settle --title \"Settlement\" --children psp_router,card_net,clearing
text --id note --text \"audit trail\" --x 100 --y 640
";

const PARALLEL_SCRIPT: &str = "\
node --id psp_router --label \"PSP Router\" --x 100 --y 420
node --id card_net --label \"Card Net\" --right-of psp_router --gap 200
connect --from psp_router --to card_net --id a_req --label \"AUTH req\"
connect --from card_net --to psp_router --id a_resp --label \"200 OK\"
";

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_excal")
}

fn temp_path(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("excal-t40-{}-{}-{tag}", std::process::id(), nanos))
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

fn write_script(tag: &str, body: &str) -> (PathBuf, String) {
    let script = temp_path(tag);
    std::fs::write(&script, body).expect("write script");
    let script_s = script.to_str().expect("utf8 script path").to_string();
    (script, script_s)
}

fn fresh_file(tag: &str) -> (PathBuf, String) {
    let path = temp_path(tag);
    let _ = std::fs::remove_file(&path);
    assert!(!path.exists(), "scratch file must not exist yet");
    let path_s = path.to_str().expect("utf8 path").to_string();
    assert!(
        !path_s.contains(' '),
        "temp path must not contain spaces: {path_s}"
    );
    (path, path_s)
}

fn load_doc(path: &std::path::Path) -> excalidraw_spatial::schema::Document {
    let text = std::fs::read_to_string(path).expect("read authored file");
    excalidraw_spatial::schema::Document::parse(&text).expect("parse authored file")
}

fn element<'a>(doc: &'a excalidraw_spatial::schema::Document, id: &str) -> &'a serde_json::Value {
    doc.value["elements"]
        .as_array()
        .expect("elements")
        .iter()
        .find(|el| el["id"] == id)
        .unwrap_or_else(|| panic!("missing element {id}"))
}

fn num(value: &serde_json::Value) -> f64 {
    value
        .as_f64()
        .unwrap_or_else(|| panic!("not a number: {value}"))
}

fn parent_of(inspect: &str, id: &str) -> String {
    let line = inspect
        .lines()
        .find(|line| line.starts_with(&format!("- {id} ")))
        .unwrap_or_else(|| panic!("inspect has no component line for {id}:\n{inspect}"));
    let marker = " parent=";
    let start = line
        .find(marker)
        .unwrap_or_else(|| panic!("no parent in {line}"))
        + marker.len();
    line[start..]
        .split_whitespace()
        .next()
        .expect("parent token")
        .to_string()
}

fn connection_line<'a>(inspect: &'a str, id: &str) -> &'a str {
    inspect
        .lines()
        .find(|line| line.starts_with(&format!("- {id} ")))
        .unwrap_or_else(|| panic!("inspect has no connection line for {id}:\n{inspect}"))
}

fn assert_clean_lint(path: &str) {
    let (code, stdout, stderr) = run(&["lint", path]);
    assert_eq!(code, 0, "stdout={stdout:?} stderr={stderr:?}");
    assert!(
        stdout.contains("errors: 0, warnings: 0, info: 0"),
        "lint stdout={stdout:?}"
    );
}

fn assert_nothing_to_change(args: &[&str]) {
    let (code, stdout, stderr) = run(args);
    assert_eq!(code, 0, "args={args:?} stdout={stdout:?} stderr={stderr:?}");
    assert!(
        stderr.contains("excal: nothing to change"),
        "args={args:?} stderr={stderr:?}"
    );
}

fn batch_in_place(path: &str, script: &str) -> (i32, String, String) {
    let (script_path, script_s) = write_script("batch.excal", script);
    let result = run(&["draw", "batch", path, "--script", &script_s, "--in-place"]);
    let _ = std::fs::remove_file(script_path);
    result
}

/// Shell-style tokens so `--label "Pay GW"` stays one argument when each script
/// line is re-executed as its own `excal draw` process.
fn shell_tokens(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars().peekable();
    let mut in_token = false;
    while let Some(c) = chars.next() {
        match c {
            '"' | '\'' => {
                let quote = c;
                in_token = true;
                for ch in chars.by_ref() {
                    if ch == quote {
                        break;
                    }
                    cur.push(ch);
                }
            }
            c if c.is_ascii_whitespace() => {
                if in_token {
                    tokens.push(std::mem::take(&mut cur));
                    in_token = false;
                }
            }
            _ => {
                in_token = true;
                cur.push(c);
            }
        }
    }
    if in_token {
        tokens.push(cur);
    }
    tokens
}

fn incremental_in_place(path: &str, script: &str) -> Vec<(i32, String, String)> {
    script
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let tokens = shell_tokens(line);
            let mut args = vec!["draw".to_string(), tokens[0].clone(), path.to_string()];
            args.extend(tokens.into_iter().skip(1));
            args.push("--in-place".to_string());
            let argv: Vec<&str> = args.iter().map(String::as_str).collect();
            run(&argv)
        })
        .collect()
}

fn geometry_without_versions(raw: &str, ids: &[&str]) -> serde_json::Value {
    let mut value: serde_json::Value = serde_json::from_str(raw).expect("json");
    let elements = value["elements"].as_array_mut().expect("elements");
    for el in elements.iter_mut() {
        if !ids.iter().any(|id| el["id"] == *id) {
            continue;
        }
        if let Some(obj) = el.as_object_mut() {
            obj.remove("version");
            obj.remove("versionNonce");
        }
    }
    value
}

#[test]
fn from_scratch_multi_zone_architecture_via_batch() {
    let (path, path_s) = fresh_file("arch.excalidraw");
    let (code, stdout, stderr) = batch_in_place(&path_s, ARCHITECTURE_SCRIPT);
    assert_eq!(code, 0, "stdout={stdout:?} stderr={stderr:?}");
    assert!(
        stderr.contains("excal: lint after change: errors: 0, warnings: 0, info: 0"),
        "batch stderr={stderr:?}"
    );
    assert!(
        path.exists(),
        "batch must create {path_s} from a missing path"
    );

    assert_clean_lint(&path_s);

    let (inspect_code, inspect, inspect_err) = run(&["inspect", &path_s]);
    assert_eq!(inspect_code, 0, "stderr={inspect_err:?}");
    assert!(
        inspect.contains("- z_ingress zone \"Ingress\""),
        "missing ingress zone:\n{inspect}"
    );
    assert!(
        inspect.contains("- z_settle zone \"Settlement\""),
        "missing settlement zone:\n{inspect}"
    );
    for (id, parent) in [
        ("pay_gw", "z_ingress"),
        ("risk_decision", "z_ingress"),
        ("psp_router", "z_settle"),
        ("card_net", "z_settle"),
        ("clearing", "z_settle"),
    ] {
        assert_eq!(parent_of(&inspect, id), parent, "inspect:\n{inspect}");
    }
    for id in ["a_req", "a_resp", "a_cross", "a_bypass"] {
        let line = connection_line(&inspect, id);
        assert!(
            !line.contains("free") && !line.contains("near:"),
            "{id} endpoints are not both Bound: {line}"
        );
    }
    assert!(
        connection_line(&inspect, "a_req").contains("\"AUTH req\""),
        "a_req label missing"
    );
    assert!(
        connection_line(&inspect, "a_resp").contains("\"200 OK\""),
        "a_resp label missing"
    );
    assert!(
        connection_line(&inspect, "a_cross").contains(" elbow "),
        "cross-zone connector must be elbowed"
    );

    let before = std::fs::read(&path).expect("read before no-op layout");
    assert_nothing_to_change(&["layout", "fit-text", &path_s, "--all"]);
    assert_nothing_to_change(&[
        "layout",
        "fit-container",
        &path_s,
        "--id",
        "z_ingress",
        "--expand-only",
    ]);
    let after = std::fs::read(&path).expect("read after no-op layout");
    assert_eq!(before, after, "no-op layout must not rewrite the file");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn incremental_cli_build_matches_batch() {
    let (batch_path, batch_s) = fresh_file("bottom-batch.excalidraw");
    let (inc_path, inc_s) = fresh_file("bottom-inc.excalidraw");

    let (code, stdout, stderr) = batch_in_place(&batch_s, BOTTOM_UP_SCRIPT);
    assert_eq!(code, 0, "batch stdout={stdout:?} stderr={stderr:?}");
    let steps = incremental_in_place(&inc_s, BOTTOM_UP_SCRIPT);
    for (code, stdout, stderr) in &steps {
        assert_eq!(*code, 0, "incremental stdout={stdout:?} stderr={stderr:?}");
    }

    let batch_bytes = std::fs::read(&batch_path).expect("batch file");
    let inc_bytes = std::fs::read(&inc_path).expect("incremental file");
    assert_eq!(
        batch_bytes, inc_bytes,
        "bottom-up incremental CLI must match draw batch byte-for-byte"
    );

    let (par_batch, par_batch_s) = fresh_file("par-batch.excalidraw");
    let (par_inc, par_inc_s) = fresh_file("par-inc.excalidraw");
    let (code, stdout, stderr) = batch_in_place(&par_batch_s, PARALLEL_SCRIPT);
    assert_eq!(
        code, 0,
        "parallel batch stdout={stdout:?} stderr={stderr:?}"
    );
    let steps = incremental_in_place(&par_inc_s, PARALLEL_SCRIPT);
    for (code, stdout, stderr) in &steps {
        assert_eq!(
            *code, 0,
            "parallel incremental stdout={stdout:?} stderr={stderr:?}"
        );
    }

    let batch_raw = std::fs::read_to_string(&par_batch).expect("parallel batch");
    let inc_raw = std::fs::read_to_string(&par_inc).expect("parallel incremental");
    assert_ne!(
        batch_raw, inc_raw,
        "re-spreading a persisted a_req must change version metadata"
    );
    assert_eq!(
        geometry_without_versions(&batch_raw, &["a_req", "a_req-label"]),
        geometry_without_versions(&inc_raw, &["a_req", "a_req-label"]),
        "parallel incremental and batch must match except a_req version metadata"
    );

    let batch_doc = load_doc(&par_batch);
    let inc_doc = load_doc(&par_inc);
    for id in ["a_req", "a_req-label"] {
        assert_eq!(element(&batch_doc, id)["version"], 1, "{id} batch version");
        assert!(
            num(&element(&inc_doc, id)["version"]) > 1.0,
            "{id} incremental version must advance after the second connect"
        );
        assert_ne!(
            element(&batch_doc, id)["versionNonce"],
            element(&inc_doc, id)["versionNonce"],
            "{id} versionNonce must differ"
        );
    }

    let _ = std::fs::remove_file(batch_path);
    let _ = std::fs::remove_file(inc_path);
    let _ = std::fs::remove_file(par_batch);
    let _ = std::fs::remove_file(par_inc);
}

#[test]
fn stderr_actionable_lint_closed_loop_repair() {
    let (path, path_s) = fresh_file("repair.excalidraw");

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        &path_s,
        "--id",
        "api_gw",
        "--label",
        "API",
        "--x",
        "100",
        "--y",
        "100",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stdout={stdout:?} stderr={stderr:?}");

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        &path_s,
        "--id",
        "auth",
        "--label",
        "Auth",
        "--right-of",
        "api_gw",
        "--gap",
        "40",
        "--in-place",
    ]);
    assert_eq!(code, 0, "stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(num(&element(&load_doc(&path), "auth")["x"]), 300.0);

    let (code, stdout, stderr) = run(&[
        "draw",
        "connect",
        &path_s,
        "--from",
        "api_gw",
        "--to",
        "auth",
        "--id",
        "a1",
        "--label",
        "validateTokenAndPermissions()",
        "--in-place",
    ]);
    assert_eq!(code, 0, "connect stdout={stdout:?} stderr={stderr:?}");
    assert!(
        stderr.contains("ERROR ARROW_LABEL_COLLISION"),
        "stderr={stderr:?}"
    );
    let expected_run = format!("run: excal layout move {path_s} --id auth --dx 259");
    assert!(
        stderr.contains(&expected_run),
        "stderr missing {expected_run:?}:\n{stderr}"
    );

    let run_line = stderr
        .lines()
        .find(|line| line.trim_start().starts_with("run: "))
        .expect("stderr run: line")
        .trim();
    let command = run_line.trim_start_matches("run: ").trim();
    assert!(command.starts_with("excal "), "run command={command:?}");
    let mut args: Vec<&str> = command.split_whitespace().skip(1).collect();
    args.push("--in-place");
    let (code, stdout, fix_err) = run(&args);
    assert_eq!(code, 0, "fix stdout={stdout:?} stderr={fix_err:?}");
    assert_eq!(num(&element(&load_doc(&path), "auth")["x"]), 559.0);
    assert_clean_lint(&path_s);

    let (code, stdout, stderr) = run(&[
        "draw",
        "node",
        &path_s,
        "--id",
        "db",
        "--label",
        "DB",
        "--right-of",
        "auth",
        "--gap",
        "60",
        "--in-place",
    ]);
    assert_eq!(code, 0, "db stdout={stdout:?} stderr={stderr:?}");
    assert!(stderr.contains("INFO  UNEVEN_SPACING"), "stderr={stderr:?}");
    let expected_dist = format!(
        "run: excal layout distribute {path_s} --ids api_gw,auth,db --direction horizontal --gap 299"
    );
    assert!(
        stderr.contains(&expected_dist),
        "stderr missing {expected_dist:?}:\n{stderr}"
    );
    let run_line = stderr
        .lines()
        .find(|line| line.contains("UNEVEN_SPACING") || line.trim_start().starts_with("run: "))
        .and_then(|_| {
            stderr
                .lines()
                .filter(|line| line.trim_start().starts_with("run: "))
                .find(|line| line.contains("distribute"))
        })
        .expect("distribute run: line");
    let command = run_line.trim().trim_start_matches("run: ").trim();
    let mut args: Vec<&str> = command.split_whitespace().skip(1).collect();
    args.push("--in-place");
    let (code, stdout, fix_err) = run(&args);
    assert_eq!(code, 0, "distribute stdout={stdout:?} stderr={fix_err:?}");
    assert_clean_lint(&path_s);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn byte_for_byte_determinism() {
    let (first, first_s) = fresh_file("det-a.excalidraw");
    let (second, second_s) = fresh_file("det-b.excalidraw");
    let (code, stdout, stderr) = batch_in_place(&first_s, ARCHITECTURE_SCRIPT);
    assert_eq!(code, 0, "first stdout={stdout:?} stderr={stderr:?}");
    let (code, stdout, stderr) = batch_in_place(&second_s, ARCHITECTURE_SCRIPT);
    assert_eq!(code, 0, "second stdout={stdout:?} stderr={stderr:?}");

    let a = std::fs::read(&first).expect("first file");
    let b = std::fs::read(&second).expect("second file");
    assert_eq!(
        a, b,
        "identical batch scripts must be byte-for-byte identical"
    );
    assert!(a.ends_with(b"\n"), "authored file must end with a newline");

    let _ = std::fs::remove_file(first);
    let _ = std::fs::remove_file(second);
}
