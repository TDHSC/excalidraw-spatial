use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::Read;

use crate::geometry::{Point, Rect};

pub fn json_num(v: f64) -> serde_json::Value {
    let r = (v * 100.0).round() / 100.0;
    if r.fract() == 0.0 && r.abs() < 9e15 {
        serde_json::Value::Number(serde_json::Number::from(r as i64))
    } else {
        serde_json::Value::Number(serde_json::Number::from_f64(r).expect("finite json number"))
    }
}

pub struct Document {
    pub value: serde_json::Value,
    pub elements: Vec<RawElement>,
    trailing_newline: bool,
    index: HashMap<String, usize>,
    touched: BTreeSet<String>,
    created: BTreeSet<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawElement {
    pub id: String,
    #[serde(rename = "type")]
    pub element_type: String,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub is_deleted: Option<bool>,
    pub group_ids: Option<Vec<String>>,
    pub frame_id: Option<String>,
    pub stroke_width: Option<f64>,
    pub text: Option<String>,
    pub font_size: Option<f64>,
    pub line_height: Option<f64>,
    pub text_align: Option<String>,
    pub vertical_align: Option<String>,
    pub container_id: Option<String>,
    pub label_position: Option<f64>,
    pub points: Option<Vec<[f64; 2]>>,
    pub start_binding: Option<Binding>,
    pub end_binding: Option<Binding>,
    pub elbowed: Option<bool>,
    pub name: Option<String>,
    #[serde(skip)]
    pub z: usize,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    pub element_id: Option<String>,
    pub fixed_point: Option<[f64; 2]>,
    pub mode: Option<String>,
}

impl RawElement {
    pub fn x(&self) -> f64 {
        self.x.unwrap_or(0.0)
    }

    pub fn y(&self) -> f64 {
        self.y.unwrap_or(0.0)
    }

    pub fn width(&self) -> f64 {
        self.width.unwrap_or(0.0)
    }

    pub fn height(&self) -> f64 {
        self.height.unwrap_or(0.0)
    }

    pub fn stroke_width(&self) -> f64 {
        self.stroke_width.unwrap_or(2.0)
    }

    pub fn font_size(&self) -> f64 {
        self.font_size.unwrap_or(20.0)
    }

    pub fn line_height(&self) -> f64 {
        self.line_height.unwrap_or(1.25)
    }

    pub fn is_deleted(&self) -> bool {
        self.is_deleted.unwrap_or(false)
    }

    pub fn group_ids(&self) -> &[String] {
        self.group_ids.as_deref().unwrap_or(&[])
    }

    pub fn outer_group(&self) -> Option<&str> {
        self.group_ids().last().map(String::as_str)
    }

    pub fn abs_points(&self) -> Vec<Point> {
        let origin_x = self.x();
        let origin_y = self.y();
        match &self.points {
            Some(pts) => pts
                .iter()
                .map(|p| Point {
                    x: origin_x + p[0],
                    y: origin_y + p[1],
                })
                .collect(),
            None => vec![Point {
                x: origin_x,
                y: origin_y,
            }],
        }
    }

    pub fn rect(&self) -> Rect {
        if self.element_type == "arrow" || self.element_type == "line" {
            Rect::from_points(&self.abs_points())
        } else {
            Rect::from_xywh(self.x(), self.y(), self.width(), self.height())
        }
    }
}

impl Document {
    pub fn parse(src: &str) -> anyhow::Result<Document> {
        let value: serde_json::Value = serde_json::from_str(src)?;
        if !value.is_object() {
            anyhow::bail!("top-level JSON must be an object");
        }
        if !value.get("elements").map(|e| e.is_array()).unwrap_or(false) {
            anyhow::bail!("elements must be an array");
        }
        let (elements, index) = collect_elements(&value, true);
        Ok(Document {
            value,
            elements,
            trailing_newline: src.ends_with('\n'),
            index,
            touched: BTreeSet::new(),
            created: BTreeSet::new(),
        })
    }

    pub fn empty() -> Document {
        let value = serde_json::json!({
            "type": "excalidraw",
            "version": 2,
            "source": "https://excalidraw.com",
            "elements": [],
            "appState": {
                "gridSize": 20,
                "gridStep": 5,
                "gridModeEnabled": false,
                "viewBackgroundColor": "#ffffff",
                "lockedMultiSelections": {}
            },
            "files": {}
        });
        let (elements, index) = collect_elements(&value, false);
        Document {
            value,
            elements,
            trailing_newline: true,
            index,
            touched: BTreeSet::new(),
            created: BTreeSet::new(),
        }
    }

