use std::collections::{HashMap, HashSet};

use crate::geometry::{Point, Rect};
use crate::linter::{
    label_str, text_factor, LintIssue, Severity, SHAPE_TYPES, TEXT_PAD_X, TEXT_PAD_Y,
    ZONE_CHILD_AREA_RATIO, ZONE_CONTAINED_AREA_RATIO, ZONE_TITLE_BAND,
};
use crate::schema::{Document, RawElement};
use crate::text_metrics::{text_rect, text_size, TextSize};

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
    /// `text_size` of `label_element`, captured at build so lint can run without the document.
    #[serde(skip)]
    pub label_size: Option<TextSize>,
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
    /// `text_size` height of `title_element`, captured at build so
    /// `fit_container` can run without the document.
    #[serde(skip)]
    pub title_height: Option<f64>,
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
    vertical_align: Option<String>,
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
                        rect: text_rect(el),
                    },
                );
            } else {
                shape_labels.insert(
                    container_id.to_string(),
                    ShapeLabel {
                        text: el.text.clone().unwrap_or_default(),
                        element_id: el.id.clone(),
                        vertical_align: el.vertical_align.clone(),
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

        let candidates = candidate_nodes(&elements, &atomic_groups, &atomic_member_ids);
        let zone_ids = zone_rectangle_ids(
            &candidates,
            &shape_labels,
            &elements,
            &claimed,
            &atomic_member_ids,
        );
        let mut containers = Vec::new();
        for candidate in &candidates {
            if !zone_ids.contains(candidate.id) {
                continue;
            }
            let label = shape_labels.remove(candidate.id);
            let mut member_ids = vec![candidate.id.to_string()];
            if let Some(label) = &label {
                member_ids.push(label.element_id.clone());
            }
            let title_height = label.as_ref().and_then(|label| {
                elements
                    .iter()
                    .find(|el| el.id == label.element_id)
                    .map(|el| text_size(el).height)
            });
            containers.push(Container {
                id: candidate.id.to_string(),
                kind: ContainerKind::Zone,
                rect: candidate.rect,
                name: label.as_ref().map(|label| label.text.clone()),
                title_element: label.as_ref().map(|label| label.element_id.clone()),
                title_height,
                elements: member_ids,
                parent: None,
                frame_id: candidate.frame_id.clone(),
                z: candidate.z,
            });
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
                label_size: None,
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
            if atomic_member_ids.contains(&el.id) || zone_ids.contains(el.id.as_str()) {
                continue;
            }
            let label = shape_labels.remove(&el.id);
            let mut member_ids = vec![el.id.clone()];
            if let Some(label) = &label {
                member_ids.push(label.element_id.clone());
            }
            let label_size = label.as_ref().and_then(|label| {
                elements
                    .iter()
                    .find(|candidate| candidate.id == label.element_id)
                    .map(|candidate| text_size(candidate))
            });
            components.push(Component {
                id: el.id.clone(),
                kind: ComponentKind::Shape,
                type_name: el.element_type.clone(),
                rect: el.rect(),
                label: label.as_ref().map(|label| label.text.clone()),
                label_element: label.map(|label| label.element_id),
                label_size,
                elements: member_ids,
                parent: None,
                frame_id: non_empty(el.frame_id.as_deref()),
                z: el.z,
            });
        }

        assign_zone_titles(&mut containers, &components, &elements, &mut claimed);

        let mut notes = Vec::new();
        for el in &elements {
            if el.element_type != "text" || claimed.contains(&el.id) {
                continue;
            }
            let free_rect = text_rect(el);
            let text_area = free_rect.area();
            let text_center = free_rect.center();
            // Equal-area hosts: keep the first Shape in current component order
            // (document/z order for shapes created above).
            let index = smallest_absorbing_host(&components, text_area, |component| {
                component.rect.contains_point(text_center)
            })
            .or_else(|| {
                smallest_absorbing_host(&components, text_area, |component| {
                    alignment_anchor_hits(component.rect, free_rect, el.text_align.as_deref())
                })
            });
            let Some(index) = index else {
                continue;
            };
            let ts = text_size(el);
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
            }
            if host.label_element.is_none() {
                host.label_element = Some(el.id.clone());
            }
            let req = absorbed_label_room(host, free_rect, ts.width, ts.height);
            host.label_size = Some(match &host.label_size {
                Some(prev) => TextSize {
                    width: prev.width.max(req.0),
                    height: prev.height.max(req.1),
                    estimated: prev.estimated || ts.estimated,
                },
                None => TextSize {
                    width: req.0,
                    height: req.1,
                    estimated: ts.estimated,
                },
            });
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
                rect: text_rect(el),
                label: el.text.clone(),
                label_element: Some(el.id.clone()),
                label_size: Some(text_size(el)),
                elements: vec![el.id.clone()],
                parent: None,
                frame_id: non_empty(el.frame_id.as_deref()),
                z: el.z,
            });
        }
        components.sort_by_key(|component| component.z);

        for el in &elements {
            if el.element_type != "frame" && el.element_type != "magicframe" {
                continue;
            }
            let name = non_empty(el.name.as_deref()).unwrap_or_else(|| "Frame".to_string());
            containers.push(Container {
                id: el.id.clone(),
                kind: ContainerKind::Frame,
                rect: el.rect(),
                name: Some(name),
                title_element: None,
                title_height: None,
                elements: vec![el.id.clone()],
                parent: None,
                frame_id: non_empty(el.frame_id.as_deref()),
                z: el.z,
            });
        }
        containers.sort_by_key(|container| container.z);
        assign_parents(&mut components, &mut containers);

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

    pub fn resolve(&self, query: &str) -> anyhow::Result<Target> {
        if let Some(component) = self.component(query) {
            return Ok(Target::Component(component.id.clone()));
        }
        if let Some(container) = self.container(query) {
            return Ok(Target::Container(container.id.clone()));
        }
        if let Some(connector) = self.connector(query) {
            return Ok(Target::Connector(connector.id.clone()));
        }

        if let Some(component) = self
            .components
            .iter()
            .find(|component| component.elements.iter().any(|id| id == query))
        {
            return Ok(Target::Component(component.id.clone()));
        }
        if let Some(container) = self
            .containers
            .iter()
            .find(|container| container.elements.iter().any(|id| id == query))
        {
            return Ok(Target::Container(container.id.clone()));
        }

        let needle = query.trim();
        let mut label_matches: Vec<(usize, Target)> = Vec::new();
        for component in &self.components {
            if component
                .label
                .as_deref()
                .is_some_and(|label| label.trim() == needle)
            {
                label_matches.push((component.z, Target::Component(component.id.clone())));
            }
        }
        for container in &self.containers {
            if container
                .name
                .as_deref()
                .is_some_and(|name| name.trim() == needle)
            {
                label_matches.push((container.z, Target::Container(container.id.clone())));
            }
        }
        label_matches.sort_by_key(|(z, _)| *z);
        match label_matches.len() {
            0 => anyhow::bail!("unknown id \"{query}\""),
            1 => Ok(label_matches.pop().expect("one match").1),
            _ => {
                let ids = label_matches
                    .iter()
                    .map(|(_, target)| match target {
                        Target::Component(id) | Target::Container(id) | Target::Connector(id) => {
                            id.as_str()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                anyhow::bail!("ambiguous id \"{query}\": matches {ids}")
            }
        }
    }
}

fn smallest_absorbing_host(
    components: &[Component],
    text_area: f64,
    matches: impl Fn(&Component) -> bool,
) -> Option<usize> {
    let mut best: Option<usize> = None;
    let mut best_area = f64::INFINITY;
    for (index, component) in components.iter().enumerate() {
        if component.kind != ComponentKind::Shape {
            continue;
        }
        let host_area = component.rect.area();
        if host_area > text_area && matches(component) && host_area < best_area {
            best = Some(index);
            best_area = host_area;
        }
    }
    best
}

fn alignment_anchor_hits(host: Rect, text: Rect, text_align: Option<&str>) -> bool {
    let center = text.center();
    if center.y < host.y1 || center.y > host.y2 {
        return false;
    }
    let host_center_x = host.center().x;
    match text_align {
        Some("right") => host_center_x <= text.x2 && text.x2 <= host.x2,
        Some("center") => false,
        _ => host.x1 <= text.x1 && text.x1 <= host_center_x,
    }
}

fn absorbed_label_room(host: &Component, text: Rect, width: f64, height: f64) -> (f64, f64) {
    let f = text_factor(&host.type_name);
    let hc = host.rect.center();
    let inner_w = (host.rect.width() * f - f64::from(TEXT_PAD_X)).max(0.0);
    let inner_h = (host.rect.height() * f - f64::from(TEXT_PAD_Y)).max(0.0);
    let usable = Rect::from_xywh(hc.x - inner_w / 2.0, hc.y - inner_h / 2.0, inner_w, inner_h);
    if usable.contains_rect(&text) {
        return (width, height);
    }
    let env_w = 2.0 * (hc.x - text.x1).abs().max((text.x2 - hc.x).abs());
    let env_h = 2.0 * (hc.y - text.y1).abs().max((text.y2 - hc.y).abs());
    (width.max(env_w), height.max(env_h))
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

struct Candidate<'a> {
    id: &'a str,
    rect: Rect,
    z: usize,
    frame_id: Option<String>,
    is_ungrouped_rectangle: bool,
}

fn candidate_nodes<'a>(
    elements: &[&'a RawElement],
    atomic_groups: &[(&'a str, Vec<&'a RawElement>)],
    atomic_member_ids: &HashSet<String>,
) -> Vec<Candidate<'a>> {
    let mut candidates = Vec::new();
    for el in elements {
        if !SHAPE_TYPES.contains(&el.element_type.as_str()) || atomic_member_ids.contains(&el.id) {
            continue;
        }
        candidates.push(Candidate {
            id: el.id.as_str(),
            rect: el.rect(),
            z: el.z,
            frame_id: non_empty(el.frame_id.as_deref()),
            is_ungrouped_rectangle: el.element_type == "rectangle",
        });
    }
    for (group_id, members) in atomic_groups {
        let mut rect = members[0].rect();
        let mut z = members[0].z;
        let mut frame_id = None;
        for member in members {
            rect = rect.union(&member.rect());
            z = z.min(member.z);
            if frame_id.is_none() {
                frame_id = non_empty(member.frame_id.as_deref());
            }
        }
        candidates.push(Candidate {
            id: group_id,
            rect,
            z,
            frame_id,
            is_ungrouped_rectangle: false,
        });
    }
    candidates
}

fn is_zone_child(parent_rect: &Rect, child_rect: &Rect) -> bool {
    child_rect.area() <= ZONE_CHILD_AREA_RATIO * parent_rect.area()
        || (parent_rect.contains_rect(child_rect)
            && child_rect.area() <= ZONE_CONTAINED_AREA_RATIO * parent_rect.area())
}

fn zone_rectangle_ids<'a>(
    candidates: &[Candidate<'a>],
    shape_labels: &HashMap<String, ShapeLabel>,
    elements: &[&RawElement],
    claimed: &HashSet<String>,
    atomic_member_ids: &HashSet<String>,
) -> HashSet<&'a str> {
    let mut zones = HashSet::new();
    for rectangle in candidates
        .iter()
        .filter(|candidate| candidate.is_ungrouped_rectangle)
    {
        let children: Vec<&Candidate> = candidates
            .iter()
            .filter(|other| other.id != rectangle.id)
            .filter(|other| other.z > rectangle.z)
            .filter(|other| rectangle.rect.contains_point(other.rect.center()))
            .filter(|other| is_zone_child(&rectangle.rect, &other.rect))
            .collect();
        if children.len() >= 2 {
            zones.insert(rectangle.id);
        } else if children.len() == 1 {
            let child = children[0];
            if rectangle.rect.contains_rect(&child.rect)
                && has_top_title_evidence(
                    rectangle,
                    child,
                    shape_labels,
                    elements,
                    claimed,
                    atomic_member_ids,
                )
            {
                zones.insert(rectangle.id);
            }
        }
    }
    zones
}

