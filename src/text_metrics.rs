use unicode_width::UnicodeWidthChar;

use crate::geometry::Rect;
use crate::schema::RawElement;

#[derive(Debug, Clone, PartialEq)]
pub struct TextSize {
    pub width: f64,
    pub height: f64,
    pub estimated: bool,
}

pub fn estimate(text: &str, font_size: f64, line_height: f64) -> (f64, f64) {
    let mut width = 0.0;
    let mut lines = 0.0;
    for line in text.split('\n') {
        lines += 1.0;
        let mut line_width = 0.0;
        for c in line.chars() {
            let k = match c.width() {
                Some(1) => 0.6,
                Some(2) => 1.0,
                _ => 0.0,
            };
            line_width += font_size * k;
        }
        if line_width > width {
            width = line_width;
        }
    }
    (width, lines * font_size * line_height)
}

pub fn text_size(el: &RawElement) -> TextSize {
    let text = el.text.as_deref().unwrap_or("");
    let est = estimate(text, el.font_size(), el.line_height());
    let width = el.width();
    let height = el.height();
    if width > 0.0 && height > 0.0 && est.0 <= 1.5 * width {
        TextSize {
            width,
            height,
            estimated: false,
        }
    } else {
        TextSize {
            width: est.0,
            height: est.1,
            estimated: true,
        }
    }
}

pub fn text_rect(el: &RawElement) -> Rect {
    let ts = text_size(el);
    if !ts.estimated {
        return el.rect();
    }
    let x1 = match el.text_align.as_deref() {
        Some("center") => el.x() + el.width() / 2.0 - ts.width / 2.0,
        Some("right") => el.x() + el.width() - ts.width,
        _ => el.x(),
    };
    Rect::from_xywh(x1, el.y(), ts.width, ts.height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Document;

    fn client_label() -> crate::schema::RawElement {
        let doc = Document::parse(include_str!("../tests/fixtures/basic_flow.excalidraw")).unwrap();
        doc.element("client-label").unwrap().clone()
    }

    #[test]
    fn estimate_ascii_client() {
        assert_eq!(estimate("Client", 20.0, 1.25), (72.0, 25.0));
    }

    #[test]
    fn estimate_cjk_mixed() {
        assert_eq!(estimate("缓存 Cache", 20.0, 1.25), (112.0, 25.0));
    }

    #[test]
    fn estimate_multiline() {
        assert_eq!(estimate("a\nbcd", 10.0, 1.5), (18.0, 30.0));
    }

    #[test]
    fn text_size_prefers_stored_when_close() {
        let el = client_label();
        assert_eq!(
            text_size(&el),
            TextSize {
                width: 62.4,
                height: 25.0,
                estimated: false,
            }
        );
    }

    #[test]
    fn text_size_estimates_when_width_zero() {
        let mut el = client_label();
        el.width = Some(0.0);
        assert_eq!(
            text_size(&el),
            TextSize {
                width: 72.0,
                height: 25.0,
                estimated: true,
            }
        );
    }

    #[test]
    fn text_size_estimates_when_stored_too_narrow() {
        let el: crate::schema::RawElement = serde_json::from_str(
            r#"{
                "id": "n",
                "type": "text",
                "text": "Notification Service",
                "fontSize": 20,
                "width": 50,
                "height": 25
            }"#,
        )
        .unwrap();
        assert_eq!(
            text_size(&el),
            TextSize {
                width: 240.0,
                height: 25.0,
                estimated: true,
            }
        );
    }

    #[test]
    fn text_rect_center_anchor_when_stored_too_narrow() {
        let el: crate::schema::RawElement = serde_json::from_str(
            r#"{
                "id": "lbl",
                "type": "text",
                "x": 140,
                "y": 130,
                "width": 40,
                "height": 25,
                "text": "Order Fulfillment Service",
                "fontSize": 20,
                "textAlign": "center"
            }"#,
        )
        .unwrap();
        assert_eq!(
            text_rect(&el),
            crate::geometry::Rect {
                x1: 10.0,
                y1: 130.0,
                x2: 310.0,
                y2: 155.0,
            }
        );
    }

    #[test]
    fn text_rect_center_anchor_when_width_zero() {
        let el: crate::schema::RawElement = serde_json::from_str(
            r#"{
                "id": "arr-lbl",
                "type": "text",
                "x": 170,
                "y": 130,
                "width": 0,
                "height": 0,
                "text": "validateToken()",
                "fontSize": 16,
                "textAlign": "center"
            }"#,
        )
        .unwrap();
        let got = text_rect(&el);
        let expected = crate::geometry::Rect {
            x1: 98.0,
            y1: 130.0,
            x2: 242.0,
            y2: 150.0,
        };
        // 15 * 16 * 0.6 is not a dyadic rational in f64, so x1 is 98±ε.
        assert!(
            (got.x1 - expected.x1).abs() < 1e-6
                && (got.y1 - expected.y1).abs() < 1e-6
                && (got.x2 - expected.x2).abs() < 1e-6
                && (got.y2 - expected.y2).abs() < 1e-6,
            "{got:?} != {expected:?}"
        );
    }

    #[test]
    fn text_rect_left_anchor_when_estimated() {
        let el: crate::schema::RawElement = serde_json::from_str(
            r#"{
                "id": "lbl",
                "type": "text",
                "x": 140,
                "y": 130,
                "width": 40,
                "height": 25,
                "text": "Order Fulfillment Service",
                "fontSize": 20,
                "textAlign": "left"
            }"#,
        )
        .unwrap();
        assert_eq!(
            text_rect(&el),
            crate::geometry::Rect {
                x1: 140.0,
                y1: 130.0,
                x2: 440.0,
                y2: 155.0,
            }
        );
    }

    #[test]
    fn text_rect_right_anchor_when_estimated() {
        let el: crate::schema::RawElement = serde_json::from_str(
            r#"{
                "id": "lbl",
                "type": "text",
                "x": 140,
                "y": 130,
                "width": 40,
                "height": 25,
                "text": "Order Fulfillment Service",
                "fontSize": 20,
                "textAlign": "right"
            }"#,
        )
        .unwrap();
        assert_eq!(
            text_rect(&el),
            crate::geometry::Rect {
                x1: -120.0,
                y1: 130.0,
                x2: 180.0,
                y2: 155.0,
            }
        );
    }

    #[test]
    fn text_rect_uses_stored_rect_when_not_estimated() {
        let el: crate::schema::RawElement = serde_json::from_str(
            r#"{
                "id": "hi",
                "type": "text",
                "x": 10,
                "y": 20,
                "width": 40,
                "height": 25,
                "text": "Hi",
                "fontSize": 20,
                "textAlign": "center"
            }"#,
        )
        .unwrap();
        assert_eq!(
            text_rect(&el),
            crate::geometry::Rect {
                x1: 10.0,
                y1: 20.0,
                x2: 50.0,
                y2: 45.0,
            }
        );
    }
}