    pub fn refresh(&mut self) {
        let (elements, index) = collect_elements(&self.value, false);
        self.elements = elements;
        self.index = index;
    }

    pub fn has_raw_id(&self, id: &str) -> bool {
        self.value
            .get("elements")
            .and_then(|elements| elements.as_array())
            .is_some_and(|elements| {
                elements
                    .iter()
                    .any(|el| el.get("id").and_then(|raw| raw.as_str()) == Some(id))
            })
    }

    pub fn insert_element(&mut self, pos: usize, el: serde_json::Value) {
        let id = el
            .get("id")
            .and_then(|raw| raw.as_str())
            .map(str::to_string);
        if let Some(elements) = self
            .value
            .get_mut("elements")
            .and_then(|e| e.as_array_mut())
        {
            let at = pos.min(elements.len());
            elements.insert(at, el);
        }
        if let Some(id) = id {
            self.created.insert(id);
        }
        self.refresh();
    }

    pub fn push_element(&mut self, el: serde_json::Value) {
        let len = self
            .value
            .get("elements")
            .and_then(|e| e.as_array())
            .map(|e| e.len())
            .unwrap_or(0);
        self.insert_element(len, el);
    }

    pub fn remove_elements(&mut self, ids: &HashSet<String>) {
        if let Some(elements) = self
            .value
            .get_mut("elements")
            .and_then(|e| e.as_array_mut())
        {
            elements.retain(|el| {
                el.get("id")
                    .and_then(|raw| raw.as_str())
                    .is_none_or(|id| !ids.contains(id))
            });
        }
        self.touched.retain(|id| !ids.contains(id));
        self.created.retain(|id| !ids.contains(id));
        self.refresh();
    }

    pub fn set_value(&mut self, id: &str, key: &str, val: serde_json::Value) {
        if let Some(el) = self.element_object_mut(id) {
            el.insert(key.to_string(), val);
            self.touched.insert(id.to_string());
        }
    }

    pub fn push_bound_element(&mut self, id: &str, kind: &str, target_id: &str) {
        let Some(el) = self.element_object_mut(id) else {
            return;
        };
        let mut bound = match el.get("boundElements").and_then(|v| v.as_array()) {
            Some(arr) => arr.clone(),
            None => Vec::new(),
        };
        let duplicate = bound.iter().any(|entry| {
            entry.get("id").and_then(|v| v.as_str()) == Some(target_id)
                && entry.get("type").and_then(|v| v.as_str()) == Some(kind)
        });
        if !duplicate {
            bound.push(serde_json::json!({"type": kind, "id": target_id}));
            el.insert("boundElements".to_string(), serde_json::Value::Array(bound));
        }
    }

    pub fn remove_bound_references(&mut self, removed_ids: &HashSet<String>) {
        let Some(elements) = self
            .value
            .get_mut("elements")
            .and_then(|e| e.as_array_mut())
        else {
            return;
        };
        let mut touched = Vec::new();
        for el in elements.iter_mut() {
            let Some(obj) = el.as_object_mut() else {
                continue;
            };
            let id = obj
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if id.is_empty() || removed_ids.contains(&id) {
                continue;
            }
            let mut changed = false;
            if let Some(bound) = obj.get("boundElements").and_then(|v| v.as_array()) {
                let filtered: Vec<serde_json::Value> = bound
                    .iter()
                    .filter(|entry| {
                        entry
                            .get("id")
                            .and_then(|v| v.as_str())
                            .is_none_or(|bound_id| !removed_ids.contains(bound_id))
                    })
                    .cloned()
                    .collect();
                if filtered.len() != bound.len() {
                    obj.insert(
                        "boundElements".to_string(),
                        serde_json::Value::Array(filtered),
                    );
                    changed = true;
                }
            }
            if obj
                .get("containerId")
                .and_then(|v| v.as_str())
                .is_some_and(|container_id| removed_ids.contains(container_id))
            {
                obj.insert("containerId".to_string(), serde_json::Value::Null);
                changed = true;
            }
            if changed {
                touched.push(id);
            }
        }
        for id in touched {
            self.touched.insert(id);
        }
    }

    pub fn load(path: &str) -> anyhow::Result<Document> {
        let src = if path == "-" {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            buf
        } else {
            std::fs::read_to_string(path)?
        };
        Document::parse(&src)
    }