fn has_top_title_evidence(
    rectangle: &Candidate,
    child: &Candidate,
    shape_labels: &HashMap<String, ShapeLabel>,
    elements: &[&RawElement],
    claimed: &HashSet<String>,
    atomic_member_ids: &HashSet<String>,
) -> bool {
    if shape_labels
        .get(rectangle.id)
        .and_then(|label| label.vertical_align.as_deref())
        == Some("top")
    {
        return true;
    }
    let band = f64::from(ZONE_TITLE_BAND);
    for el in elements {
        if el.element_type != "text" {
            continue;
        }
        if claimed.contains(&el.id) || atomic_member_ids.contains(&el.id) {
            continue;
        }
        let tr = text_rect(el);
        let tc = tr.center();
        if rectangle.rect.contains_point(tc)
            && tc.y <= rectangle.rect.y1 + band
            && tc.y < child.rect.y1
        {
            return true;
        }
    }
    false
}

fn assign_zone_titles(
    containers: &mut [Container],
    components: &[Component],
    elements: &[&RawElement],
    claimed: &mut HashSet<String>,
) {
    let nodes: Vec<&Component> = components
        .iter()
        .filter(|component| component.kind != ComponentKind::Text)
        .collect();
    let mut order: Vec<usize> = containers
        .iter()
        .enumerate()
        .filter(|(_, container)| container.kind == ContainerKind::Zone && container.name.is_none())
        .map(|(index, _)| index)
        .collect();
    order.sort_by(|&left, &right| {
        containers[left]
            .rect
            .area()
            .partial_cmp(&containers[right].rect.area())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(left.cmp(&right))
    });

    let band = f64::from(ZONE_TITLE_BAND);
    for index in order {
        let zone = &containers[index];
        let mut best: Option<&RawElement> = None;
        for el in elements {
            if el.element_type != "text" || claimed.contains(&el.id) {
                continue;
            }
            let rect = text_rect(el);
            let center = rect.center();
            if !zone.rect.contains_point(center) || center.y > zone.rect.y1 + band {
                continue;
            }
            if nodes.iter().any(|node| node.rect.contains_point(center)) {
                continue;
            }
            best = Some(match best {
                Some(current)
                    if (rect.y1, rect.x1) >= (text_rect(current).y1, text_rect(current).x1) =>
                {
                    current
                }
                _ => el,
            });
        }
        let Some(title) = best else {
            continue;
        };
        let title_id = title.id.clone();
        let title_text = title.text.clone().unwrap_or_default();
        let zone = &mut containers[index];
        zone.name = Some(title_text);
        zone.title_element = Some(title_id.clone());
        zone.title_height = Some(text_size(title).height);
        zone.elements.push(title_id.clone());
        claimed.insert(title_id);
    }
}

