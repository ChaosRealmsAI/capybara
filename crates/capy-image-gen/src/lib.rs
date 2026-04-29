mod apimart;
mod prompt;
mod types;

use std::path::PathBuf;

use serde_json::{json, Value};
use thiserror::Error;

pub use prompt::{
    missing_cutout_prompt_requirements, missing_prompt_sections, validate_cutout_prompt,
    validate_prompt_sections, REQUIRED_PROMPT_SECTIONS,
};
pub use types::{
    DoctorReport, GenerateImageRequest, ImageGenerateMode, ImageProviderId, ProviderInfo,
};

#[derive(Debug, Error)]
pub enum ImageGenError {
    #[error("{0}")]
    Message(String),
}

pub type Result<T> = std::result::Result<T, ImageGenError>;

pub fn providers() -> Vec<ProviderInfo> {
    vec![apimart::provider_info()]
}

pub fn doctor(provider: ImageProviderId) -> DoctorReport {
    match provider {
        ImageProviderId::ApimartGptImage2 => apimart::doctor(),
    }
}

pub fn balance(provider: ImageProviderId) -> Result<Value> {
    match provider {
        ImageProviderId::ApimartGptImage2 => apimart::balance(),
    }
}

pub fn generate_image(request: GenerateImageRequest) -> Result<Value> {
    request.validate()?;
    match request.mode {
        ImageGenerateMode::DryRun => Ok(dry_run_response(&request)),
        ImageGenerateMode::Generate | ImageGenerateMode::SubmitOnly | ImageGenerateMode::Resume => {
            match request.provider {
                ImageProviderId::ApimartGptImage2 => apimart::generate(request),
            }
        }
    }
}

fn dry_run_response(request: &GenerateImageRequest) -> Value {
    json!({
        "ok": true,
        "provider": request.provider.as_str(),
        "kind": "image-generation-dry-run",
        "mode": "dry-run",
        "request": request.to_summary_json(),
        "request_body": provider_request_body(request)
    })
}

fn provider_request_body(request: &GenerateImageRequest) -> Value {
    let mut body = json!({
        "model": request.provider.model(),
        "prompt": request.prompt.as_deref().unwrap_or_default(),
        "n": 1,
        "size": request.size
    });
    if !request.resolution.trim().is_empty() {
        body["resolution"] = json!(request.resolution);
    }
    if !request.refs.is_empty() {
        body["image_urls"] = json!(request.refs);
    }
    body
}

pub fn find_downloaded_image_path(value: &Value) -> Option<PathBuf> {
    let mut paths = Vec::new();
    collect_image_paths(value, &mut paths);
    paths.into_iter().find(|path| path.is_file())
}

fn collect_image_paths(value: &Value, paths: &mut Vec<PathBuf>) {
    match value {
        Value::String(text) => {
            let lower = text.to_ascii_lowercase();
            if lower.ends_with(".png")
                || lower.ends_with(".jpg")
                || lower.ends_with(".jpeg")
                || lower.ends_with(".webp")
            {
                paths.push(PathBuf::from(text));
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_image_paths(item, paths);
            }
        }
        Value::Object(object) => {
            for item in object.values() {
                collect_image_paths(item, paths);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_prompt() -> String {
        [
            "Scene: Warm studio.",
            "Subject: One ceramic cup centered.",
            "Important details: Product photo, soft light.",
            "Use case: Hero card.",
            "Constraints: No text, no watermark.",
        ]
        .join(" ")
    }

    #[test]
    fn valid_prompt_has_no_missing_sections() {
        assert!(missing_prompt_sections(&valid_prompt()).is_empty());
    }

    #[test]
    fn bad_prompt_reports_missing_sections() {
        let missing = missing_prompt_sections("cute cat");
        assert!(missing.contains(&"Scene:"));
        assert!(missing.contains(&"Subject:"));
    }

    #[test]
    fn cutout_prompt_reports_missing_requirements() {
        let missing = missing_cutout_prompt_requirements(&valid_prompt());
        assert!(missing.contains(&"neutral matte #E0E0E0 background"));
        assert!(missing.contains(&"fully visible uncropped subject"));
        assert!(missing.contains(&"no green screen"));
    }

    #[test]
    fn cutout_prompt_accepts_isolated_neutral_source() -> Result<()> {
        let prompt = [
            "Scene: Neutral matte #E0E0E0 studio background for cutout source.",
            "Subject: One single ceramic cup centered, fully visible, uncropped, 70% frame height.",
            "Important details: Product photo with clean silhouette, clear edges, soft even light.",
            "Use case: Source for automated alpha cutout and transparent PNG UI composition.",
            "Constraints: No text, no watermark, no extra objects, no green screen, no blue screen, no cast shadow, no reflection.",
        ]
        .join(" ");
        validate_cutout_prompt(&prompt)
    }

    #[test]
    fn dry_run_response_contains_provider_body() -> Result<()> {
        let request = GenerateImageRequest {
            provider: ImageProviderId::ApimartGptImage2,
            mode: ImageGenerateMode::DryRun,
            prompt: Some(valid_prompt()),
            size: "16:9".to_string(),
            resolution: "1k".to_string(),
            refs: Vec::new(),
            output_dir: None,
            name: Some("hero".to_string()),
            download: true,
            task_id: None,
            cutout_ready: false,
        };
        let response = generate_image(request)?;
        assert_eq!(
            response
                .get("request_body")
                .and_then(|body| body.get("model"))
                .and_then(Value::as_str),
            Some("gpt-image-2")
        );
        assert_eq!(
            response
                .get("request_body")
                .and_then(|body| body.get("size"))
                .and_then(Value::as_str),
            Some("16:9")
        );
        Ok(())
    }

    #[test]
    fn find_downloaded_image_path_recurses_into_provider_result(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!(
            "capy-image-gen-path-test-{}.png",
            std::process::id()
        ));
        std::fs::write(&path, b"png")?;
        let value = json!({
            "ok": true,
            "result": {
                "images": [
                    {
                        "local_path": path.display().to_string()
                    }
                ]
            }
        });
        assert_eq!(find_downloaded_image_path(&value), Some(path.clone()));
        let _remove_result = std::fs::remove_file(path);
        Ok(())
    }
}