    pub fn to_json_string(&self) -> String {
        let mut s = serde_json::to_string_pretty(&self.value).expect("json value");
        if self.trailing_newline {
            s.push('\n');
        }
        s
    }

    pub fn element(&self, id: &str) -> Option<&RawElement> {
        let z = *self.index.get(id)?;
        self.elements.iter().find(|el| el.z == z)
    }

    pub fn set_num(&mut self, id: &str, key: &str, v: f64) {
        if let Some(el) = self.element_object_mut(id) {
            el.insert(key.to_string(), json_num(v));
            self.touched.insert(id.to_string());
        }
    }

    pub fn set_points(&mut self, id: &str, pts: &[[f64; 2]]) {
        if let Some(el) = self.element_object_mut(id) {
            let arr = pts
                .iter()
                .map(|p| serde_json::Value::Array(vec![json_num(p[0]), json_num(p[1])]))
                .collect();
            el.insert("points".to_string(), serde_json::Value::Array(arr));
            self.touched.insert(id.to_string());
        }
    }

    pub fn finish(&mut self, now_ms: i64) {
        let ids: Vec<String> = self.touched.iter().cloned().collect();
        for id in ids {
            let created = self.created.contains(&id);
            if let Some(el) = self.element_object_mut(&id) {
                if created {
                    el.insert(
                        "updated".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(now_ms)),
                    );
                    continue;
                }
                let version = match el.get("version").and_then(|v| v.as_i64()) {
                    Some(n) => n + 1,
                    None => 1,
                };
                el.insert(
                    "version".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(version)),
                );

                let old_nonce = el
                    .get("versionNonce")
                    .and_then(|v| v.as_i64())
                    .filter(|&n| n >= 0)
                    .unwrap_or(0) as u64;
                let new_nonce =
                    (old_nonce.wrapping_mul(1_103_515_245).wrapping_add(12_345)) % 2_147_483_648;
                el.insert(
                    "versionNonce".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(new_nonce as i64)),
                );

                el.insert(
                    "updated".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(now_ms)),
                );
            }
        }
        self.touched.clear();
        self.created.clear();
        let (elements, index) = collect_elements(&self.value, false);
        self.elements = elements;
        self.index = index;
    }

    fn element_object_mut(
        &mut self,
        id: &str,
    ) -> Option<&mut serde_json::Map<String, serde_json::Value>> {
        let idx = *self.index.get(id)?;
        self.value
            .get_mut("elements")?
            .get_mut(idx)?
            .as_object_mut()
    }
}

