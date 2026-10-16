use crate::geometry::{Point, Rect};
use crate::scene_graph::Scene;

pub(crate) fn canvas_rect(scene: &Scene) -> Option<Rect> {
    let mut rects = scene
        .components
        .iter()
        .map(|component| component.rect)
        .chain(scene.containers.iter().map(|container| container.rect))
        .chain(
            scene
                .connectors
                .iter()
                .filter(|connector| !connector.points.is_empty())
                .map(|connector| Rect::from_points(&connector.points)),
        )
        .chain(
            scene
                .connectors
                .iter()
                .filter_map(|connector| connector.label_rect),
        );
    let first = rects.next()?;
    Some(rects.fold(first, |acc, rect| acc.union(&rect)))
}

/// Map rows plus a trailing legend. Empty when the scene has no components;
/// the inspect report prints `(none)` in that case.
pub fn render(scene: &Scene) -> Vec<String> {
    if scene.components.is_empty() {
        return Vec::new();
    }
    let Some(canvas) = canvas_rect(scene) else {
        return Vec::new();
    };

    let width = canvas.width().max(1.0);
    let height = canvas.height().max(1.0);
    let scale = (width / 64.0).max(height / 48.0);
    let cols = (width / scale).ceil().clamp(1.0, 64.0) as usize;
    let rows = (height / (2.0 * scale)).ceil().clamp(1.0, 24.0) as usize;

    let col = |x: f64| -> usize {
        ((x - canvas.x1) / scale)
            .floor()
            .clamp(0.0, (cols - 1) as f64) as usize
    };
    let row = |y: f64| -> usize {
        ((y - canvas.y1) / (2.0 * scale))
            .floor()
            .clamp(0.0, (rows - 1) as f64) as usize
    };

    let mut grid = vec![vec![' '; cols]; rows];

    for container in &scene.containers {
        draw_border(&mut grid, &container.rect, col, row);
    }
    for connector in &scene.connectors {
        draw_polyline(&mut grid, &connector.points, scale, col, row);
    }

    let mut legend = Vec::with_capacity(scene.components.len());
    for (index, component) in scene.components.iter().enumerate() {
        let glyph = legend_char(index);
        fill_rect(&mut grid, &component.rect, glyph, col, row);
        legend.push(format!("{glyph}={}", component.id));
    }

    let mut lines: Vec<String> = grid
        .into_iter()
        .map(|cells| {
            let mut line: String = cells.into_iter().collect();
            line.truncate(line.trim_end().len());
            line
        })
        .collect();
    lines.push(legend.join(" "));
    lines
}

fn draw_border(
    grid: &mut [Vec<char>],
    rect: &Rect,
    col: impl Fn(f64) -> usize,
    row: impl Fn(f64) -> usize,
) {
    let c1 = col(rect.x1);
    let c2 = col(rect.x2);
    let r1 = row(rect.y1);
    let r2 = row(rect.y2);
    for cell in &mut grid[r1][c1..=c2] {
        *cell = '.';
    }
    for cell in &mut grid[r2][c1..=c2] {
        *cell = '.';
    }
    for line in &mut grid[r1..=r2] {
        line[c1] = '.';
        line[c2] = '.';
    }
}

fn draw_polyline(
    grid: &mut [Vec<char>],
    points: &[Point],
    scale: f64,
    col: impl Fn(f64) -> usize,
    row: impl Fn(f64) -> usize,
) {
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let dx = (b.x - a.x).abs() / scale;
        let dy = (b.y - a.y).abs() / (2.0 * scale);
        let n = (2.0 * dx.max(dy)).ceil().max(1.0) as usize;
        for k in 0..=n {
            let t = k as f64 / n as f64;
            let point = Point {
                x: a.x + t * (b.x - a.x),
                y: a.y + t * (b.y - a.y),
            };
            grid[row(point.y)][col(point.x)] = '*';
        }
    }
}

fn fill_rect(
    grid: &mut [Vec<char>],
    rect: &Rect,
    glyph: char,
    col: impl Fn(f64) -> usize,
    row: impl Fn(f64) -> usize,
) {
    let c1 = col(rect.x1);
    let c2 = col(rect.x2);
    let r1 = row(rect.y1);
    let r2 = row(rect.y2);
    for line in &mut grid[r1..=r2] {
        for cell in &mut line[c1..=c2] {
            *cell = glyph;
        }
    }
}

fn legend_char(index: usize) -> char {
    const GLYPHS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    GLYPHS.get(index).copied().map(char::from).unwrap_or('#')
}
