use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::prompt::{validate_cutout_prompt, validate_prompt_sections};
use crate::{ImageGenError, Result};

pub const VALID_SIZES: &[&str] = &[
    "auto", "1:1", "16:9", "9:16", "4:3", "3:4", "3:2", "2:3", "5:4", "4:5", "2:1", "1:2", "21:9",
    "9:21",
];

pub const VALID_RESOLUTIONS: &[&str] = &["1k", "2k", "4k"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImageProviderId {
    #[serde(rename = "apimart-gpt-image-2")]
    ApimartGptImage2,
}

impl ImageProviderId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ApimartGptImage2 => "apimart-gpt-image-2",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ApimartGptImage2 => "gpt-image-2 via local apimart-image-gen adapter",
        }
    }

    pub fn model(self) -> &'static str {
        match self {
            Self::ApimartGptImage2 => "gpt-image-2",
        }
    }
}

impl fmt::Display for ImageProviderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ImageProviderId {
    type Err = ImageGenError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "apimart-gpt-image-2" => Ok(Self::ApimartGptImage2),
            _ => Err(ImageGenError::Message(format!(
                "unsupported image provider: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImageGenerateMode {
    DryRun,
    SubmitOnly,
    Generate,
    Resume,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: ImageProviderId,
    pub kind: String,
    pub label: String,
    pub model: String,
    pub live_generation_requires_explicit_command: bool,
    pub default_no_spend_gate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub provider: ImageProviderId,
    pub model: String,
    pub checks: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateImageRequest {
    pub provider: ImageProviderId,
    pub mode: ImageGenerateMode,
    pub prompt: Option<String>,
    pub size: String,
    pub resolution: String,
    pub refs: Vec<String>,
    pub output_dir: Option<PathBuf>,
    pub name: Option<String>,
    pub download: bool,
    pub task_id: Option<String>,
    pub cutout_ready: bool,
}

impl GenerateImageRequest {
    pub fn validate(&self) -> Result<()> {
        validate_size(&self.size)?;
        validate_resolution(&self.resolution)?;
        if self.refs.len() > 16 {
            return Err(ImageGenError::Message(format!(
                "image refs exceed limit 16; got {}",
                self.refs.len()
            )));
        }
        match self.mode {
            ImageGenerateMode::Resume => {
                if self
                    .task_id
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .is_empty()
                {
                    return Err(ImageGenError::Message(
                        "--resume requires a non-empty task id".to_string(),
                    ));
                }
            }
            ImageGenerateMode::DryRun
            | ImageGenerateMode::SubmitOnly
            | ImageGenerateMode::Generate => {
                let prompt = self.prompt.as_deref().unwrap_or_default();
                if self.cutout_ready {
                    validate_cutout_prompt(prompt)?;
                } else {
                    validate_prompt_sections(prompt)?;
                }
            }
        }
        Ok(())
    }

    pub fn to_summary_json(&self) -> Value {
        json!({
            "provider": self.provider.as_str(),
            "mode": self.mode,
            "prompt": self.prompt,
            "size": self.size,
            "resolution": self.resolution,
            "refs": self.refs,
            "output_dir": self.output_dir.as_ref().map(|path| path.display().to_string()),
            "name": self.name,
            "download": self.download,
            "task_id": self.task_id,
            "cutout_ready": self.cutout_ready
        })
    }
}

fn validate_size(size: &str) -> Result<()> {
    if VALID_SIZES.contains(&size) {
        Ok(())
    } else {
        Err(ImageGenError::Message(format!(
            "invalid size: {size}; allowed: {}",
            VALID_SIZES.join(" / ")
        )))
    }
}

fn validate_resolution(resolution: &str) -> Result<()> {
    if VALID_RESOLUTIONS.contains(&resolution) {
        Ok(())
    } else {
        Err(ImageGenError::Message(format!(
            "invalid resolution: {resolution}; allowed: {}",
            VALID_RESOLUTIONS.join(" / ")
        )))
    }
}
