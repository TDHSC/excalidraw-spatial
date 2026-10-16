use std::fs::File;
use std::process::{Command, Stdio};

use excalidraw_spatial::formatter::ai_report::format_inspect;
use excalidraw_spatial::formatter::ascii_map::render;
use excalidraw_spatial::geometry::{Point, Rect};
use excalidraw_spatial::scene_graph::{
    Attach, Component, ComponentKind, Connector, Container, ContainerKind, Scene,
};
const FIXTURES: [&str; 6] = [
    "arrows",
    "basic_flow",
    "icon_group",
    "overlap",
    "swimlanes",
    "text_layout",
];

fn run(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_excal"))
        .args(args)
        .output()
        .expect("failed to execute excal");
    let code = output.status.code().expect("process terminated by signal");
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr utf-8");
    (code, stdout, stderr)
}

fn scene(kind: ComponentKind, id: &str, rect: (f64, f64, f64, f64)) -> Scene {
    Scene {
        components: vec![component(kind, id, rect)],
        containers: Vec::new(),
        connectors: Vec::new(),
        notes: Vec::new(),
    }
}

fn component(kind: ComponentKind, id: &str, rect: (f64, f64, f64, f64)) -> Component {
    Component {
        id: id.to_string(),
        kind,
        type_name: match kind {
            ComponentKind::Shape => "rectangle".to_string(),
            ComponentKind::Group => "group".to_string(),
            ComponentKind::Text => "text".to_string(),
        },
        rect: Rect::from_xywh(rect.0, rect.1, rect.2, rect.3),
        label: None,
        label_element: None,
        label_size: None,
        elements: vec![id.to_string()],
        parent: None,
        frame_id: None,
        z: 0,
    }
}

#[test]
fn inspect_all_fixtures_match_golden_and_exit_0() {
    for name in FIXTURES {
        let (code, stdout, stderr) =
            run(&["inspect", &format!("tests/fixtures/{name}.excalidraw")]);
        let expected = std::fs::read_to_string(format!("tests/expected/{name}.inspect.txt"))
            .unwrap_or_else(|err| panic!("read expected {name}: {err}"));
        assert_eq!(code, 0, "{name} stderr:\n{stderr}");
        assert_eq!(stdout, expected, "{name}");
    }
}