/// A container at `p_rect`/`p_z` may parent `k` only when it is behind `k` and
/// contains `k`'s center. A zone child must also be strictly smaller and fully
/// enclosed, so side-by-side swimlanes that only overlap at a border stay siblings.
pub(crate) fn zone_admits(
    p_rect: &Rect,
    p_z: usize,
    k_rect: &Rect,
    k_z: usize,
    k_is_zone: bool,
) -> bool {
    if p_z >= k_z || !p_rect.contains_point(k_rect.center()) {
        return false;
    }
    if k_is_zone {
        return p_rect.area() > k_rect.area() && p_rect.contains_rect(k_rect);
    }
    true
}

fn assign_parents(components: &mut [Component], containers: &mut [Container]) {
    // Containers first, in z order, so a later zone sees parents already written.
    let mut zone_order: Vec<usize> = containers
        .iter()
        .enumerate()
        .filter(|(_, container)| container.kind == ContainerKind::Zone)
        .map(|(index, _)| index)
        .collect();
    zone_order.sort_by_key(|&index| containers[index].z);
    for index in zone_order {
        let (frame_id, rect, z, id) = {
            let zone = &containers[index];
            (zone.frame_id.clone(), zone.rect, zone.z, zone.id.clone())
        };
        containers[index].parent = pick_parent(containers, frame_id.as_deref(), rect, z, &id);
    }

    for component in components.iter_mut() {
        component.parent = pick_parent(
            containers,
            component.frame_id.as_deref(),
            component.rect,
            component.z,
            &component.id,
        );
    }
}

