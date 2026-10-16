use std::collections::{BTreeSet, HashMap};
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
        })
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
            if let Some(el) = self.element_object_mut(&id) {
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
