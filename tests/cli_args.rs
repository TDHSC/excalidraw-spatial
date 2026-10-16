use std::process::Command;

fn run(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_excal"))
        .args(args)
        .output()
        .expect("failed to execute excal");
    let code = output.status.code().expect("process terminated by signal");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (code, stdout, stderr)
}

fn stderr_is_not_implemented(stderr: &str) -> bool {
    matches!(
        stderr.strip_suffix('\n').unwrap_or(stderr),
        "excal: error: not implemented"
    )
}

#[test]
fn help_lists_top_level_commands() {
    let (code, stdout, _stderr) = run(&["--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("inspect"),
        "stdout missing inspect:\n{stdout}"
    );
    assert!(stdout.contains("lint"), "stdout missing lint:\n{stdout}");
    assert!(
        stdout.contains("layout"),
        "stdout missing layout:\n{stdout}"
    );
}

#[test]
fn layout_help_lists_layout_commands() {
    let (code, stdout, _stderr) = run(&["layout", "--help"]);
    assert_eq!(code, 0);
    for name in [
        "move",
        "align",
        "distribute",
        "place",
        "fit-container",
        "fit-text",
    ] {
        assert!(stdout.contains(name), "stdout missing {name}:\n{stdout}");
    }
}

#[test]
fn move_accepts_negative_dy_and_is_not_implemented() {
    let (code, _stdout, stderr) =
        run(&["layout", "move", "f.excalidraw", "--id", "a", "--dy", "-20"]);
    assert_eq!(code, 2);
    assert!(
        stderr_is_not_implemented(&stderr),
        "expected exactly 'excal: error: not implemented' (optional trailing newline), got {stderr:?}"
    );
}

#[test]
fn place_rejects_two_direction_flags() {
    let (code, _stdout, stderr) = run(&[
        "layout",
        "place",
        "f.excalidraw",
        "--id",
        "a",
        "--right-of",
        "b",
        "--below",
        "c",
    ]);
    assert_eq!(code, 2);
    assert!(
        !stderr.contains("not implemented"),
        "clap error must not print not implemented, got {stderr:?}"
    );
}

#[test]
fn place_requires_a_direction_flag() {
    let (code, _stdout, stderr) = run(&["layout", "place", "f.excalidraw", "--id", "a"]);
    assert_eq!(code, 2);
    assert!(
        !stderr.contains("not implemented"),
        "clap error must not print not implemented, got {stderr:?}"
    );
}

#[test]
fn move_rejects_in_place_with_output_path() {
    let (code, _stdout, stderr) = run(&[
        "layout",
        "move",
        "f.excalidraw",
        "--id",
        "a",
        "--in-place",
        "-o",
        "x.json",
    ]);
    assert_eq!(code, 2);
    assert!(
        !stderr.contains("not implemented"),
        "clap error must not print not implemented, got {stderr:?}"
    );
}

#[test]
fn fit_text_requires_id_or_all() {
    let (code, _stdout, stderr) = run(&["layout", "fit-text", "f.excalidraw"]);
    assert_eq!(code, 2);
    assert!(
        !stderr.contains("not implemented"),
        "clap error must not print not implemented, got {stderr:?}"
    );
}
