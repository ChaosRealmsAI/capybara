use std::collections::BTreeSet;
use std::path::Path;

use crate::model::{
    ArtifactKind, ProjectSurfaceNodeV1, ProjectSurfaceNodesV1, SURFACE_NODES_SCHEMA_VERSION,
    SurfaceGeometryV1,
};
use crate::package::{
    CAPY_DIR, ProjectPackage, ProjectPackageError, ProjectPackageResult, now_ms, read_to_string,
};

const SURFACE_NODES_FILE: &str = "surface-nodes.json";

impl ProjectPackage {
    pub fn ensure_surface_nodes(&self) -> ProjectPackageResult<ProjectSurfaceNodesV1> {
        let manifest = self.project_manifest()?;
        let artifacts = self.artifacts()?.artifacts;
        let path = surface_nodes_path(self.root());
        let mut surface_nodes = if path.exists() {
            read_surface_nodes(&path)?
        } else {
            ProjectSurfaceNodesV1 {
                schema_version: SURFACE_NODES_SCHEMA_VERSION.to_string(),
                project_id: manifest.id.clone(),
                nodes: Vec::new(),
            }
        };
        if surface_nodes.schema_version != SURFACE_NODES_SCHEMA_VERSION {
            return Err(ProjectPackageError::Invalid(format!(
                "unsupported surface nodes schema: {}",
                surface_nodes.schema_version
            )));
        }
        surface_nodes.project_id = manifest.id;
        let mut known = surface_nodes
            .nodes
            .iter()
            .map(|node| node.artifact_id.clone())
            .collect::<BTreeSet<_>>();
        let mut changed = !path.exists();
        let updated_at = now_ms();
        for (index, artifact) in artifacts.iter().enumerate() {
            if known.contains(&artifact.id) {
                continue;
            }
            surface_nodes.nodes.push(ProjectSurfaceNodeV1 {
                id: surface_node_id(&artifact.id),
                surface: "canvas".to_string(),
                artifact_id: artifact.id.clone(),
                geometry: default_geometry(index, &artifact.kind),
                status: "ready".to_string(),
                updated_at,
            });
            known.insert(artifact.id.clone());
            changed = true;
        }
        if changed {
            self.write_json(&path, &surface_nodes)?;
            self.touch_project_manifest()?;
        }
        Ok(surface_nodes)
    }

    pub fn update_surface_node_geometry(
        &self,
        node_id: &str,
        geometry: SurfaceGeometryV1,
    ) -> ProjectPackageResult<ProjectSurfaceNodesV1> {
        validate_geometry(geometry)?;
        let mut surface_nodes = self.ensure_surface_nodes()?;
        let Some(node) = surface_nodes
            .nodes
            .iter_mut()
            .find(|node| node.id == node_id)
        else {
            return Err(ProjectPackageError::Invalid(format!(
                "unknown surface node id: {node_id}"
            )));
        };
        node.geometry = geometry;
        node.updated_at = now_ms();
        self.write_json(&surface_nodes_path(self.root()), &surface_nodes)?;
        self.touch_project_manifest()?;
        Ok(surface_nodes)
    }
}

fn read_surface_nodes(path: &Path) -> ProjectPackageResult<ProjectSurfaceNodesV1> {
    let raw = read_to_string(path, "read surface nodes")?;
    serde_json::from_str(&raw).map_err(|source| ProjectPackageError::Json {
        context: format!("parse {}", path.display()),
        source,
    })
}

fn surface_nodes_path(root: &Path) -> std::path::PathBuf {
    root.join(CAPY_DIR).join(SURFACE_NODES_FILE)
}

fn surface_node_id(artifact_id: &str) -> String {
    let safe = artifact_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("surf_{safe}")
}

fn default_geometry(index: usize, kind: &ArtifactKind) -> SurfaceGeometryV1 {
    let column = (index % 3) as f64;
    let row = (index / 3) as f64;
    let (w, h) = match kind {
        ArtifactKind::Html => (420.0, 260.0),
        ArtifactKind::Image => (320.0, 220.0),
        ArtifactKind::PosterJson | ArtifactKind::PptJson | ArtifactKind::CompositionJson => {
            (360.0, 220.0)
        }
        ArtifactKind::Video => (360.0, 220.0),
        _ => (320.0, 180.0),
    };
    SurfaceGeometryV1 {
        x: 96.0 + column * 380.0,
        y: 360.0 + row * 250.0,
        w,
        h,
    }
}

fn validate_geometry(geometry: SurfaceGeometryV1) -> ProjectPackageResult<()> {
    if !geometry.x.is_finite()
        || !geometry.y.is_finite()
        || !geometry.w.is_finite()
        || !geometry.h.is_finite()
    {
        return Err(ProjectPackageError::Invalid(
            "surface node geometry must be finite".to_string(),
        ));
    }
    if geometry.w <= 0.0 || geometry.h <= 0.0 {
        return Err(ProjectPackageError::Invalid(
            "surface node geometry width and height must be positive".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectPackage;
    use std::error::Error;
    use std::fs;

    #[test]
    fn ensure_surface_nodes_creates_one_node_per_artifact() -> Result<(), Box<dyn Error>> {
        let temp = tempfile::tempdir()?;
        fs::create_dir_all(temp.path().join("web"))?;
        fs::write(temp.path().join("web/index.html"), "<h1>Landing</h1>")?;
        let package = ProjectPackage::init(temp.path(), Some("Surface Nodes".to_string()))?;
        let artifact = package.add_artifact(
            ArtifactKind::Html,
            "web/index.html",
            "Landing HTML".to_string(),
            Vec::new(),
        )?;

        let surface_nodes = package.ensure_surface_nodes()?;
        assert_eq!(surface_nodes.schema_version, SURFACE_NODES_SCHEMA_VERSION);
        assert_eq!(surface_nodes.nodes.len(), 1);
        assert_eq!(surface_nodes.nodes[0].id, surface_node_id(&artifact.id));
        assert_eq!(surface_nodes.nodes[0].artifact_id, artifact.id);
        assert_eq!(surface_nodes.nodes[0].geometry.w, 420.0);
        assert!(surface_nodes_path(temp.path()).exists());
        Ok(())
    }

    #[test]
    fn update_surface_node_geometry_persists_canvas_bounds() -> Result<(), Box<dyn Error>> {
        let temp = tempfile::tempdir()?;
        fs::create_dir_all(temp.path().join("assets"))?;
        fs::write(temp.path().join("assets/key.svg"), "<svg/>")?;
        let package = ProjectPackage::init(temp.path(), Some("Surface Nodes".to_string()))?;
        package.add_artifact(
            ArtifactKind::Image,
            "assets/key.svg",
            "Key Visual".to_string(),
            Vec::new(),
        )?;
        let node_id = package.ensure_surface_nodes()?.nodes[0].id.clone();
        let updated = package.update_surface_node_geometry(
            &node_id,
            SurfaceGeometryV1 {
                x: 144.0,
                y: 188.0,
                w: 512.0,
                h: 340.0,
            },
        )?;
        assert_eq!(updated.nodes[0].geometry.x, 144.0);
        let round_trip = read_surface_nodes(&surface_nodes_path(temp.path()))?;
        assert_eq!(round_trip.nodes[0].geometry.h, 340.0);
        Ok(())
    }
}