fn pick_parent(
    containers: &[Container],
    child_frame_id: Option<&str>,
    target_rect: Rect,
    z: usize,
    self_id: &str,
) -> Option<String> {
    let candidates = parent_candidates(containers, child_frame_id, target_rect, z, self_id);
    let leaves = leaf_candidates(containers, &candidates);
    smallest_parent(&leaves, target_rect)
}

fn is_zone(containers: &[Container], id: &str) -> bool {
    containers
        .iter()
        .any(|container| container.id == id && container.kind == ContainerKind::Zone)
}

fn parent_candidates<'a>(
    containers: &'a [Container],
    child_frame_id: Option<&str>,
    target_rect: Rect,
    z: usize,
    self_id: &str,
) -> Vec<&'a Container> {
    let k_is_zone = is_zone(containers, self_id);
    let mut candidates = Vec::new();
    // A frame parents a child only when the child names that frame. Two missing
    // frame ids are not a match.
    if let Some(frame_id) = child_frame_id {
        for container in containers {
            if container.kind == ContainerKind::Frame && container.id == frame_id {
                candidates.push(container);
            }
        }
    }
    for container in containers {
        if container.kind != ContainerKind::Zone || container.id == self_id {
            continue;
        }
        if zone_admits(&container.rect, container.z, &target_rect, z, k_is_zone) {
            candidates.push(container);
        }
    }
    candidates
}

