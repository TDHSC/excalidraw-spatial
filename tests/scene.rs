use excalidraw_spatial::geometry::{Point, Rect};
use excalidraw_spatial::linter::Severity;
use excalidraw_spatial::scene_graph::{Attach, ComponentKind, ContainerKind, Scene};
use excalidraw_spatial::schema::Document;

fn load(name: &str) -> Document {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{name}.excalidraw"));
    let src = std::fs::read_to_string(path).expect("fixture");
    Document::parse(&src).expect("parse fixture")
}

fn approx_eq(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

fn assert_rect(rect: Rect, expected: (f64, f64, f64, f64)) {
    approx_eq(rect.x1, expected.0);
    approx_eq(rect.y1, expected.1);
    approx_eq(rect.x2, expected.2);
    approx_eq(rect.y2, expected.3);
}

fn assert_points(points: &[Point], expected: &[(f64, f64)]) {
    assert_eq!(points.len(), expected.len());
    for (point, (x, y)) in points.iter().zip(expected) {
        approx_eq(point.x, *x);
        approx_eq(point.y, *y);
    }
}

#[test]
fn basic_flow_shapes_are_labeled_rectangles() {
    let scene = Scene::build(&load("basic_flow"));

    assert!(scene.containers.is_empty());
    assert!(scene.notes.is_empty());
    let ids: Vec<_> = scene.components.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["client", "api", "svc"]);

    for component in &scene.components {
        assert_eq!(component.kind, ComponentKind::Shape);
        assert_eq!(component.type_name, "rectangle");
        assert!(component.parent.is_none());
    }

    let api = scene.component("api").expect("api");
    assert_eq!(api.label.as_deref(), Some("API Gateway"));
    assert_eq!(api.label_element.as_deref(), Some("api-label"));
    assert_rect(api.rect, (360.0, 100.0, 520.0, 180.0));
    assert_eq!(api.elements, ["api", "api-label"]);
}

#[test]
fn basic_flow_connectors_bind_both_ends_and_keep_labels() {
    let scene = Scene::build(&load("basic_flow"));
    let ids: Vec<_> = scene.connectors.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["a1", "a2"]);

    let a1 = scene.connector("a1").expect("a1");
    assert_eq!(
        a1.start,
        Attach::Bound {
            target: "client".to_string(),
            element: "client".to_string(),
        }
    );
    assert_eq!(
        a1.end,
        Attach::Bound {
            target: "api".to_string(),
            element: "api".to_string(),
        }
    );
    assert_eq!(a1.label.as_deref(), Some("HTTPS"));
    let label_rect = a1.label_rect.expect("a1 label rect");
    assert_rect(label_rect, (289.2, 130.0, 330.8, 150.0));
    assert_points(&a1.points, &[(266.0, 140.0), (354.0, 140.0)]);
}

#[test]
fn arrows_distinguish_free_ends_from_near_elbows() {
    let scene = Scene::build(&load("arrows"));
    let dangling = scene.connector("arr_dangling").expect("arr_dangling");
    assert_eq!(dangling.end, Attach::Free);

    let near = scene.connector("arr_near").expect("arr_near");
    assert!(near.elbowed);
    assert_eq!(
        near.end,
        Attach::Near {
            target: "d".to_string(),
            distance: 8.0,
        }
    );
}

#[test]
fn overlap_has_two_shapes_and_no_connectors() {
    let scene = Scene::build(&load("overlap"));
    let ids: Vec<_> = scene.components.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["api", "auth"]);
    assert!(scene.connectors.is_empty());
    assert!(scene.containers.is_empty());
}

