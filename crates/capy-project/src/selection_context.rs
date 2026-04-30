use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{ArtifactKind, ArtifactRefV1, ContextBuildRequest};
use crate::package::{ProjectPackage, ProjectPackageResult};

pub const SELECTION_CONTEXT_SCHEMA_VERSION: &str = "capy.selection-context.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionBoundsV1 {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionContextV1 {
    pub schema_version: String,
    pub scope: String,
    pub kind: String,
    pub artifact_id: String,
    pub source_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_pointer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_json: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<SelectionBoundsV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
    #[serde(default)]
    pub verification_requirements: Vec<String>,
}

impl ProjectPackage {
    pub(crate) fn build_selection_context(
        &self,
        artifact: &ArtifactRefV1,
        request: &ContextBuildRequest,
    ) -> ProjectPackageResult<Option<SelectionContextV1>> {
        if let Some(selector) = request.selector.as_deref() {
            return Ok(Some(
                self.html_selection_context(artifact, request, selector)?,
            ));
        }
        if let Some(pointer) = request.json_pointer.as_deref() {
            return Ok(Some(
                self.json_selection_context(artifact, request, pointer)?,
            ));
        }
        if let Some(surface_node_id) = request.canvas_node.as_deref() {
            return Ok(Some(
                base_context(artifact, request, "artifact", "canvas-node")
                    .with_surface_node(surface_node_id),
            ));
        }
        Ok(None)
    }

    fn html_selection_context(
        &self,
        artifact: &ArtifactRefV1,
        request: &ContextBuildRequest,
        selector: &str,
    ) -> ProjectPackageResult<SelectionContextV1> {
        let source = self.read_artifact_source(&artifact.id)?;
        if artifact.kind != ArtifactKind::Html {
            return Ok(fallback_context(
                artifact,
                request,
                "artifact",
                "file",
                format!("selector {selector} is only deeply supported for html artifacts"),
            ));
        }
        let selected_text = selected_html_text(&source, selector);
        let mut context = base_context(artifact, request, "sub-artifact", "html-section");
        context.selector = Some(selector.to_string());
        context.selected_text = selected_text;
        if context.selected_text.is_none() {
            context.scope = "artifact".to_string();
            context.kind = "file".to_string();
            context.fallback_reason =
                Some(format!("selector {selector} did not resolve in source"));
        }
        Ok(context)
    }

    fn json_selection_context(
        &self,
        artifact: &ArtifactRefV1,
        request: &ContextBuildRequest,
        pointer: &str,
    ) -> ProjectPackageResult<SelectionContextV1> {
        let source = self.read_artifact_source(&artifact.id)?;
        let Ok(json) = serde_json::from_str::<Value>(&source) else {
            return Ok(fallback_context(
                artifact,
                request,
                "artifact",
                "file",
                format!("json pointer {pointer} requires a JSON artifact source"),
            ));
        };
        let mut context = base_context(artifact, request, "sub-artifact", "json-pointer");
        context.json_pointer = Some(pointer.to_string());
        if let Some(value) = json.pointer(pointer) {
            context.selected_json = Some(value.clone());
            context.selected_text = Some(compact_json(value));
        } else {
            context.scope = "artifact".to_string();
            context.kind = "file".to_string();
            context.fallback_reason = Some(format!("json pointer {pointer} did not resolve"));
        }
        Ok(context)
    }
}

impl SelectionContextV1 {
    fn with_surface_node(mut self, surface_node_id: &str) -> Self {
        self.surface_node_id = Some(surface_node_id.to_string());
        self
    }
}

