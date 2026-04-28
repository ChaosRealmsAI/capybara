use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{NextFrameError, NextFrameErrorCode};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrandTokens {
    pub values: BTreeMap<String, String>,
}

impl BrandTokens {
    pub fn to_css(&self) -> String {
        let mut text = String::from(":root {\n");
        for (name, value) in &self.values {
            text.push_str("  ");
            text.push_str(name);
            text.push_str(": ");
            text.push_str(value);
            text.push_str(";\n");
        }
        text.push_str("}\n");
        text
    }
}

pub fn load_tokens(path: &Path) -> Result<BrandTokens, NextFrameError> {
    let text = fs::read_to_string(path).map_err(|err| {
        NextFrameError::new(
            NextFrameErrorCode::BrandTokenMissing,
            format!("read brand tokens {} failed: {err}", path.display()),
            "next step · pass an existing --brand-tokens CSS or JSON file",
        )
    })?;
    parse_tokens(path, &text)
}

pub fn parse_tokens(path: &Path, text: &str) -> Result<BrandTokens, NextFrameError> {
    match extension(path).as_deref() {
        Some("css") => parse_css(text),
        Some("json") => parse_json(text),
        _ => Err(NextFrameError::new(
            NextFrameErrorCode::BrandTokenMissing,
            format!("unsupported brand token file extension: {}", path.display()),
            "next step · pass --brand-tokens with a .css or .json file",
        )),
    }
}

pub fn parse_css(text: &str) -> Result<BrandTokens, NextFrameError> {
    let values = text
        .lines()
        .filter_map(css_token_line)
        .collect::<BTreeMap<_, _>>();
    if values.is_empty() {
        return Err(empty_error("CSS"));
    }
    Ok(BrandTokens { values })
}

pub fn parse_json(text: &str) -> Result<BrandTokens, NextFrameError> {
    let value: Value = serde_json::from_str(text).map_err(|err| {
        NextFrameError::new(
            NextFrameErrorCode::BrandTokenMissing,
            format!("brand token JSON is invalid: {err}"),
            "next step · pass valid JSON to --brand-tokens",
        )
    })?;
    let object = token_object(&value).ok_or_else(|| empty_error("JSON"))?;
    let values = object
        .iter()
        .filter_map(|(name, value)| token_value(name, value))
        .collect::<BTreeMap<_, _>>();
    if values.is_empty() {
        return Err(empty_error("JSON"));
    }
    Ok(BrandTokens { values })
}

fn css_token_line(line: &str) -> Option<(String, String)> {
    let without_comment = match line.split("/*").next() {
        Some(value) => value.trim(),
        None => line.trim(),
    };
    let start = without_comment.find("--")?;
    let token = &without_comment[start..];
    let (name, rest) = token.split_once(':')?;
    let value = match rest.split(';').next() {
        Some(value) => value.trim(),
        None => rest.trim(),
    };
    if name.trim().is_empty() || value.is_empty() {
        return None;
    }
    Some((name.trim().to_string(), value.to_string()))
}

fn token_object(value: &Value) -> Option<&serde_json::Map<String, Value>> {
    value
        .get("tokens")
        .or_else(|| value.get("values"))
        .and_then(Value::as_object)
        .or_else(|| value.as_object())
}

fn token_value(name: &str, value: &Value) -> Option<(String, String)> {
    if !name.starts_with("--") {
        return None;
    }
    let value = match value.as_str() {
        Some(value) => value.to_string(),
        None => value.to_string(),
    };
    Some((name.to_string(), value))
}

fn empty_error(format: &str) -> NextFrameError {
    NextFrameError::new(
        NextFrameErrorCode::BrandTokenMissing,
        format!("brand token {format} contains no CSS custom properties"),
        "next step · include at least one --token-name value",
    )
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{BrandTokens, parse_css, parse_json};

    #[test]
    fn parses_css_tokens() -> Result<(), Box<dyn std::error::Error>> {
        let tokens = parse_css(":root {\n  --c-brand-1: #f9a8d4;\n  --r-card: 20px;\n}")?;

        assert_eq!(tokens.values["--c-brand-1"], "#f9a8d4");
        assert_eq!(tokens.values["--r-card"], "20px");
        Ok(())
    }

    #[test]
    fn parses_json_tokens_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let tokens = parse_json(r##"{"tokens":{"--c-brand-1":"#f9a8d4","--r-card":"20px"}}"##)?;
        let text = serde_json::to_string(&tokens)?;
        let decoded: BrandTokens = serde_json::from_str(&text)?;

        assert_eq!(decoded, tokens);
        assert!(decoded.to_css().contains("--c-brand-1: #f9a8d4;"));
        Ok(())
    }

    #[test]
    fn rejects_css_without_custom_properties() -> Result<(), Box<dyn std::error::Error>> {
        let error = match parse_css("body { color: red; }") {
            Ok(_) => return Err("empty CSS should fail".into()),
            Err(error) => error,
        };

        assert_eq!(error.body.code, "BRAND_TOKEN_MISSING");
        Ok(())
    }

    #[test]
    fn rejects_unsupported_extension() -> Result<(), Box<dyn std::error::Error>> {
        let error = match super::parse_tokens(Path::new("tokens.txt"), "--c: red;") {
            Ok(_) => return Err("unsupported extension should fail".into()),
            Err(error) => error,
        };

        assert_eq!(error.body.code, "BRAND_TOKEN_MISSING");
        Ok(())
    }
}