#[test]
fn icon_group_builds_atomic_group_and_ungrouped_shapes() {
    let scene = Scene::build(&load("icon_group"));
    let ids: Vec<_> = scene.components.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "group:kq3Xv9TzL2mPw8RbN5cYd",
            "Mx9Cv2Bn5Lk8Jh1Gf4DsA",
            "Ab1Cd2Ef3Gh4Ij5Kl6MnO",
        ]
    );
    let group = scene
        .component("group:kq3Xv9TzL2mPw8RbN5cYd")
        .expect("postgres group");
    assert_eq!(group.kind, ComponentKind::Group);
    assert_eq!(group.type_name, "group");
    assert_eq!(group.label.as_deref(), Some("Postgres"));
    assert!(group.label_element.is_none());
    assert_rect(group.rect, (58.4, 80.0, 141.6, 209.0));
    assert_eq!(
        group.elements,
        [
            "V1sQx7Kd2LpW9mZa4RtYc",
            "Hn6Bf3Jq8XcV1sMz5LwPe",
            "Pz4Wk9Ry2TcN7bQx3MfLa",
            "Ty8Lm3Qv6ZpR1cXw9KdBn",
            "Rc2Np7Xw4LqZ9vBk6MtYs",
            "Qw5Er8Ty1Ui4Op7As0DfG",
        ]
    );

    let image = scene
        .component("Ab1Cd2Ef3Gh4Ij5Kl6MnO")
        .expect("ungrouped image");
    assert_eq!(image.kind, ComponentKind::Shape);
    assert_eq!(image.type_name, "image");
}

#[test]
fn icon_group_arrow_binds_to_group_via_member_shape() {
    let scene = Scene::build(&load("icon_group"));
    let arrow = scene
        .connector("Lg7Hs4Kd1Fp8Wq5Er2TyU")
        .expect("group arrow");
    assert_eq!(
        arrow.end,
        Attach::Bound {
            target: "group:kq3Xv9TzL2mPw8RbN5cYd".to_string(),
            element: "Hn6Bf3Jq8XcV1sMz5LwPe".to_string(),
        }
    );
}

#[test]
fn text_layout_absorbs_unbound_text_into_host_shape() {
    let scene = Scene::build(&load("text_layout"));
    assert_eq!(scene.components.len(), 4);

    let n4 = scene.component("n4").expect("n4");
    assert_eq!(n4.label.as_deref(), Some("缓存 Cache"));
    assert_eq!(n4.label_element.as_deref(), Some("n4-text"));
    assert_eq!(n4.elements, ["n4", "n4-text"]);

    assert_eq!(scene.notes.len(), 1);
    let note = &scene.notes[0];
    assert_eq!(note.rule, "UNBOUND_TEXT");
    assert_eq!(note.severity, Severity::Info);
    assert_eq!(note.targets, ["n4-text", "n4"]);
    assert_eq!(
        note.message,
        "text n4-text \"缓存 Cache\" sits inside n4 but is not bound to it"
    );
    assert_eq!(
        note.fix.as_deref(),
        Some("bind it as the label of n4 (set containerId)")
    );
    assert!(note.command.is_none());
}

fn parse_scene(src: &str) -> Scene {
    Scene::build(&Document::parse(src).expect("parse constructed document"))
}

#[test]
fn group_with_zone_like_children_is_dissolved() {
    let scene = parse_scene(
        r#"{
            "elements": [
                {
                    "id": "bg",
                    "type": "rectangle",
                    "x": 0,
                    "y": 0,
                    "width": 400,
                    "height": 300,
                    "groupIds": ["g1"]
                },
                {
                    "id": "a",
                    "type": "rectangle",
                    "x": 20,
                    "y": 20,
                    "width": 100,
                    "height": 80,
                    "groupIds": ["g1"]
                },
                {
                    "id": "b",
                    "type": "rectangle",
                    "x": 200,
                    "y": 40,
                    "width": 100,
                    "height": 80,
                    "groupIds": ["g1"]
                }
            ]
        }"#,
    );

    assert!(scene
        .components
        .iter()
        .all(|component| component.kind != ComponentKind::Group));
    let ids: Vec<_> = scene.components.iter().map(|c| c.id.as_str()).collect();
    assert!(ids.contains(&"a"));
    assert!(ids.contains(&"b"));
    assert_eq!(scene.component("a").unwrap().kind, ComponentKind::Shape);
    assert_eq!(scene.component("b").unwrap().kind, ComponentKind::Shape);
}

