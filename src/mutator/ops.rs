use crate::scene_graph::{Scene, Target};
use crate::{Axis, Direction, PlaceAlign};

use super::Change;

pub fn move_target(scene: &Scene, id: &str, dx: f64, dy: f64) -> anyhow::Result<Change> {
    let resolved = match scene.resolve(id)? {
        Target::Component(id) | Target::Container(id) | Target::Connector(id) => id,
    };
    Ok(Change {
        moves: vec![(resolved, dx, dy)],
        resizes: vec![],
    })
}

pub fn align(_scene: &Scene, _ids: &[String], _axis: Axis) -> anyhow::Result<Change> {
    anyhow::bail!("not implemented")
}

pub fn distribute(
    _scene: &Scene,
    _ids: &[String],
    _direction: Direction,
    _gap: Option<f64>,
) -> anyhow::Result<Change> {
    anyhow::bail!("not implemented")
}

#[allow(clippy::too_many_arguments)]
pub fn place(
    _scene: &Scene,
    _id: &str,
    _right_of: Option<&str>,
    _left_of: Option<&str>,
    _below: Option<&str>,
    _above: Option<&str>,
    _gap: Option<f64>,
    _align: PlaceAlign,
) -> anyhow::Result<Change> {
    anyhow::bail!("not implemented")
}

pub fn fit_container(_scene: &Scene, _id: &str, _padding: Option<f64>) -> anyhow::Result<Change> {
    anyhow::bail!("not implemented")
}

pub fn fit_text(
    _scene: &Scene,
    _id: Option<&str>,
    _all: bool,
    _padding_x: Option<f64>,
    _padding_y: Option<f64>,
) -> anyhow::Result<Change> {
    anyhow::bail!("not implemented")
}