fn collect_elements(
    value: &serde_json::Value,
    warn: bool,
) -> (Vec<RawElement>, HashMap<String, usize>) {
    let mut elements = Vec::new();
    let mut index = HashMap::new();
    let Some(arr) = value.get("elements").and_then(|e| e.as_array()) else {
        return (elements, index);
    };
    for (i, item) in arr.iter().enumerate() {
        match serde_json::from_value::<RawElement>(item.clone()) {
            Ok(mut el) => {
                el.z = i;
                index.entry(el.id.clone()).or_insert(i);
                elements.push(el);
            }
            Err(err) => {
                if warn {
                    eprintln!("excal: warning: skip element #{i}: {err}");
                }
            }
        }
    }
    (elements, index)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW_MS: i64 = 1_700_000_000_000;

    fn seed_doc() -> Document {
        Document::parse(
            r#"{
  "type": "excalidraw",
  "version": 2,
  "source": "https://excalidraw.com",
  "elements": [
    {
      "id": "old",
      "type": "rectangle",
      "x": 0,
      "y": 0,
      "width": 10,
      "height": 10,
      "version": 3,
      "versionNonce": 11,
      "updated": 1,
      "boundElements": null,
      "containerId": "gone"
    }
  ],
  "appState": {},
  "files": {}
}
"#,
        )
        .expect("parse")
    }

    fn element<'a>(doc: &'a Document, id: &str) -> &'a serde_json::Value {
        doc.value["elements"]
            .as_array()
            .expect("elements")
            .iter()
            .find(|el| el["id"] == id)
            .unwrap_or_else(|| panic!("missing {id}"))
    }

    #[test]
    fn created_element_keeps_version_when_touched_before_finish() {
        let mut doc = seed_doc();
        let old_nonce = element(&doc, "old")["versionNonce"].clone();
        let old_version = element(&doc, "old")["version"].as_i64().expect("version");

        let created = serde_json::json!({
            "id": "new",
            "type": "rectangle",
            "x": 1,
            "y": 2,
            "version": 1,
            "versionNonce": 99,
            "updated": 0
        });
        doc.push_element(created);
        doc.set_num("new", "x", 40.0);
        doc.set_num("old", "x", 8.0);
        doc.finish(NOW_MS);

        let created_el = element(&doc, "new");
        assert_eq!(created_el["version"], 1);
        assert_eq!(created_el["versionNonce"], 99);
        assert_eq!(created_el["updated"], NOW_MS);
        assert_eq!(created_el["x"], 40);

        let old_el = element(&doc, "old");
        assert_eq!(old_el["version"], old_version + 1);
        assert_ne!(old_el["versionNonce"], old_nonce);
        assert_eq!(old_el["updated"], NOW_MS);
    }

    #[test]
    fn empty_document_matches_authoring_template() {
        let doc = Document::empty();
        assert!(doc.to_json_string().ends_with('\n'));
        let value: serde_json::Value = serde_json::from_str(&doc.to_json_string()).expect("json");
        assert_eq!(
            value,
            serde_json::json!({
                "type": "excalidraw",
                "version": 2,
                "source": "https://excalidraw.com",
                "elements": [],
                "appState": {
                    "gridSize": 20,
                    "gridStep": 5,
                    "gridModeEnabled": false,
                    "viewBackgroundColor": "#ffffff",
                    "lockedMultiSelections": {}
                },
                "files": {}
            })
        );
    }

    #[test]
    fn insert_push_remove_and_refresh_track_created_ids() {
        let mut doc = Document::empty();
        doc.push_element(serde_json::json!({"id": "a", "type": "rectangle", "version": 1}));
        doc.insert_element(
            0,
            serde_json::json!({"id": "b", "type": "text", "version": 1}),
        );
        assert!(doc.has_raw_id("a"));
        assert!(doc.has_raw_id("b"));
        assert_eq!(doc.elements[0].id, "b");
        assert_eq!(doc.elements[1].id, "a");

        let mut removed = std::collections::HashSet::new();
        removed.insert("a".to_string());
        doc.remove_elements(&removed);
        assert!(!doc.has_raw_id("a"));
        assert_eq!(doc.elements.len(), 1);

        doc.value["elements"]
            .as_array_mut()
            .expect("elements")
            .push(serde_json::json!({"id": "raw", "type": "rectangle"}));
        doc.refresh();
        assert!(doc.has_raw_id("raw"));
        assert_eq!(doc.element("raw").map(|el| el.id.as_str()), Some("raw"));
    }

    #[test]
    fn push_bound_element_does_not_touch_and_skips_duplicates() {
        let mut doc = seed_doc();
        doc.push_bound_element("old", "text", "old-label");
        doc.push_bound_element("old", "text", "old-label");
        doc.push_bound_element("old", "arrow", "a1");
        doc.finish(NOW_MS);

        let bound = &element(&doc, "old")["boundElements"];
        assert_eq!(
            bound,
            &serde_json::json!([
                {"type": "text", "id": "old-label"},
                {"type": "arrow", "id": "a1"}
            ])
        );
        assert_eq!(element(&doc, "old")["version"], 3);
    }

    #[test]
    fn remove_bound_references_clears_links_and_marks_touched() {
        let mut doc = seed_doc();
        doc.push_element(serde_json::json!({
            "id": "label",
            "type": "text",
            "version": 1,
            "versionNonce": 5,
            "containerId": "gone",
            "boundElements": [{"type": "arrow", "id": "gone"}]
        }));
        doc.set_value(
            "old",
            "boundElements",
            serde_json::json!([{"type": "text", "id": "label"}, {"type": "arrow", "id": "gone"}]),
        );
        let mut removed = std::collections::HashSet::new();
        removed.insert("gone".to_string());
        doc.remove_bound_references(&removed);
        doc.finish(NOW_MS);

        assert_eq!(
            element(&doc, "old")["boundElements"],
            serde_json::json!([{"type": "text", "id": "label"}])
        );
        assert!(element(&doc, "old")["version"].as_i64().expect("version") > 3);
        assert_eq!(
            element(&doc, "label")["containerId"],
            serde_json::Value::Null
        );
        assert_eq!(element(&doc, "label")["version"], 1);
    }
}