#[test]
fn overlapping_free_text_with_center_outside_stays_text_component() {
    // Rectangle (0,0)-(100,40). Text (80,0)-(160,20) overlaps the right edge,
    // but its center (120, 10) is outside the rectangle.
    let scene = parse_scene(
        r#"{
            "elements": [
                {
                    "id": "box",
                    "type": "rectangle",
                    "x": 0,
                    "y": 0,
                    "width": 100,
                    "height": 40
                },
                {
                    "id": "caption",
                    "type": "text",
                    "x": 80,
                    "y": 0,
                    "width": 80,
                    "height": 20,
                    "text": "outside"
                }
            ]
        }"#,
    );

    let box_shape = scene.component("box").expect("box");
    assert_eq!(box_shape.kind, ComponentKind::Shape);
    assert!(box_shape.label.is_none());
    assert_eq!(box_shape.elements, ["box"]);

    let caption = scene.component("caption").expect("caption");
    assert_eq!(caption.kind, ComponentKind::Text);
    assert_eq!(caption.label.as_deref(), Some("outside"));
    assert_eq!(caption.label_element.as_deref(), Some("caption"));
    assert!(scene.notes.is_empty());
}

#[test]
fn already_labeled_shape_absorbs_inner_free_text_without_note() {
    let scene = parse_scene(
        r#"{
            "elements": [
                {
                    "id": "box",
                    "type": "rectangle",
                    "x": 0,
                    "y": 0,
                    "width": 200,
                    "height": 100
                },
                {
                    "id": "title",
                    "type": "text",
                    "x": 10,
                    "y": 10,
                    "width": 40,
                    "height": 20,
                    "text": "Title",
                    "containerId": "box"
                },
                {
                    "id": "extra",
                    "type": "text",
                    "x": 80,
                    "y": 40,
                    "width": 40,
                    "height": 20,
                    "text": "extra"
                }
            ]
        }"#,
    );

    let box_shape = scene.component("box").expect("box");
    assert_eq!(box_shape.label.as_deref(), Some("Title"));
    assert_eq!(box_shape.label_element.as_deref(), Some("title"));
    assert_eq!(box_shape.elements, ["box", "title", "extra"]);
    assert!(scene.component("extra").is_none());
    assert!(scene.notes.is_empty());
}

#[test]
fn swimlanes_are_named_zones_with_title_elements() {
    let scene = Scene::build(&load("swimlanes"));
    let ids: Vec<_> = scene.containers.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["lane_fe", "lane_be"]);

    for container in &scene.containers {
        assert_eq!(container.kind, ContainerKind::Zone);
        assert!(container.parent.is_none());
    }

    let lane_fe = scene.container("lane_fe").expect("lane_fe");
    assert_eq!(lane_fe.name.as_deref(), Some("Frontend"));
    assert_eq!(lane_fe.title_element.as_deref(), Some("lane_fe_title"));
    assert_eq!(lane_fe.elements, ["lane_fe", "lane_fe_title"]);
    assert_rect(lane_fe.rect, (0.0, 0.0, 400.0, 300.0));

    let lane_be = scene.container("lane_be").expect("lane_be");
    assert_eq!(lane_be.name.as_deref(), Some("Backend"));
    assert_eq!(lane_be.title_element.as_deref(), Some("lane_be_title"));
    assert_eq!(lane_be.elements, ["lane_be", "lane_be_title"]);
}

#[test]
fn swimlanes_components_parent_to_their_lanes() {
    let scene = Scene::build(&load("swimlanes"));
    let ids: Vec<_> = scene.components.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["web", "mobile", "api", "db"]);

    let parents: Vec<_> = scene
        .components
        .iter()
        .map(|c| c.parent.as_deref())
        .collect();
    assert_eq!(
        parents,
        [
            Some("lane_fe"),
            Some("lane_fe"),
            Some("lane_be"),
            Some("lane_be"),
        ]
    );
    assert!(scene.component("lane_fe_title").is_none());
    assert!(scene.component("lane_be_title").is_none());
}

#[test]
fn icon_group_frame_parents_every_component() {
    let scene = Scene::build(&load("icon_group"));
    assert_eq!(scene.containers.len(), 1);

    let frame = scene
        .container("Fr8Mw2Nq5Xs1Lp4Kv7RbZ")
        .expect("storage frame");
    assert_eq!(frame.kind, ContainerKind::Frame);
    assert_eq!(frame.name.as_deref(), Some("Storage"));
    assert!(frame.parent.is_none());
    assert!(frame.title_element.is_none());
    assert_eq!(frame.elements, ["Fr8Mw2Nq5Xs1Lp4Kv7RbZ"]);
    assert_rect(frame.rect, (0.0, 0.0, 520.0, 260.0));

    assert_eq!(scene.components.len(), 3);
    for component in &scene.components {
        assert_eq!(component.parent.as_deref(), Some("Fr8Mw2Nq5Xs1Lp4Kv7RbZ"));
        assert_eq!(component.frame_id.as_deref(), Some("Fr8Mw2Nq5Xs1Lp4Kv7RbZ"));
    }
}