fn base_context(
    artifact: &ArtifactRefV1,
    request: &ContextBuildRequest,
    scope: &str,
    kind: &str,
) -> SelectionContextV1 {
    SelectionContextV1 {
        schema_version: SELECTION_CONTEXT_SCHEMA_VERSION.to_string(),
        scope: scope.to_string(),
        kind: kind.to_string(),
        artifact_id: artifact.id.clone(),
        source_path: artifact.source_path.clone(),
        surface_node_id: request.canvas_node.clone(),
        selector: None,
        json_pointer: None,
        selected_text: None,
        selected_json: None,
        bounds: None,
        fallback_reason: None,
        verification_requirements: vec![
            "Prompt must name the selected target and containing artifact.".to_string(),
            "Review diff must not silently mutate unrelated source.".to_string(),
        ],
    }
}

fn fallback_context(
    artifact: &ArtifactRefV1,
    request: &ContextBuildRequest,
    scope: &str,
    kind: &str,
    reason: String,
) -> SelectionContextV1 {
    let mut context = base_context(artifact, request, scope, kind);
    context.fallback_reason = Some(reason);
    context
}

fn selected_html_text(source: &str, selector: &str) -> Option<String> {
    let attr = selector
        .strip_prefix("[data-capy-section=\"")
        .and_then(|value| value.strip_suffix("\"]"))?;
    let needle = format!("data-capy-section=\"{attr}\"");
    let attr_index = source.find(&needle)?;
    let tag_start = source[..attr_index].rfind('<')?;
    let tag_name_start = tag_start + 1;
    let tag_name_end = source[tag_name_start..]
        .find(|ch: char| ch.is_whitespace() || ch == '>' || ch == '/')
        .map(|index| tag_name_start + index)?;
    let tag_name = &source[tag_name_start..tag_name_end];
    let open_end = source[attr_index..]
        .find('>')
        .map(|index| attr_index + index + 1)?;
    let close_tag = format!("</{tag_name}>");
    let close_start = source[open_end..]
        .find(&close_tag)
        .map(|index| open_end + index)?;
    let text = strip_tags(&source[open_end..close_start]);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn strip_tags(value: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }
    output
}

fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ArtifactKind, ProjectPackage};
    use std::fs;

    #[test]
    fn resolves_html_data_capy_section() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let project = ProjectPackage::init(temp.path(), Some("Selection".to_string()))?;
        fs::create_dir_all(temp.path().join("web"))?;
        fs::write(
            temp.path().join("web/index.html"),
            r#"<main><h1 data-capy-section="hero-title">Draft Title</h1></main>"#,
        )?;
        let artifact = project.add_artifact(
            ArtifactKind::Html,
            "web/index.html",
            "Landing".to_string(),
            Vec::new(),
        )?;
        let context = project
            .build_selection_context(
                &artifact,
                &ContextBuildRequest {
                    artifact_id: artifact.id.clone(),
                    selector: Some("[data-capy-section=\"hero-title\"]".to_string()),
                    canvas_node: Some("node-1".to_string()),
                    json_pointer: None,
                },
            )?
            .ok_or("missing selection context")?;

        assert_eq!(context.kind, "html-section");
        assert_eq!(context.selected_text.as_deref(), Some("Draft Title"));
        assert_eq!(context.surface_node_id.as_deref(), Some("node-1"));
        Ok(())
    }

    #[test]
    fn resolves_json_pointer() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let project = ProjectPackage::init(temp.path(), Some("Selection".to_string()))?;
        fs::create_dir_all(temp.path().join("poster"))?;
        fs::write(
            temp.path().join("poster/poster.json"),
            r#"{"pages":[{"title":"Launch"}]}"#,
        )?;
        let artifact = project.add_artifact(
            ArtifactKind::PosterJson,
            "poster/poster.json",
            "Poster".to_string(),
            Vec::new(),
        )?;
        let context = project
            .build_selection_context(
                &artifact,
                &ContextBuildRequest {
                    artifact_id: artifact.id.clone(),
                    selector: None,
                    canvas_node: None,
                    json_pointer: Some("/pages/0/title".to_string()),
                },
            )?
            .ok_or("missing selection context")?;

        assert_eq!(context.kind, "json-pointer");
        assert_eq!(context.selected_text.as_deref(), Some("\"Launch\""));
        Ok(())
    }
}