#[test]
fn inspect_basic_flow_ascii_is_five_map_lines() {
    let (code, stdout, stderr) = run(&[
        "inspect",
        "tests/fixtures/basic_flow.excalidraw",
        "--format",
        "ascii",
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        stdout,
        "\
AAAAAAAAAAAAAAAA        BBBBBBBBBBBBBBBB        CCCCCCCCCCCCCCCC
AAAAAAAAAAAAAAAA********BBBBBBBBBBBBBBBB********CCCCCCCCCCCCCCCC
AAAAAAAAAAAAAAAA        BBBBBBBBBBBBBBBB        CCCCCCCCCCCCCCCC
AAAAAAAAAAAAAAAA        BBBBBBBBBBBBBBBB        CCCCCCCCCCCCCCCC
A=client B=api C=svc
"
    );
}

#[test]
fn inspect_stdin_title_uses_stdin_and_run_commands_use_file() {
    let fixture = File::open("tests/fixtures/basic_flow.excalidraw").expect("open fixture");
    let output = Command::new(env!("CARGO_BIN_EXE_excal"))
        .args(["inspect", "-"])
        .stdin(Stdio::from(fixture))
        .output()
        .expect("failed to execute excal");
    let code = output.status.code().expect("process terminated by signal");
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    assert_eq!(code, 0);
    let expected = include_str!("expected/basic_flow.inspect.txt").replacen(
        "tests/fixtures/basic_flow.excalidraw",
        "<stdin>",
        1,
    );
    assert_eq!(stdout, expected);
    assert!(stdout.starts_with("# excal inspect <stdin>\n"));
}

#[test]
fn inspect_empty_document_reports_empty_canvas_and_none_sections() {
    let path = std::env::temp_dir().join("excal-inspect-empty.excalidraw");
    std::fs::write(&path, r#"{"type":"excalidraw","version":2,"elements":[]}"#)
        .expect("write empty fixture");
    let (code, stdout, stderr) = run(&["inspect", path.to_str().expect("path")]);
    assert_eq!(code, 0, "{stderr}");
    let mut lines = stdout.lines();
    assert!(lines.next().unwrap().starts_with("# excal inspect "));
    assert_eq!(
        lines.next(),
        Some("canvas (empty) | components: 0, containers: 0, connectors: 0 | errors: 0, warnings: 0, info: 0")
    );
    let rest: Vec<_> = lines
        .filter(|line| !line.is_empty() && !line.starts_with("## "))
        .collect();
    assert_eq!(
        rest,
        ["(none)", "(none)", "(none)", "(none)", "(none)", "(none)", "(none)"]
    );
}

#[test]
fn inspect_json_stays_unimplemented() {
    let (code, _stdout, stderr) = run(&[
        "inspect",
        "tests/fixtures/basic_flow.excalidraw",
        "--format",
        "json",
    ]);
    assert_eq!(code, 2);
    assert!(
        stderr.contains("not implemented"),
        "json inspect belongs to T14, got {stderr:?}"
    );
}

#[test]
fn format_inspect_omits_neighbor_suffix_for_text_components() {
    let report = format_inspect(
        "note.excalidraw",
        &scene(ComponentKind::Text, "note", (0.0, 0.0, 40.0, 20.0)),
        &[],
    );
    let component = report
        .lines()
        .find(|line| line.starts_with("- note "))
        .expect("text component line");
    assert!(component.contains("parent=-"));
    assert!(
        !component.contains(" L="),
        "text components stop after parent, got {component}"
    );
}

#[test]
fn format_inspect_includes_neighbor_suffix_for_shapes_and_groups() {
    let mut group = scene(ComponentKind::Group, "icon", (0.0, 0.0, 40.0, 40.0));
    group.components.push(component(
        ComponentKind::Shape,
        "box",
        (80.0, 0.0, 40.0, 40.0),
    ));
    group.components[1].z = 1;
    let report = format_inspect("g.excalidraw", &group, &[]);
    let icon = report
        .lines()
        .find(|line| line.starts_with("- icon "))
        .expect("group line");
    assert!(
        icon.contains("parent=- L=- R=box(40) T=- B=-"),
        "group is a node and keeps neighbors, got {icon}"
    );
}

#[test]
fn format_inspect_orders_tracks_top_then_container_z_then_pos() {
    let mut built = scene(ComponentKind::Shape, "late-a", (0.0, 200.0, 40.0, 40.0));
    built.components[0].parent = Some("zone-b".to_string());
    built.components[0].z = 3;
    built.components.push(component(
        ComponentKind::Shape,
        "late-b",
        (80.0, 200.0, 40.0, 40.0),
    ));
    built.components[1].parent = Some("zone-b".to_string());
    built.components[1].z = 4;
    built.components.push(component(
        ComponentKind::Shape,
        "early-a",
        (0.0, 0.0, 40.0, 40.0),
    ));
    built.components[2].parent = Some("zone-a".to_string());
    built.components[2].z = 1;
    built.components.push(component(
        ComponentKind::Shape,
        "early-b",
        (80.0, 0.0, 40.0, 40.0),
    ));
    built.components[3].parent = Some("zone-a".to_string());
    built.components[3].z = 2;
    built.components.push(component(
        ComponentKind::Shape,
        "top-a",
        (0.0, 400.0, 40.0, 40.0),
    ));
    built.components[4].z = 5;
    built.components.push(component(
        ComponentKind::Shape,
        "top-b",
        (0.0, 480.0, 40.0, 40.0),
    ));
    built.components[5].z = 6;
    built.containers.push(Container {
        id: "zone-b".to_string(),
        kind: ContainerKind::Zone,
        rect: Rect::from_xywh(0.0, 180.0, 200.0, 80.0),
        name: None,
        title_element: None,
        title_height: None,
        elements: vec!["zone-b".to_string()],
        parent: None,
        frame_id: None,
        z: 10,
    });
    built.containers.push(Container {
        id: "zone-a".to_string(),
        kind: ContainerKind::Zone,
        rect: Rect::from_xywh(0.0, 0.0, 200.0, 80.0),
        name: None,
        title_element: None,
        title_height: None,
        elements: vec!["zone-a".to_string()],
        parent: None,
        frame_id: None,
        z: 0,
    });
    let report = format_inspect("tracks.excalidraw", &built, &[]);
    let rows: Vec<_> = report
        .lines()
        .skip_while(|line| *line != "## Rows")
        .skip(1)
        .take_while(|line| !line.is_empty())
        .collect();
    let columns: Vec<_> = report
        .lines()
        .skip_while(|line| *line != "## Columns")
        .skip(1)
        .take_while(|line| !line.is_empty())
        .collect();
    assert_eq!(
        rows,
        [
            "- zone-a y≈20: early-a --40-- early-b",
            "- zone-b y≈220: late-a --40-- late-b",
        ],
        "container rows follow container z, not insertion order"
    );
    assert_eq!(columns, ["- top x≈20: top-a --40-- top-b"]);
}

#[test]
fn render_minimap_is_none_when_there_are_no_components() {
    let empty = Scene {
        components: Vec::new(),
        containers: vec![Container {
            id: "frame".to_string(),
            kind: ContainerKind::Frame,
            rect: Rect::from_xywh(0.0, 0.0, 100.0, 50.0),
            name: Some("Frame".to_string()),
            title_element: None,
            title_height: None,
            elements: vec!["frame".to_string()],
            parent: None,
            frame_id: None,
            z: 0,
        }],
        connectors: vec![Connector {
            id: "line".to_string(),
            kind: "line".to_string(),
            elbowed: false,
            points: vec![Point { x: 0.0, y: 0.0 }, Point { x: 100.0, y: 50.0 }],
            start: Attach::Free,
            end: Attach::Free,
            label: None,
            label_element: None,
            label_rect: None,
            z: 1,
        }],
        notes: Vec::new(),
    };
    assert!(render(&empty).is_empty());
    let report = format_inspect("frame-only.excalidraw", &empty, &[]);
    let minimap: Vec<_> = report
        .lines()
        .skip_while(|line| *line != "## Minimap")
        .skip(1)
        .take_while(|line| !line.is_empty())
        .collect();
    assert_eq!(minimap, ["(none)"]);
}

#[test]
fn render_minimap_legend_uses_hash_after_sixty_two_components() {
    let mut built = Scene {
        components: Vec::new(),
        containers: Vec::new(),
        connectors: Vec::new(),
        notes: Vec::new(),
    };
    for i in 0..63 {
        let mut item = component(
            ComponentKind::Shape,
            &format!("n{i}"),
            (i as f64, 0.0, 1.0, 1.0),
        );
        item.z = i;
        built.components.push(item);
    }
    let lines = render(&built);
    let legend = lines.last().expect("legend");
    assert!(legend.starts_with("A=n0 "));
    assert!(legend.contains(" Z=n25 "));
    assert!(legend.contains(" a=n26 "));
    assert!(legend.contains(" 9=n61 "));
    assert!(legend.ends_with(" #=n62"), "got {legend}");
}

#[test]
fn format_inspect_empty_scene_matches_spec_sections() {
    let empty = Scene {
        components: Vec::new(),
        containers: Vec::new(),
        connectors: Vec::new(),
        notes: Vec::new(),
    };
    let report = format_inspect("<stdin>", &empty, &[]);
    assert_eq!(
        report,
        "\
# excal inspect <stdin>
canvas (empty) | components: 0, containers: 0, connectors: 0 | errors: 0, warnings: 0, info: 0

## Containers
(none)

## Components
(none)

## Rows
(none)

## Columns
(none)

## Connections
(none)

## Minimap
(none)

## Issues
(none)
"
    );
}
