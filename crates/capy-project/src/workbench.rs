use crate::model::{
    ArtifactKind, ArtifactRefV1, ProjectWorkbenchCardV1, ProjectWorkbenchV1,
    WORKBENCH_SCHEMA_VERSION, WorkbenchPreviewV1,
};
use crate::package::{ProjectPackage, ProjectPackageResult, now_ms};

const CARD_ORDER: &[(&str, &str)] = &[
    ("image", "图片"),
    ("poster", "海报"),
    ("ppt", "PPT"),
    ("video", "视频"),
    ("web", "网页"),
    ("export_center", "导出中心"),
];

impl ProjectPackage {
    pub fn workbench(&self) -> ProjectPackageResult<ProjectWorkbenchV1> {
        let manifest = self.project_manifest()?;
        let artifacts = self.artifacts()?.artifacts;
        let cards = CARD_ORDER
            .iter()
            .map(|(kind, title)| card_for_kind(kind, title, &artifacts))
            .collect();
        Ok(ProjectWorkbenchV1 {
            schema_version: WORKBENCH_SCHEMA_VERSION.to_string(),
            project_id: manifest.id,
            project_name: manifest.name,
            cards,
            generated_at: now_ms(),
        })
    }
}

fn card_for_kind(kind: &str, title: &str, artifacts: &[ArtifactRefV1]) -> ProjectWorkbenchCardV1 {
    if kind == "export_center" {
        return export_center_card(artifacts);
    }
    let artifact = artifacts
        .iter()
        .find(|artifact| product_kind(artifact) == kind);
    match artifact {
        Some(artifact) => artifact_card(kind, title, artifact),
        None => missing_card(kind, title),
    }
}

fn artifact_card(kind: &str, title: &str, artifact: &ArtifactRefV1) -> ProjectWorkbenchCardV1 {
    ProjectWorkbenchCardV1 {
        id: artifact.id.clone(),
        kind: kind.to_string(),
        title: artifact.title.clone().if_empty(title),
        status: if artifact.evidence_refs.is_empty() {
            "ready".to_string()
        } else {
            "generated".to_string()
        },
        source_path: Some(artifact.source_path.clone()),
        source_refs: artifact.source_refs.clone(),
        output_refs: artifact.output_refs.clone(),
        design_language_refs: artifact.design_language_refs.clone(),
        evidence_refs: artifact.evidence_refs.clone(),
        preview: preview_for(artifact),
        next_actions: vec![
            "select".to_string(),
            "generate".to_string(),
            "open-editor".to_string(),
        ],
        updated_at: artifact.updated_at,
    }
}

fn missing_card(kind: &str, title: &str) -> ProjectWorkbenchCardV1 {
    ProjectWorkbenchCardV1 {
        id: format!("missing_{kind}"),
        kind: kind.to_string(),
        title: title.to_string(),
        status: "missing".to_string(),
        source_path: None,
        source_refs: Vec::new(),
        output_refs: Vec::new(),
        design_language_refs: Vec::new(),
        evidence_refs: Vec::new(),
        preview: WorkbenchPreviewV1 {
            kind: "none".to_string(),
            source_path: None,
            text: Some("等待项目源文件".to_string()),
        },
        next_actions: vec!["register-artifact".to_string()],
        updated_at: 0,
    }
}

fn export_center_card(artifacts: &[ArtifactRefV1]) -> ProjectWorkbenchCardV1 {
    let output_refs = artifacts
        .iter()
        .flat_map(|artifact| artifact.output_refs.clone())
        .collect::<Vec<_>>();
    let evidence_refs = artifacts
        .iter()
        .flat_map(|artifact| artifact.evidence_refs.clone())
        .collect::<Vec<_>>();
    ProjectWorkbenchCardV1 {
        id: "export_center".to_string(),
        kind: "export_center".to_string(),
        title: "导出中心".to_string(),
        status: if output_refs.is_empty() && evidence_refs.is_empty() {
            "waiting".to_string()
        } else {
            "ready".to_string()
        },
        source_path: None,
        source_refs: Vec::new(),
        output_refs,
        design_language_refs: Vec::new(),
        evidence_refs,
        preview: WorkbenchPreviewV1 {
            kind: "summary".to_string(),
            source_path: None,
            text: Some("汇总导出文件、manifest 和 evidence".to_string()),
        },
        next_actions: vec!["review-evidence".to_string()],
        updated_at: artifacts
            .iter()
            .map(|artifact| artifact.updated_at)
            .max()
            .unwrap_or(0),
    }
}

fn preview_for(artifact: &ArtifactRefV1) -> WorkbenchPreviewV1 {
    let kind = match artifact.kind {
        ArtifactKind::Html => "html",
        ArtifactKind::Image => "image",
        ArtifactKind::Markdown => "text",
        ArtifactKind::PosterJson | ArtifactKind::PptJson | ArtifactKind::CompositionJson => "json",
        _ => "none",
    };
    WorkbenchPreviewV1 {
        kind: kind.to_string(),
        source_path: Some(artifact.source_path.clone()),
        text: None,
    }
}

fn product_kind(artifact: &ArtifactRefV1) -> &'static str {
    match artifact.kind {
        ArtifactKind::Html => "web",
        ArtifactKind::Image => "image",
        ArtifactKind::PosterJson => "poster",
        ArtifactKind::PptJson => "ppt",
        ArtifactKind::CompositionJson | ArtifactKind::Video => "video",
        _ => "other",
    }
}

trait IfEmpty {
    fn if_empty(self, fallback: &str) -> String;
}

impl IfEmpty for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.trim().is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ProjectPackage;
    use std::error::Error;

    #[test]
    fn workbench_returns_six_product_cards() -> Result<(), Box<dyn Error>> {
        let package = ProjectPackage::open("../../fixtures/project/html-context")
            .or_else(|_| ProjectPackage::open("fixtures/project/html-context"))?;
        let workbench = package.workbench()?;
        assert_eq!(workbench.cards.len(), 6);
        assert!(workbench.cards.iter().any(|card| card.kind == "web"));
        assert!(
            workbench
                .cards
                .iter()
                .any(|card| card.kind == "export_center")
        );
        Ok(())
    }
}
