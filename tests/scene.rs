use excalidraw_spatial::geometry::{Point, Rect};
use excalidraw_spatial::scene_graph::{Attach, ComponentKind, Scene};
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
