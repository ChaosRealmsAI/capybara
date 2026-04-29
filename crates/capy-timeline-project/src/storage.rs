use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use directories::BaseDirs;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

static SLUG_RE: Lazy<Result<Regex, regex::Error>> =
    Lazy::new(|| Regex::new(r"^[a-z][a-z0-9.-]{0,63}$"));

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("storage failed: {0}")]
    StorageFailed(String),
    #[error("invalid slug '{slug}' · hint: {hint}")]
    SlugInvalid { slug: String, hint: String },
    #[error("{0}")]
    ValidationFailed(String),
}

impl From<std::io::Error> for ProjectError {
    fn from(value: std::io::Error) -> Self {
        Self::StorageFailed(value.to_string())
    }
}

impl From<serde_json::Error> for ProjectError {
    fn from(value: serde_json::Error) -> Self {
        Self::StorageFailed(value.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Registry {
    pub projects: Vec<RegistryProject>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryProject {
    pub slug: String,
    pub name: String,
    pub created: String,
    pub last_modified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Project {
    pub slug: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    pub created: String,
    pub modified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Episode {
    pub slug: String,
    pub name: String,
    pub duration: f64,
    #[serde(default)]
    pub anchors: BTreeMap<String, f64>,
    #[serde(default)]
    pub clips: Vec<Value>,
    #[serde(default)]
    pub log: Vec<Value>,
}

pub trait Storage {
    fn load_registry(&self) -> Result<Registry, ProjectError>;
    fn save_registry(&self, registry: &Registry) -> Result<(), ProjectError>;
    fn load_project(&self, slug: &str) -> Result<Project, ProjectError>;
    fn save_project(&self, project: &Project) -> Result<(), ProjectError>;
    fn load_episode(&self, project_slug: &str, episode_slug: &str)
    -> Result<Episode, ProjectError>;
    fn save_episode(&self, project_slug: &str, episode: &Episode) -> Result<(), ProjectError>;
}

#[derive(Debug, Clone)]
pub struct JsonStorage {
    root: PathBuf,
}

impl JsonStorage {
    pub fn default_root() -> Result<PathBuf, ProjectError> {
        if let Some(root) = std::env::var_os("CAPY_TIMELINE_HOME") {
            return Ok(PathBuf::from(root));
        }

        BaseDirs::new()
            .map(|dirs| dirs.home_dir().join(".capybara").join("timeline"))
            .ok_or_else(|| ProjectError::StorageFailed("home directory is unavailable".to_string()))
    }

    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn registry_path(&self) -> PathBuf {
        self.root.join("registry.json")
    }

    fn project_path(&self, slug: &str) -> Result<PathBuf, ProjectError> {
        validate_slug(slug)?;
        Ok(self.root.join(slug).join("project.json"))
    }

    fn episode_path(
        &self,
        project_slug: &str,
        episode_slug: &str,
    ) -> Result<PathBuf, ProjectError> {
        validate_slug(project_slug)?;
        validate_slug(episode_slug)?;
        Ok(self
            .root
            .join(project_slug)
            .join("episodes")
            .join(format!("{episode_slug}.json")))
    }

    fn composition_path(
        &self,
        project_slug: &str,
        composition_slug: &str,
    ) -> Result<PathBuf, ProjectError> {
        validate_slug(project_slug)?;
        validate_slug(composition_slug)?;
        Ok(self
            .root
            .join(project_slug)
            .join("compositions")
            .join(format!("{composition_slug}.json")))
    }

    pub fn load_composition(
        &self,
        project_slug: &str,
        composition_slug: &str,
    ) -> Result<Value, ProjectError> {
        read_json(&self.composition_path(project_slug, composition_slug)?)
    }

    pub fn save_composition(
        &self,
        project_slug: &str,
        composition_slug: &str,
        composition: &Value,
    ) -> Result<(), ProjectError> {
        atomic_write(
            &self.composition_path(project_slug, composition_slug)?,
            composition,
        )
    }

    pub fn composition_exists(
        &self,
        project_slug: &str,
        composition_slug: &str,
    ) -> Result<bool, ProjectError> {
        Ok(self
            .composition_path(project_slug, composition_slug)?
            .exists())
    }
}

impl Storage for JsonStorage {
    fn load_registry(&self) -> Result<Registry, ProjectError> {
        read_json(&self.registry_path())
    }

    fn save_registry(&self, registry: &Registry) -> Result<(), ProjectError> {
        atomic_write(&self.registry_path(), registry)
    }

    fn load_project(&self, slug: &str) -> Result<Project, ProjectError> {
        read_json(&self.project_path(slug)?)
    }

    fn save_project(&self, project: &Project) -> Result<(), ProjectError> {
        validate_slug(&project.slug)?;
        atomic_write(&self.project_path(&project.slug)?, project)
    }

    fn load_episode(
        &self,
        project_slug: &str,
        episode_slug: &str,
    ) -> Result<Episode, ProjectError> {
        read_json(&self.episode_path(project_slug, episode_slug)?)
    }

    fn save_episode(&self, project_slug: &str, episode: &Episode) -> Result<(), ProjectError> {
        validate_slug(&episode.slug)?;
        atomic_write(&self.episode_path(project_slug, &episode.slug)?, episode)
    }
}

pub fn validate_slug(slug: &str) -> Result<(), ProjectError> {
    match &*SLUG_RE {
        Ok(regex) if regex.is_match(slug) => return Ok(()),
        Ok(_) => {}
        Err(err) => {
            return Err(ProjectError::ValidationFailed(format!(
                "built-in slug regex failed to compile: {err}"
            )));
        }
    }

    Err(ProjectError::SlugInvalid {
        slug: slug.to_string(),
        hint:
            "use lowercase letters, numbers, dots, and hyphens; start with a letter; max 64 chars"
                .to_string(),
    })
}

pub fn atomic_write<T: Serialize>(path: &Path, value: &T) -> Result<(), ProjectError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| ProjectError::StorageFailed(err.to_string()))?;
    }

    let tmp_path = unique_tmp_path(path);
    let result = (|| -> Result<(), ProjectError> {
        let json = serde_json::to_string_pretty(value)?;
        let mut tmp = fs::File::create(&tmp_path)?;
        tmp.write_all(json.as_bytes())?;
        tmp.write_all(b"\n")?;
        tmp.sync_all()?;
        fs::rename(&tmp_path, path)?;
        Ok(())
    })();

    if result.is_err() {
        let _cleanup_result = fs::remove_file(&tmp_path);
    }

    result
}

fn unique_tmp_path(path: &Path) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("timeline.json");
    path.with_file_name(format!("{file_name}.tmp-{}-{nonce}", std::process::id()))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, ProjectError> {
    let raw =
        fs::read_to_string(path).map_err(|err| ProjectError::StorageFailed(err.to_string()))?;
    serde_json::from_str(&raw).map_err(|err| ProjectError::StorageFailed(err.to_string()))
}

