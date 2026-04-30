use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const COMPONENT_SCHEMA: &str = "capy.component.v1";
pub const VALIDATION_SCHEMA: &str = "capy.component.validation.v1";

#[derive(Debug, Error)]
pub enum ComponentError {
    #[error("component read failed: {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("component JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("component validation failed: {0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, ComponentError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentManifestV1 {
    pub schema: String,
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub surfaces: Vec<String>,
    pub entrypoints: ComponentEntrypointsV1,
    #[serde(default)]
    pub params_schema: Value,
    #[serde(default)]
    pub trusted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentEntrypointsV1 {
    pub runtime: String,
    #[serde(default)]
    pub static_svg: Option<String>,
    #[serde(default)]
    pub style: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ComponentPackage {
    pub dir: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest: ComponentManifestV1,
    pub runtime_path: PathBuf,
    pub runtime: String,
    pub static_svg_path: Option<PathBuf>,
    pub static_svg: Option<String>,
    pub style_path: Option<PathBuf>,
    pub style: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComponentValidationReport {
    pub ok: bool,
    pub schema: &'static str,
    pub root: PathBuf,
    pub components: Vec<ComponentInspection>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComponentInspection {
    pub id: String,
    pub version: String,
    pub manifest_path: PathBuf,
    pub runtime_path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub static_svg_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_path: Option<PathBuf>,
    pub surfaces: Vec<String>,
    pub trusted: bool,
    pub runtime_bytes: usize,
    pub static_svg_bytes: usize,
    pub style_bytes: usize,
    pub exports: ComponentExports,
}

#[derive(Debug, Clone, Copy, Serialize, Default)]
pub struct ComponentExports {
    pub mount: bool,
    pub update: bool,
    pub destroy: bool,
    pub imports: bool,
    pub dynamic_imports: bool,
}

pub fn load_component_package(path: &Path) -> Result<ComponentPackage> {
    let manifest_path = if path.is_dir() {
        path.join("component.json")
    } else {
        path.to_path_buf()
    };
    let dir = manifest_path
        .parent()
        .ok_or_else(|| ComponentError::Validation("component manifest needs a parent dir".into()))?
        .to_path_buf();
    let text = read_text(&manifest_path)?;
    let manifest: ComponentManifestV1 = serde_json::from_str(&text)?;
    validate_manifest(&manifest)?;
    let runtime_path = entrypoint_path(&dir, &manifest.entrypoints.runtime, "runtime")?;
    let runtime = read_text(&runtime_path)?;
    let static_svg_path = optional_entrypoint_path(
        &dir,
        manifest.entrypoints.static_svg.as_deref(),
        "static_svg",
    )?;
    let static_svg = optional_read(&static_svg_path)?;
    let style_path =
        optional_entrypoint_path(&dir, manifest.entrypoints.style.as_deref(), "style")?;
    let style = optional_read(&style_path)?;
    validate_runtime_exports(&manifest.id, &runtime)?;
    Ok(ComponentPackage {
        dir,
        manifest_path,
        manifest,
        runtime_path,
        runtime,
        static_svg_path,
        static_svg,
        style_path,
        style,
    })
}

pub fn validate_components_root(root: &Path) -> ComponentValidationReport {
    let mut report = ComponentValidationReport {
        ok: true,
        schema: VALIDATION_SCHEMA,
        root: root.to_path_buf(),
        components: Vec::new(),
        errors: Vec::new(),
        warnings: Vec::new(),
    };
    let Ok(entries) = fs::read_dir(root) else {
        report.ok = false;
        report
            .errors
            .push(format!("components root missing: {}", root.display()));
        return report;
    };
    let mut seen = BTreeSet::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join("component.json").is_file() {
            push_inspection(&mut report, &path, &mut seen);
        }
    }
    if report.components.is_empty() {
        report
            .warnings
            .push("no component packages found".to_string());
    }
    report.ok = report.errors.is_empty();
    report
}

pub fn inspect_component(path: &Path) -> ComponentValidationReport {
    let root = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let mut report = ComponentValidationReport {
        ok: true,
        schema: VALIDATION_SCHEMA,
        root,
        components: Vec::new(),
        errors: Vec::new(),
        warnings: Vec::new(),
    };
    let mut seen = BTreeSet::new();
    push_inspection(&mut report, path, &mut seen);
    report.ok = report.errors.is_empty();
    report
}

pub fn validate_component_id(component_id: &str) -> Result<()> {
    let mut chars = component_id.chars();
    let Some(first) = chars.next() else {
        return Err(ComponentError::Validation(
            "invalid component id: empty".into(),
        ));
    };
    if !first.is_ascii_lowercase() {
        return Err(ComponentError::Validation(format!(
            "invalid component id '{component_id}': must start with lowercase letter"
        )));
    }
    if component_id.len() > 128
        || component_id.contains("..")
        || component_id
            .chars()
            .any(|ch| !(ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '.' || ch == '-'))
    {
        return Err(ComponentError::Validation(format!(
            "invalid component id '{component_id}': use lowercase letters, numbers, dots, and hyphens"
        )));
    }
    Ok(())
}

pub fn inspect_component_exports(source: &str) -> ComponentExports {
    ComponentExports {
        mount: source.contains("export function mount(")
            || source.contains("export async function mount("),
        update: source.contains("export function update(")
            || source.contains("export async function update("),
        destroy: source.contains("export function destroy(")
            || source.contains("export async function destroy("),
        imports: source
            .lines()
            .map(str::trim_start)
            .any(|line| line.starts_with("import ")),
        dynamic_imports: source.contains("import("),
    }
}

fn push_inspection(
    report: &mut ComponentValidationReport,
    path: &Path,
    seen: &mut BTreeSet<String>,
) {
    match load_component_package(path) {
        Ok(package) => {
            if !seen.insert(package.manifest.id.clone()) {
                report
                    .errors
                    .push(format!("duplicate component id '{}'", package.manifest.id));
            }
            let exports = inspect_component_exports(&package.runtime);
            report.components.push(ComponentInspection {
                id: package.manifest.id,
                version: package.manifest.version,
                manifest_path: package.manifest_path,
                runtime_path: package.runtime_path,
                static_svg_path: package.static_svg_path,
                style_path: package.style_path,
                surfaces: package.manifest.surfaces,
                trusted: package.manifest.trusted,
                runtime_bytes: package.runtime.len(),
                static_svg_bytes: package.static_svg.as_ref().map_or(0, String::len),
                style_bytes: package.style.as_ref().map_or(0, String::len),
                exports,
            });
        }
        Err(err) => report.errors.push(err.to_string()),
    }
}

fn validate_manifest(manifest: &ComponentManifestV1) -> Result<()> {
    if manifest.schema != COMPONENT_SCHEMA {
        return Err(ComponentError::Validation(format!(
            "component '{}' schema must be {COMPONENT_SCHEMA}",
            manifest.id
        )));
    }
    validate_component_id(&manifest.id)?;
    if manifest.version.trim().is_empty() {
        return Err(ComponentError::Validation(format!(
            "component '{}' requires version",
            manifest.id
        )));
    }
    if manifest.entrypoints.runtime.trim().is_empty() {
        return Err(ComponentError::Validation(format!(
            "component '{}' requires entrypoints.runtime",
            manifest.id
        )));
    }
    Ok(())
}

fn validate_runtime_exports(component_id: &str, source: &str) -> Result<()> {
    let exports = inspect_component_exports(source);
    if !exports.mount {
        return Err(ComponentError::Validation(format!(
            "component '{component_id}' missing export function mount"
        )));
    }
    if !exports.update {
        return Err(ComponentError::Validation(format!(
            "component '{component_id}' missing export function update"
        )));
    }
    if exports.imports || exports.dynamic_imports {
        return Err(ComponentError::Validation(format!(
            "component '{component_id}' must be single-file and cannot use import"
        )));
    }
    Ok(())
}

fn optional_entrypoint_path(
    dir: &Path,
    value: Option<&str>,
    name: &str,
) -> Result<Option<PathBuf>> {
    value.map(|raw| entrypoint_path(dir, raw, name)).transpose()
}

fn entrypoint_path(dir: &Path, raw: &str, name: &str) -> Result<PathBuf> {
    let path = Path::new(raw);
    if path.is_absolute() || raw.contains("..") || raw.trim().is_empty() {
        return Err(ComponentError::Validation(format!(
            "entrypoint '{name}' must be a relative file inside the component package"
        )));
    }
    Ok(dir.join(path))
}

fn optional_read(path: &Option<PathBuf>) -> Result<Option<String>> {
    path.as_ref().map(|path| read_text(path)).transpose()
}

fn read_text(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|source| ComponentError::Read {
        path: path.display().to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::{inspect_component_exports, validate_component_id};

    #[test]
    fn validates_component_ids() {
        assert!(validate_component_id("html.capy-title").is_ok());
        assert!(validate_component_id("Html.capy-title").is_err());
        assert!(validate_component_id("html/escape").is_err());
    }

    #[test]
    fn inspects_runtime_exports() {
        let exports = inspect_component_exports(
            "export function mount() {}\nexport function update() {}\nexport function destroy() {}",
        );
        assert!(exports.mount);
        assert!(exports.update);
        assert!(exports.destroy);
    }
}
