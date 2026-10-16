use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Rect {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl Rect {
    pub fn from_xywh(x: f64, y: f64, w: f64, h: f64) -> Rect {
        let (x1, x2) = if w < 0.0 { (x + w, x) } else { (x, x + w) };
        let (y1, y2) = if h < 0.0 { (y + h, y) } else { (y, y + h) };
        Rect { x1, y1, x2, y2 }
    }

    pub fn from_points(pts: &[Point]) -> Rect {
        if pts.is_empty() {
            return Rect {
                x1: 0.0,
                y1: 0.0,
                x2: 0.0,
                y2: 0.0,
            };
        }
        let mut x1 = pts[0].x;
        let mut y1 = pts[0].y;
        let mut x2 = pts[0].x;
        let mut y2 = pts[0].y;
        for p in &pts[1..] {
            x1 = x1.min(p.x);
            y1 = y1.min(p.y);
            x2 = x2.max(p.x);
            y2 = y2.max(p.y);
        }
        Rect { x1, y1, x2, y2 }
    }

    pub fn width(&self) -> f64 {
        self.x2 - self.x1
    }

    pub fn height(&self) -> f64 {
        self.y2 - self.y1
    }

    pub fn area(&self) -> f64 {
        self.width() * self.height()
    }

    pub fn center(&self) -> Point {
        Point {
            x: (self.x1 + self.x2) / 2.0,
            y: (self.y1 + self.y2) / 2.0,
        }
    }

    pub fn union(&self, o: &Rect) -> Rect {
        Rect {
            x1: self.x1.min(o.x1),
            y1: self.y1.min(o.y1),
            x2: self.x2.max(o.x2),
            y2: self.y2.max(o.y2),
        }
    }

    pub fn translate(&self, dx: f64, dy: f64) -> Rect {
        Rect {
            x1: self.x1 + dx,
            y1: self.y1 + dy,
            x2: self.x2 + dx,
            y2: self.y2 + dy,
        }
    }

    pub fn inflate(&self, d: f64) -> Rect {
        Rect {
            x1: self.x1 - d,
            y1: self.y1 - d,
            x2: self.x2 + d,
            y2: self.y2 + d,
        }
    }

    pub fn contains_point(&self, p: Point) -> bool {
        p.x >= self.x1 && p.x <= self.x2 && p.y >= self.y1 && p.y <= self.y2
    }

    pub fn overlap_depth(&self, o: &Rect) -> (f64, f64) {
        (
            self.x2.min(o.x2) - self.x1.max(o.x1),
            self.y2.min(o.y2) - self.y1.max(o.y1),
        )
    }

    pub fn overlaps(&self, o: &Rect) -> bool {
        let (dx, dy) = self.overlap_depth(o);
        dx > 0.5 && dy > 0.5
    }

    pub fn distance_to_point(&self, p: Point) -> f64 {
        let c = self.closest_point(p);
        let dx = p.x - c.x;
        let dy = p.y - c.y;
        (dx * dx + dy * dy).sqrt()
    }

    pub fn closest_point(&self, p: Point) -> Point {
        Point {
            x: p.x.clamp(self.x1, self.x2),
            y: p.y.clamp(self.y1, self.y2),
        }
    }
}

/// Liang-Barsky: parameter interval of segment a->b inside r.
pub fn clip_segment(a: Point, b: Point, r: &Rect) -> Option<(f64, f64)> {
    if a.x == b.x && a.y == b.y {
        return if r.contains_point(a) {
            Some((0.0, 1.0))
        } else {
            None
        };
    }
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let mut t0 = 0.0;
    let mut t1 = 1.0;
    let clips = [
        (-dx, a.x - r.x1),
        (dx, r.x2 - a.x),
        (-dy, a.y - r.y1),
        (dy, r.y2 - a.y),
    ];
    for (p, q) in clips {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let t = q / p;
        if p < 0.0 {
            if t > t0 {
                t0 = t;
            }
        } else if t < t1 {
            t1 = t;
        }
        if t0 > t1 {
            return None;
        }
    }
    Some((t0, t1))
}

pub fn polyline_point_at(points: &[Point], fraction: f64) -> Point {
    if points.is_empty() {
        return Point { x: 0.0, y: 0.0 };
    }
    if points.len() == 1 {
        return points[0];
    }
    let mut total = 0.0;
    let mut lengths = Vec::with_capacity(points.len() - 1);
    for i in 0..points.len() - 1 {
        let dx = points[i + 1].x - points[i].x;
        let dy = points[i + 1].y - points[i].y;
        let len = (dx * dx + dy * dy).sqrt();
        lengths.push(len);
        total += len;
    }
    if total == 0.0 {
        return points[0];
    }
    let fraction = fraction.clamp(0.0, 1.0);
    if fraction == 0.0 {
        return points[0];
    }
    if fraction == 1.0 {
        return *points.last().unwrap();
    }
    let mut remaining = fraction * total;
    for i in 0..lengths.len() {
        let len = lengths[i];
        if remaining <= 0.0 {
            return points[i];
        }
        if remaining <= len {
            if remaining == 0.0 {
                return points[i];
            }
            if remaining == len {
                return points[i + 1];
            }
            let t = remaining / len;
            return Point {
                x: points[i].x + t * (points[i + 1].x - points[i].x),
                y: points[i].y + t * (points[i + 1].y - points[i].y),
            };
        }
        remaining -= len;
    }
    *points.last().unwrap()
}

pub fn fmt_px(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_string();
    }
    let n = v.round() as i64;
    if n == 0 {
        "0".to_string()
    } else {
        n.to_string()
    }
}

