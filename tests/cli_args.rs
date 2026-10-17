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
fn move_accepts_negative_dy_and_fails_on_missing_file() {
    let (code, _stdout, stderr) =
        run(&["layout", "move", "f.excalidraw", "--id", "a", "--dy", "-20"]);
    assert_eq!(code, 2);
    assert!(
        stderr.starts_with("excal: error:"),
        "negative dy must parse (clap errors do not use this prefix); got {stderr:?}"
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

#[test]
fn help_lists_draw_among_top_level_commands() {
    let (code, stdout, _stderr) = run(&["--help"]);
    assert_eq!(code, 0);
    for name in ["draw", "inspect", "lint", "layout"] {
        assert!(stdout.contains(name), "stdout missing {name}:\n{stdout}");
    }
}

#[test]
fn draw_help_lists_authoring_commands() {
    let (code, stdout, _stderr) = run(&["draw", "--help"]);
    assert_eq!(code, 0);
    for name in [
        "init", "node", "connect", "route", "zone", "frame", "text", "label", "remove", "batch",
    ] {
        assert!(stdout.contains(name), "stdout missing {name}:\n{stdout}");
    }
}

#[test]
fn draw_subcommand_aliases_parse() {
    for args in [
        &["draw", "arrow", "--help"][..],
        &["draw", "rm", "--help"][..],
        &["draw", "delete", "--help"][..],
        &["draw", "reroute", "--help"][..],
    ] {
        let (code, stdout, stderr) = run(args);
        assert_eq!(code, 0, "args={args:?} stdout={stdout:?} stderr={stderr:?}");
    }
}

#[test]
fn draw_node_requires_y_with_x() {
    let (code, _stdout, stderr) = run(&["draw", "node", "f.excalidraw", "--id", "a", "--x", "10"]);
    assert_eq!(code, 2);
    assert!(
        !stderr.contains("not implemented"),
        "clap error must not print not implemented, got {stderr:?}"
    );
}

#[test]
fn draw_node_rejects_explicit_coords_with_relative_direction() {
    let (code, _stdout, stderr) = run(&[
        "draw",
        "node",
        "f.excalidraw",
        "--id",
        "a",
        "--x",
        "10",
        "--y",
        "20",
        "--right-of",
        "b",
    ]);
    assert_eq!(code, 2);
    assert!(
        !stderr.contains("not implemented"),
        "clap error must not print not implemented, got {stderr:?}"
    );
}

#[test]
fn draw_node_rejects_multiple_relative_directions() {
    let (code, _stdout, stderr) = run(&[
        "draw",
        "node",
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
fn draw_zone_requires_children_or_rect() {
    let (code, _stdout, stderr) =
        run(&["draw", "zone", "f.excalidraw", "--id", "z", "--title", "T"]);
    assert_eq!(code, 2);
    assert!(
        !stderr.contains("not implemented"),
        "clap error must not print not implemented, got {stderr:?}"
    );
}

#[test]
fn draw_zone_rejects_children_with_explicit_rect() {
    let (code, _stdout, stderr) = run(&[
        "draw",
        "zone",
        "f.excalidraw",
        "--id",
        "z",
        "--title",
        "T",
        "--children",
        "a",
        "--x",
        "0",
        "--y",
        "0",
        "--width",
        "100",
        "--height",
        "100",
    ]);
    assert_eq!(code, 2);
    assert!(
        !stderr.contains("not implemented"),
        "clap error must not print not implemented, got {stderr:?}"
    );
}

#[test]
fn draw_frame_requires_children_or_rect() {
    let (code, _stdout, stderr) = run(&["draw", "frame", "f.excalidraw", "--id", "f"]);
    assert_eq!(code, 2);
    assert!(
        !stderr.contains("not implemented"),
        "clap error must not print not implemented, got {stderr:?}"
    );
}

#[test]
fn draw_node_rejects_in_place_with_output_path() {
    let (code, _stdout, stderr) = run(&[
        "draw",
        "node",
        "f.excalidraw",
        "--id",
        "a",
        "--in-place",
        "-o",
        "out.json",
    ]);
    assert_eq!(code, 2);
    assert!(
        !stderr.contains("not implemented"),
        "clap error must not print not implemented, got {stderr:?}"
    );
}
