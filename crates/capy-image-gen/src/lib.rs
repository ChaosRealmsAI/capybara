mod apimart;
mod prompt;
mod types;

use serde_json::{Value, json};
use thiserror::Error;

pub use prompt::{REQUIRED_PROMPT_SECTIONS, missing_prompt_sections, validate_prompt_sections};
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
}