/// Subtract 1e-6 so float error just above an integer still ceils to that integer.
pub fn ceil_px(v: f64) -> f64 {
    if !v.is_finite() {
        return v;
    }
    (v - 1e-6).ceil()
}

pub fn fmt_rect(r: &Rect) -> String {
    format!(
        "x{}..{} y{}..{}",
        fmt_px(r.x1),
        fmt_px(r.x2),
        fmt_px(r.y1),
        fmt_px(r.y2)
    )
}

pub fn fmt_point(p: Point) -> String {
    format!("({},{})", fmt_px(p.x), fmt_px(p.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_xywh_negative_size_swaps_endpoints() {
        assert_eq!(
            Rect::from_xywh(10.0, 20.0, -5.0, -10.0),
            Rect {
                x1: 5.0,
                y1: 10.0,
                x2: 10.0,
                y2: 20.0
            }
        );
    }

    #[test]
    fn overlap_depth_and_overlaps() {
        let a = Rect {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 10.0,
        };
        let b = Rect {
            x1: 5.0,
            y1: 5.0,
            x2: 20.0,
            y2: 20.0,
        };
        assert_eq!(a.overlap_depth(&b), (5.0, 5.0));
        assert!(a.overlaps(&b));
        let edge = Rect {
            x1: 10.0,
            y1: 0.0,
            x2: 20.0,
            y2: 10.0,
        };
        assert!(!a.overlaps(&edge));
    }

    #[test]
    fn distance_to_point_outside_and_inside() {
        let r = Rect {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 10.0,
        };
        assert_eq!(r.distance_to_point(Point { x: 13.0, y: 14.0 }), 5.0);
        assert_eq!(r.distance_to_point(Point { x: 5.0, y: 5.0 }), 0.0);
    }

    #[test]
    fn closest_point_clamps_into_rect() {
        let r = Rect {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 10.0,
        };
        assert_eq!(
            r.closest_point(Point { x: 13.0, y: -4.0 }),
            Point { x: 10.0, y: 0.0 }
        );
    }

    #[test]
    fn clip_segment_horizontal_through_rect() {
        let r = Rect {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 10.0,
        };
        assert_eq!(
            clip_segment(Point { x: -5.0, y: 5.0 }, Point { x: 15.0, y: 5.0 }, &r),
            Some((0.25, 0.75))
        );
    }

    #[test]
    fn clip_segment_along_left_edge_counts() {
        let r = Rect {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 10.0,
        };
        assert_eq!(
            clip_segment(Point { x: 0.0, y: -5.0 }, Point { x: 0.0, y: 15.0 }, &r),
            Some((0.25, 0.75))
        );
    }

    #[test]
    fn clip_segment_misses_rect() {
        let r = Rect {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 10.0,
        };
        assert_eq!(
            clip_segment(Point { x: -5.0, y: -5.0 }, Point { x: -1.0, y: 20.0 }, &r),
            None
        );
    }

    #[test]
    fn clip_segment_degenerate_point() {
        let r = Rect {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 10.0,
        };
        assert_eq!(
            clip_segment(Point { x: 5.0, y: 5.0 }, Point { x: 5.0, y: 5.0 }, &r),
            Some((0.0, 1.0))
        );
        assert_eq!(
            clip_segment(Point { x: 20.0, y: 20.0 }, Point { x: 20.0, y: 20.0 }, &r),
            None
        );
    }

    #[test]
    fn polyline_point_at_mid_and_three_quarter() {
        let pts = [
            Point { x: 0.0, y: 0.0 },
            Point { x: 10.0, y: 0.0 },
            Point { x: 10.0, y: 10.0 },
        ];
        assert_eq!(polyline_point_at(&pts, 0.5), Point { x: 10.0, y: 0.0 });
        assert_eq!(polyline_point_at(&pts, 0.75), Point { x: 10.0, y: 5.0 });
        assert_eq!(
            polyline_point_at(&[Point { x: 3.0, y: 4.0 }], 0.5),
            Point { x: 3.0, y: 4.0 }
        );
    }

    #[test]
    fn fmt_px_rounds_half_away_from_zero() {
        assert_eq!(fmt_px(144.5), "145");
        assert_eq!(fmt_px(-2.5), "-3");
        assert_eq!(fmt_px(-0.4), "0");
    }

    #[test]
    fn ceil_px_absorbs_float_error() {
        assert_eq!(ceil_px(208.00000000000006), 208.0);
        assert_eq!(ceil_px(64.8), 65.0);
    }

    #[test]
    fn fmt_rect_and_fmt_point() {
        let r = Rect {
            x1: 100.0,
            y1: 100.0,
            x2: 260.0,
            y2: 180.0,
        };
        assert_eq!(fmt_rect(&r), "x100..260 y100..180");
        assert_eq!(fmt_point(Point { x: 266.0, y: 140.0 }), "(266,140)");
    }

    #[test]
    fn from_points_empty_and_two_points() {
        assert_eq!(
            Rect::from_points(&[]),
            Rect {
                x1: 0.0,
                y1: 0.0,
                x2: 0.0,
                y2: 0.0
            }
        );
        assert_eq!(
            Rect::from_points(&[Point { x: 1.0, y: 2.0 }, Point { x: 4.0, y: -1.0 }]),
            Rect {
                x1: 1.0,
                y1: -1.0,
                x2: 4.0,
                y2: 2.0
            }
        );
    }

    #[test]
    fn width_height_area_center() {
        let r = Rect {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 20.0,
        };
        assert_eq!(r.width(), 10.0);
        assert_eq!(r.height(), 20.0);
        assert_eq!(r.area(), 200.0);
        assert_eq!(r.center(), Point { x: 5.0, y: 10.0 });
    }

    #[test]
    fn union_translate_inflate() {
        let a = Rect {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 10.0,
        };
        let b = Rect {
            x1: 5.0,
            y1: 5.0,
            x2: 20.0,
            y2: 30.0,
        };
        assert_eq!(
            a.union(&b),
            Rect {
                x1: 0.0,
                y1: 0.0,
                x2: 20.0,
                y2: 30.0
            }
        );
        assert_eq!(
            Rect {
                x1: 1.0,
                y1: 2.0,
                x2: 3.0,
                y2: 4.0
            }
            .translate(10.0, -1.0),
            Rect {
                x1: 11.0,
                y1: 1.0,
                x2: 13.0,
                y2: 3.0
            }
        );
        assert_eq!(
            a.inflate(2.0),
            Rect {
                x1: -2.0,
                y1: -2.0,
                x2: 12.0,
                y2: 12.0
            }
        );
        assert_eq!(
            a.inflate(-1.0),
            Rect {
                x1: 1.0,
                y1: 1.0,
                x2: 9.0,
                y2: 9.0
            }
        );
    }

    #[test]
    fn contains_point_inclusive() {
        let r = Rect {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 10.0,
        };
        assert!(r.contains_point(Point { x: 0.0, y: 0.0 }));
        assert!(r.contains_point(Point { x: 10.0, y: 5.0 }));
        assert!(r.contains_point(Point { x: 5.0, y: 10.0 }));
        assert!(!r.contains_point(Point { x: -0.1, y: 5.0 }));
        assert!(!r.contains_point(Point { x: 10.1, y: 5.0 }));
    }

    #[test]
    fn overlap_depth_edge_touch_is_zero() {
        let a = Rect {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 10.0,
        };
        let b = Rect {
            x1: 10.0,
            y1: 0.0,
            x2: 20.0,
            y2: 10.0,
        };
        assert_eq!(a.overlap_depth(&b), (0.0, 10.0));
        assert!(!a.overlaps(&b));
    }

    #[test]
    fn polyline_endpoints_and_zero_length() {
        let pts = [
            Point { x: 0.0, y: 0.0 },
            Point { x: 10.0, y: 0.0 },
            Point { x: 10.0, y: 10.0 },
        ];
        assert_eq!(polyline_point_at(&pts, 0.0), Point { x: 0.0, y: 0.0 });
        assert_eq!(polyline_point_at(&pts, 1.0), Point { x: 10.0, y: 10.0 });
        assert_eq!(
            polyline_point_at(&[Point { x: 7.0, y: 8.0 }, Point { x: 7.0, y: 8.0 }], 0.5),
            Point { x: 7.0, y: 8.0 }
        );
    }

    #[test]
    fn clip_segment_starts_inside_and_fully_inside() {
        let r = Rect {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 10.0,
        };
        assert_eq!(
            clip_segment(Point { x: 5.0, y: 5.0 }, Point { x: 15.0, y: 5.0 }, &r),
            Some((0.0, 0.5))
        );
        assert_eq!(
            clip_segment(Point { x: 1.0, y: 1.0 }, Point { x: 9.0, y: 9.0 }, &r),
            Some((0.0, 1.0))
        );
    }
}