fn leaf_candidates<'a>(
    containers: &'a [Container],
    candidates: &[&'a Container],
) -> Vec<&'a Container> {
    candidates
        .iter()
        .copied()
        .filter(|candidate| {
            !candidates.iter().any(|other| {
                other.id != candidate.id && ancestor_of(containers, &candidate.id, &other.id)
            })
        })
        .collect()
}

fn ancestor_of(containers: &[Container], ancestor: &str, start: &str) -> bool {
    let mut id = start.to_string();
    let mut seen = HashSet::new();
    while let Some(parent) = containers
        .iter()
        .find(|container| container.id == id)
        .and_then(|container| container.parent.clone())
    {
        if parent == ancestor {
            return true;
        }
        if !seen.insert(parent.clone()) {
            return false;
        }
        id = parent;
    }
    false
}

fn smallest_parent(candidates: &[&Container], target_rect: Rect) -> Option<String> {
    let mut ranked: Vec<&Container> = candidates.to_vec();
    ranked.sort_by(|left, right| {
        (left.kind != ContainerKind::Frame)
            .cmp(&(right.kind != ContainerKind::Frame))
            .then(
                (!left.rect.contains_rect(&target_rect))
                    .cmp(&(!right.rect.contains_rect(&target_rect))),
            )
            .then(left.rect.area().total_cmp(&right.rect.area()))
            .then(left.z.cmp(&right.z))
    });
    ranked.first().map(|container| container.id.clone())
}