#[test]
fn diagrams_without_lanes_or_frames_have_no_containers() {
    for name in ["basic_flow", "overlap", "arrows", "text_layout"] {
        let scene = Scene::build(&load(name));
        assert!(
            scene.containers.is_empty(),
            "{name} should have no containers"
        );
    }
}

#[test]
fn nested_zone_parents_inner_zone_and_its_boxes() {
    // Outer 800×600 holds inner 200×200. Two 40×40 boxes sit inside the inner
    // zone only, so each zone has exactly two qualifying children.
    let scene = parse_scene(
        r#"{
            "elements": [
                {
                    "id": "outer",
                    "type": "rectangle",
                    "x": 0,
                    "y": 0,
                    "width": 800,
                    "height": 600
                },
                {
                    "id": "inner",
                    "type": "rectangle",
                    "x": 50,
                    "y": 50,
                    "width": 200,
                    "height": 200
                },
                {
                    "id": "box_a",
                    "type": "rectangle",
                    "x": 60,
                    "y": 70,
                    "width": 40,
                    "height": 40
                },
                {
                    "id": "box_b",
                    "type": "rectangle",
                    "x": 140,
                    "y": 120,
                    "width": 40,
                    "height": 40
                }
            ]
        }"#,
    );

    let kinds: Vec<_> = scene
        .containers
        .iter()
        .map(|c| (c.id.as_str(), c.kind))
        .collect();
    assert_eq!(
        kinds,
        [
            ("outer", ContainerKind::Zone),
            ("inner", ContainerKind::Zone)
        ]
    );

    let inner = scene.container("inner").expect("inner");
    assert_eq!(inner.parent.as_deref(), Some("outer"));
    assert!(scene.container("outer").unwrap().parent.is_none());

    let ids: Vec<_> = scene.components.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["box_a", "box_b"]);
    for id in ["box_a", "box_b"] {
        assert_eq!(
            scene.component(id).unwrap().parent.as_deref(),
            Some("inner"),
            "{id} should parent to the smaller zone"
        );
    }
}

#[test]
fn dissolved_lane_group_becomes_a_zone_parent() {
    let scene = parse_scene(
        r#"{
            "elements": [
                {
                    "id": "bg",
                    "type": "rectangle",
                    "x": 0,
                    "y": 0,
                    "width": 400,
                    "height": 300,
                    "groupIds": ["g1"]
                },
                {
                    "id": "a",
                    "type": "rectangle",
                    "x": 20,
                    "y": 20,
                    "width": 100,
                    "height": 80,
                    "groupIds": ["g1"]
                },
                {
                    "id": "b",
                    "type": "rectangle",
                    "x": 200,
                    "y": 40,
                    "width": 100,
                    "height": 80,
                    "groupIds": ["g1"]
                }
            ]
        }"#,
    );

    assert_eq!(scene.containers.len(), 1);
    let zone = scene.container("bg").expect("dissolved rectangle zone");
    assert_eq!(zone.kind, ContainerKind::Zone);
    assert_rect(zone.rect, (0.0, 0.0, 400.0, 300.0));
    assert!(zone.name.is_none());
    assert!(zone.title_element.is_none());
    assert_eq!(zone.elements, ["bg"]);
    assert!(scene.component("bg").is_none());

    let ids: Vec<_> = scene.components.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["a", "b"]);
    for component in &scene.components {
        assert_eq!(component.kind, ComponentKind::Shape);
        assert_eq!(component.parent.as_deref(), Some("bg"));
    }
}

