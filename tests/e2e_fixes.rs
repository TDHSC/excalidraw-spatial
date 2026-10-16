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