fn group_should_dissolve(members: &[&RawElement]) -> bool {
    for rectangle in members {
        if rectangle.element_type != "rectangle" {
            continue;
        }
        let rect = rectangle.rect();
        let nested = members
            .iter()
            .filter(|member| member.id != rectangle.id)
            .filter(|member| SHAPE_TYPES.contains(&member.element_type.as_str()))
            .filter(|member| member.z > rectangle.z)
            .filter(|member| rect.contains_point(member.rect().center()))
            .filter(|member| is_zone_child(&rect, &member.rect()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Document;

    fn load(name: &str) -> Scene {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(format!("{name}.excalidraw"));
        let src = std::fs::read_to_string(path).expect("fixture");
        Scene::build(&Document::parse(&src).expect("parse fixture"))
    }

    fn parse_scene(src: &str) -> Scene {
        Scene::build(&Document::parse(src).expect("parse constructed document"))
    }

    #[test]
    fn overlap_resolves_component_id_label_element_and_label() {
        let scene = load("overlap");
        assert_eq!(
            scene.resolve("auth").unwrap(),
            Target::Component("auth".to_string())
        );
        assert_eq!(
            scene.resolve("auth-label").unwrap(),
            Target::Component("auth".to_string())
        );
        assert_eq!(
            scene.resolve("Auth").unwrap(),
            Target::Component("auth".to_string())
        );
    }

    #[test]
    fn icon_group_resolves_member_label_frame_name_and_connector() {
        let scene = load("icon_group");
        assert_eq!(
            scene.resolve("Hn6Bf3Jq8XcV1sMz5LwPe").unwrap(),
            Target::Component("group:kq3Xv9TzL2mPw8RbN5cYd".to_string())
        );
        assert_eq!(
            scene.resolve("Postgres").unwrap(),
            Target::Component("group:kq3Xv9TzL2mPw8RbN5cYd".to_string())
        );
        assert_eq!(
            scene.resolve("Storage").unwrap(),
            Target::Container("Fr8Mw2Nq5Xs1Lp4Kv7RbZ".to_string())
        );
        assert_eq!(
            scene.resolve("Lg7Hs4Kd1Fp8Wq5Er2TyU").unwrap(),
            Target::Connector("Lg7Hs4Kd1Fp8Wq5Er2TyU".to_string())
        );
    }

    #[test]
    fn unknown_query_names_the_query() {
        let scene = load("overlap");
        let err = scene.resolve("nope").unwrap_err().to_string();
        assert_eq!(err, "unknown id \"nope\"");
    }

    #[test]
    fn duplicate_labels_are_ambiguous_in_z_order() {
        // Two rectangles, both labeled "API". The lower-z box is listed first.
        let scene = parse_scene(
            r#"{
                "elements": [
                    {
                        "id": "box-low",
                        "type": "rectangle",
                        "x": 0,
                        "y": 0,
                        "width": 80,
                        "height": 40
                    },
                    {
                        "id": "label-low",
                        "type": "text",
                        "x": 10,
                        "y": 10,
                        "width": 30,
                        "height": 20,
                        "text": "API",
                        "containerId": "box-low"
                    },
                    {
                        "id": "box-high",
                        "type": "rectangle",
                        "x": 200,
                        "y": 0,
                        "width": 80,
                        "height": 40
                    },
                    {
                        "id": "label-high",
                        "type": "text",
                        "x": 210,
                        "y": 10,
                        "width": 30,
                        "height": 20,
                        "text": "API",
                        "containerId": "box-high"
                    }
                ]
            }"#,
        );
        let err = scene.resolve("API").unwrap_err().to_string();
        assert!(err.starts_with("ambiguous id \"API\": matches"), "{err}");
        assert_eq!(err, "ambiguous id \"API\": matches box-low, box-high");
    }
}