#[test]
fn frame_is_parent_only_when_child_frame_id_matches() {
    // Both components have frameId null. The spec's frame candidate is
    // C.frame_id == Some(F.id), so a missing frame id must not match a frame
    // that also has no frame id. Zone containment still parents the inner box.
    let scene = parse_scene(
        r#"{
            "elements": [
                {
                    "id": "lane",
                    "type": "rectangle",
                    "x": 0,
                    "y": 0,
                    "width": 400,
                    "height": 300
                },
                {
                    "id": "left",
                    "type": "rectangle",
                    "x": 20,
                    "y": 40,
                    "width": 80,
                    "height": 40
                },
                {
                    "id": "right",
                    "type": "rectangle",
                    "x": 200,
                    "y": 40,
                    "width": 80,
                    "height": 40
                },
                {
                    "id": "aside",
                    "type": "frame",
                    "x": 500,
                    "y": 0,
                    "width": 100,
                    "height": 80,
                    "name": ""
                }
            ]
        }"#,
    );

    let aside = scene.container("aside").expect("unnamed frame");
    assert_eq!(aside.kind, ContainerKind::Frame);
    assert_eq!(aside.name.as_deref(), Some("Frame"));
    assert!(aside.frame_id.is_none());
    assert!(aside.parent.is_none());

    assert_eq!(scene.container("lane").unwrap().parent.as_deref(), None);
    for id in ["left", "right"] {
        let component = scene.component(id).unwrap();
        assert!(component.frame_id.is_none());
        assert_eq!(component.parent.as_deref(), Some("lane"));
    }
}

#[test]
fn zone_title_skips_text_below_the_band_or_inside_a_shape() {
    // Title band is y <= 40. "low" is inside the zone but below the band.
    // "inside" is in the band, but its center sits in a child shape, so the
    // free text "Lane" at the top-left wins.
    let scene = parse_scene(
        r#"{
            "elements": [
                {
                    "id": "lane",
                    "type": "rectangle",
                    "x": 0,
                    "y": 0,
                    "width": 400,
                    "height": 300
                },
                {
                    "id": "left",
                    "type": "rectangle",
                    "x": 40,
                    "y": 10,
                    "width": 80,
                    "height": 40
                },
                {
                    "id": "right",
                    "type": "rectangle",
                    "x": 200,
                    "y": 80,
                    "width": 80,
                    "height": 40
                },
                {
                    "id": "inside",
                    "type": "text",
                    "x": 50,
                    "y": 16,
                    "width": 40,
                    "height": 16,
                    "text": "Inside"
                },
                {
                    "id": "low",
                    "type": "text",
                    "x": 16,
                    "y": 50,
                    "width": 40,
                    "height": 16,
                    "text": "Low"
                },
                {
                    "id": "heading",
                    "type": "text",
                    "x": 16,
                    "y": 8,
                    "width": 40,
                    "height": 16,
                    "text": "Lane"
                }
            ]
        }"#,
    );

    let lane = scene.container("lane").expect("lane");
    assert_eq!(lane.name.as_deref(), Some("Lane"));
    assert_eq!(lane.title_element.as_deref(), Some("heading"));
    assert_eq!(lane.elements, ["lane", "heading"]);
    assert!(scene.component("heading").is_none());
    assert_eq!(scene.component("low").unwrap().kind, ComponentKind::Text);
    assert!(scene.component("inside").is_none());
    assert_eq!(
        scene.component("left").unwrap().elements,
        ["left", "inside"]
    );
}

#[test]
fn equal_area_parents_pick_the_lower_z() {
    // Two 200×100 zones both contain the child's center and have equal area.
    // The earlier zone (lower z) wins.
    let scene = parse_scene(
        r#"{
            "elements": [
                {
                    "id": "first",
                    "type": "rectangle",
                    "x": 0,
                    "y": 0,
                    "width": 200,
                    "height": 100
                },
                {
                    "id": "a",
                    "type": "ellipse",
                    "x": 10,
                    "y": 10,
                    "width": 20,
                    "height": 20
                },
                {
                    "id": "b",
                    "type": "ellipse",
                    "x": 80,
                    "y": 10,
                    "width": 20,
                    "height": 20
                },
                {
                    "id": "second",
                    "type": "rectangle",
                    "x": 50,
                    "y": 0,
                    "width": 200,
                    "height": 100
                },
                {
                    "id": "c",
                    "type": "ellipse",
                    "x": 180,
                    "y": 10,
                    "width": 20,
                    "height": 20
                },
                {
                    "id": "d",
                    "type": "ellipse",
                    "x": 210,
                    "y": 10,
                    "width": 20,
                    "height": 20
                },
                {
                    "id": "child",
                    "type": "rectangle",
                    "x": 90,
                    "y": 40,
                    "width": 20,
                    "height": 20
                }
            ]
        }"#,
    );

    assert_eq!(
        scene.component("child").unwrap().parent.as_deref(),
        Some("first")
    );
}
