use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CanvasDocument {
    #[serde(default)]
    pub nodes: Vec<CanvasNode>,
    #[serde(default)]
    pub edges: Vec<CanvasEdge>,
    #[serde(default)]
    pub viewport: Viewport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CanvasNode {
    pub id: String,
    pub r#type: String,
    pub x: f64,
    pub y: f64,
    #[serde(default = "default_w")]
    pub w: f64,
    #[serde(default = "default_h")]
    pub h: f64,
    #[serde(default)]
    pub data: serde_json::Value,
}

fn default_w() -> f64 { 280.0 }
fn default_h() -> f64 { 160.0 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CanvasEdge {
    pub id: String,
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Viewport {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, zoom: 1.0 }
    }
}

impl CanvasDocument {
    /// Source node ids of edges whose target is `node_id`.
    pub fn incoming_source_ids(&self, node_id: &str) -> Vec<String> {
        self.edges
            .iter()
            .filter(|e| e.target == node_id)
            .map(|e| e.source.clone())
            .collect()
    }

    pub fn node(&self, node_id: &str) -> Option<&CanvasNode> {
        self.nodes.iter().find(|n| n.id == node_id)
    }
}

pub fn default_canvas_document() -> CanvasDocument {
    CanvasDocument {
        nodes: Vec::new(),
        edges: Vec::new(),
        viewport: Viewport::default(),
    }
}

/// For an AI node `data`, return the content string of the active version.
pub fn active_version_content(data: &serde_json::Value) -> Option<String> {
    let active = data.get("activeVersionId")?.as_str()?;
    let versions = data.get("versions")?.as_array()?;
    versions
        .iter()
        .find(|v| v.get("id").and_then(|i| i.as_str()) == Some(active))
        .and_then(|v| v.get("content").and_then(|c| c.as_str()))
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, ty: &str) -> CanvasNode {
        CanvasNode {
            id: id.to_string(),
            r#type: ty.to_string(),
            x: 0.0,
            y: 0.0,
            w: 280.0,
            h: 160.0,
            data: serde_json::json!({}),
        }
    }

    #[test]
    fn incoming_source_ids_returns_sources_of_edges_targeting_node() {
        let doc = CanvasDocument {
            nodes: vec![node("a", "note"), node("b", "note"), node("c", "ai_analyze")],
            edges: vec![
                CanvasEdge { id: "e1".into(), source: "a".into(), target: "c".into() },
                CanvasEdge { id: "e2".into(), source: "b".into(), target: "c".into() },
                CanvasEdge { id: "e3".into(), source: "a".into(), target: "b".into() },
            ],
            viewport: Viewport::default(),
        };
        let mut got = doc.incoming_source_ids("c");
        got.sort();
        assert_eq!(got, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn default_canvas_document_is_empty_with_unit_viewport() {
        let doc = default_canvas_document();
        assert!(doc.nodes.is_empty());
        assert!(doc.edges.is_empty());
        assert_eq!(doc.viewport.zoom, 1.0);
    }

    #[test]
    fn active_version_content_picks_active_id() {
        let data = serde_json::json!({
            "prompt": "p",
            "versions": [
                { "id": "v1", "content": "first", "createdAt": "t1" },
                { "id": "v2", "content": "second", "createdAt": "t2" }
            ],
            "activeVersionId": "v2",
            "status": "idle",
            "error": null
        });
        assert_eq!(active_version_content(&data).as_deref(), Some("second"));
    }

    #[test]
    fn active_version_content_none_when_no_versions() {
        let data = serde_json::json!({ "prompt": "p", "versions": [], "activeVersionId": null });
        assert_eq!(active_version_content(&data), None);
    }

    #[test]
    fn document_roundtrips_through_json() {
        let doc = default_canvas_document();
        let s = serde_json::to_string(&doc).unwrap();
        let back: CanvasDocument = serde_json::from_str(&s).unwrap();
        assert_eq!(back.nodes.len(), 0);
    }
}
