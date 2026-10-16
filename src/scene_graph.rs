use std::collections::{HashMap, HashSet};

use crate::geometry::{Point, Rect};
use crate::linter::{label_str, LintIssue, Severity, SHAPE_TYPES, ZONE_CHILD_AREA_RATIO};
use crate::schema::{Document, RawElement};

const NEAR_DIST: f64 = 12.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ComponentKind {
    Shape,
    Group,
    Text,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Component {
    pub id: String,
    pub kind: ComponentKind,
    pub type_name: String,
    pub rect: Rect,
    pub label: Option<String>,
    pub label_element: Option<String>,
    pub elements: Vec<String>,
    pub parent: Option<String>,
    pub frame_id: Option<String>,
    pub z: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ContainerKind {
    Zone,
    Frame,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Container {
    pub id: String,
    pub kind: ContainerKind,
    pub rect: Rect,
    pub name: Option<String>,
    pub title_element: Option<String>,
    pub elements: Vec<String>,
    pub parent: Option<String>,
    pub frame_id: Option<String>,
    pub z: usize,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "attach", rename_all = "lowercase")]
pub enum Attach {
    Bound { target: String, element: String },
    Near { target: String, distance: f64 },
    Free,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Connector {
    pub id: String,
    pub kind: String,
    pub elbowed: bool,
    pub points: Vec<Point>,
    pub start: Attach,
    pub end: Attach,
    pub label: Option<String>,
    pub label_element: Option<String>,
    pub label_rect: Option<Rect>,
    pub z: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Component(String),
    Container(String),
    Connector(String),
}

pub struct Scene {
    pub components: Vec<Component>,
    pub containers: Vec<Container>,
    pub connectors: Vec<Connector>,
    pub notes: Vec<LintIssue>,
}

struct ShapeLabel {
    text: String,
    element_id: String,
}

struct ArrowLabel {
    text: String,
    element_id: String,
    rect: Rect,
}

impl Scene {
    pub fn build(doc: &Document) -> Scene {
        let elements = valid_elements(doc);

        let mut shape_labels: HashMap<String, ShapeLabel> = HashMap::new();
        let mut arrow_labels: HashMap<String, ArrowLabel> = HashMap::new();
        let mut claimed: HashSet<String> = HashSet::new();
        let mut arrow_label_ids: HashSet<String> = HashSet::new();
        for el in &elements {
            if el.element_type != "text" {
                continue;
            }
            let Some(container_id) = el.container_id.as_deref() else {
                continue;
            };
            let Some(host) = elements.iter().find(|host| host.id == container_id) else {
                continue;
            };
            claimed.insert(el.id.clone());
            if is_connector_type(&host.element_type) {
                arrow_label_ids.insert(el.id.clone());
                arrow_labels.insert(
                    container_id.to_string(),
                    ArrowLabel {
                        text: el.text.clone().unwrap_or_default(),
                        element_id: el.id.clone(),
                        rect: el.rect(),
                    },
                );
            } else {
                shape_labels.insert(
                    container_id.to_string(),
                    ShapeLabel {
                        text: el.text.clone().unwrap_or_default(),
                        element_id: el.id.clone(),
                    },
                );
            }
        }

        let mut members_by_group: HashMap<&str, Vec<&RawElement>> = HashMap::new();
        for el in &elements {
            if matches!(el.element_type.as_str(), "arrow" | "frame" | "magicframe")
                || arrow_label_ids.contains(&el.id)
            {
                continue;
            }
            let Some(group_id) = el.outer_group() else {
                continue;
            };
            members_by_group.entry(group_id).or_default().push(*el);
        }

        let mut atomic_groups: Vec<(&str, Vec<&RawElement>)> = Vec::new();
        let mut atomic_member_ids: HashSet<String> = HashSet::new();
        for (group_id, members) in members_by_group {
            if group_should_dissolve(&members) {
                continue;
            }
            for member in &members {
                atomic_member_ids.insert(member.id.clone());
            }
            atomic_groups.push((group_id, members));
        }

        let mut components = Vec::new();
        for (group_id, members) in atomic_groups {
            let mut member_ids: Vec<String> =
                members.iter().map(|member| member.id.clone()).collect();
            let member_id_set: HashSet<&str> =
                members.iter().map(|member| member.id.as_str()).collect();
            for member in &members {
                if let Some(label) = shape_labels.get(&member.id) {
                    if !member_id_set.contains(label.element_id.as_str()) {
                        member_ids.push(label.element_id.clone());
                    }
                }
                claimed.insert(member.id.clone());
            }

            let mut rect = members[0].rect();
            let mut z = members[0].z;
            let mut frame_id = None;
            let mut texts = Vec::new();
            for member in &members {
                rect = rect.union(&member.rect());
                z = z.min(member.z);
                if frame_id.is_none() {
                    frame_id = non_empty(member.frame_id.as_deref());
                }
                if member.element_type == "text" {
                    texts.push(member.text.clone().unwrap_or_default());
                }
            }

            components.push(Component {
                id: format!("group:{group_id}"),
                kind: ComponentKind::Group,
                type_name: "group".to_string(),
                rect,
                label: if texts.is_empty() {
                    None
                } else {
                    Some(texts.join(" "))
                },
                label_element: None,
                elements: member_ids,
                parent: None,
                frame_id,
                z,
            });
        }

        for el in &elements {
            if !SHAPE_TYPES.contains(&el.element_type.as_str()) {
                continue;
            }
            if atomic_member_ids.contains(&el.id) {
                continue;
            }
            let label = shape_labels.remove(&el.id);
            let mut member_ids = vec![el.id.clone()];
            if let Some(label) = &label {
                member_ids.push(label.element_id.clone());
            }
            components.push(Component {
                id: el.id.clone(),
                kind: ComponentKind::Shape,
                type_name: el.element_type.clone(),
                rect: el.rect(),
                label: label.as_ref().map(|label| label.text.clone()),
                label_element: label.map(|label| label.element_id),
                elements: member_ids,
                parent: None,
                frame_id: non_empty(el.frame_id.as_deref()),
                z: el.z,
            });
        }

        let mut notes = Vec::new();
        for el in &elements {
            if el.element_type != "text" || claimed.contains(&el.id) {
                continue;
            }
            let text_rect = el.rect();
            let text_area = text_rect.area();
            let text_center = text_rect.center();
            // Equal-area hosts: keep the first Shape in current component order
            // (document/z order for shapes created above).
            let mut best: Option<usize> = None;
            let mut best_area = f64::INFINITY;
            for (index, component) in components.iter().enumerate() {
                if component.kind != ComponentKind::Shape {
                    continue;
                }
                let host_area = component.rect.area();
                if host_area > text_area
                    && component.rect.contains_point(text_center)
                    && host_area < best_area
                {
                    best = Some(index);
                    best_area = host_area;
                }
            }
            let Some(index) = best else {
                continue;
            };
            let host = &mut components[index];
            host.elements.push(el.id.clone());
            if host.label.is_none() {
                let text = el.text.clone().unwrap_or_default();
                notes.push(LintIssue {
                    rule: "UNBOUND_TEXT",
                    severity: Severity::Info,
                    targets: vec![el.id.clone(), host.id.clone()],
                    message: format!(
                        "text {} \"{}\" sits inside {} but is not bound to it",
                        el.id,
                        label_str(&text),
                        host.id
                    ),
                    fix: Some(format!(
                        "bind it as the label of {} (set containerId)",
                        host.id
                    )),
                    command: None,
                });
                host.label = Some(text);
                host.label_element = Some(el.id.clone());
            }
            claimed.insert(el.id.clone());
        }

        for el in &elements {
            if el.element_type != "text" || claimed.contains(&el.id) {
                continue;
            }
            components.push(Component {
                id: el.id.clone(),
                kind: ComponentKind::Text,
                type_name: "text".to_string(),
                rect: el.rect(),
                label: el.text.clone(),
                label_element: Some(el.id.clone()),
                elements: vec![el.id.clone()],
                parent: None,
                frame_id: non_empty(el.frame_id.as_deref()),
                z: el.z,
            });
        }
        components.sort_by_key(|component| component.z);

        let containers = Vec::new();
        let element_owner = owner_index(&components, &containers);
        let nodes: Vec<&Component> = components
            .iter()
            .filter(|component| component.kind != ComponentKind::Text)
            .collect();

        let mut connectors = Vec::new();
        for el in &elements {
            if el.element_type != "arrow"
                && (el.element_type != "line" || atomic_member_ids.contains(&el.id))
            {
                continue;
            }
            let points = el.abs_points();
            let start_point = points.first().copied().unwrap_or(Point { x: 0.0, y: 0.0 });
            let end_point = points.last().copied().unwrap_or(start_point);
            let label = arrow_labels.remove(&el.id);
            connectors.push(Connector {
                id: el.id.clone(),
                kind: el.element_type.clone(),
                elbowed: el.elbowed.unwrap_or(false),
                start: attach_end(
                    el.start_binding
                        .as_ref()
                        .and_then(|b| b.element_id.as_deref()),
                    start_point,
                    &element_owner,
                    &nodes,
                ),
                end: attach_end(
                    el.end_binding
                        .as_ref()
                        .and_then(|b| b.element_id.as_deref()),
                    end_point,
                    &element_owner,
                    &nodes,
                ),
                points,
                label: label.as_ref().map(|label| label.text.clone()),
                label_element: label.as_ref().map(|label| label.element_id.clone()),
                label_rect: label.map(|label| label.rect),
                z: el.z,
            });
        }
        connectors.sort_by_key(|connector| connector.z);

        Scene {
            components,
            containers,
            connectors,
            notes,
        }
    }

    pub fn component(&self, id: &str) -> Option<&Component> {
        self.components.iter().find(|component| component.id == id)
    }

    pub fn container(&self, id: &str) -> Option<&Container> {
        self.containers.iter().find(|container| container.id == id)
    }

    pub fn connector(&self, id: &str) -> Option<&Connector> {
        self.connectors.iter().find(|connector| connector.id == id)
    }

    pub fn rect_of(&self, id: &str) -> Option<Rect> {
        self.component(id)
            .map(|component| component.rect)
            .or_else(|| self.container(id).map(|container| container.rect))
    }

    pub fn nodes(&self) -> Vec<&Component> {
        self.components
            .iter()
            .filter(|component| component.kind != ComponentKind::Text)
            .collect()
    }

    pub fn resolve(&self, _query: &str) -> anyhow::Result<Target> {
        anyhow::bail!("not implemented")
    }
}

fn valid_elements(doc: &Document) -> Vec<&RawElement> {
    doc.elements
        .iter()
        .filter(|el| !el.is_deleted())
        .filter(|el| el.element_type != "selection" && el.element_type != "freedraw")
        .filter(|el| {
            SHAPE_TYPES.contains(&el.element_type.as_str())
                || el.element_type == "text"
                || is_connector_type(&el.element_type)
                || el.element_type == "frame"
                || el.element_type == "magicframe"
        })
        .collect()
}

fn is_connector_type(element_type: &str) -> bool {
    element_type == "arrow" || element_type == "line"
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value.filter(|value| !value.is_empty()).map(str::to_string)
}

fn group_should_dissolve(members: &[&RawElement]) -> bool {
    for rectangle in members {
        if rectangle.element_type != "rectangle" {
            continue;
        }
        let rect = rectangle.rect();
        let child_limit = ZONE_CHILD_AREA_RATIO * rect.area();
        let nested = members
            .iter()
            .filter(|member| member.id != rectangle.id)
            .filter(|member| SHAPE_TYPES.contains(&member.element_type.as_str()))
            .filter(|member| member.z > rectangle.z)
            .filter(|member| rect.contains_point(member.rect().center()))
            .filter(|member| member.rect().area() <= child_limit)
            .count();
        if nested >= 2 {
            return true;
        }
    }
    false
}

fn owner_index<'a>(
    components: &'a [Component],
    containers: &'a [Container],
) -> HashMap<&'a str, &'a str> {
    let mut owners = HashMap::new();
    for component in components {
        for element_id in &component.elements {
            owners.insert(element_id.as_str(), component.id.as_str());
        }
    }
    for container in containers {
        for element_id in &container.elements {
            owners.insert(element_id.as_str(), container.id.as_str());
        }
    }
    owners
}

fn attach_end(
    binding_id: Option<&str>,
    point: Point,
    owners: &HashMap<&str, &str>,
    nodes: &[&Component],
) -> Attach {
    if let Some(element_id) = binding_id {
        if let Some(target) = owners.get(element_id) {
            return Attach::Bound {
                target: (*target).to_string(),
                element: element_id.to_string(),
            };
        }
    }
    let mut nearest: Option<(&Component, f64)> = None;
    for node in nodes {
        let distance = node.rect.distance_to_point(point);
        if nearest.is_none_or(|(_, best)| distance < best) {
            nearest = Some((node, distance));
        }
    }
    match nearest {
        Some((node, distance)) if distance <= NEAR_DIST => Attach::Near {
            target: node.id.clone(),
            distance,
        },
        _ => Attach::Free,
    }
}
