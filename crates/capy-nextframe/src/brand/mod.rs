pub mod hash;
pub mod tokens;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::brand::hash::tokens_hash;
use crate::brand::tokens::BrandTokens;
use crate::compile::{CompileCompositionRequest, CompileError, compile_composition};
use crate::compose::{CompositionDocument, CompositionTheme};
use crate::error::{NextFrameError, NextFrameErrorCode};

pub use tokens::load_tokens;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RebuildReport {
    pub ok: bool,
    pub trace_id: String,
    pub stage: &'static str,
    pub composition_path: PathBuf,
    pub render_source_path: PathBuf,
    pub theme_hash: String,
    pub previous_theme_hash: String,
    #[serde(skip_serializing_if = "is_false")]
    pub skipped: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<CompileError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebuildRequest {
    pub composition_path: PathBuf,
    pub strict_binary: bool,
}

pub fn copy_tokens(
    source_path: &Path,
    project_root: &Path,
) -> Result<CompositionTheme, NextFrameError> {
    let tokens = load_tokens(source_path)?;
    write_materialized_tokens(&tokens, source_path, project_root)?;
    Ok(CompositionTheme {
        tokens_ref: "tokens/tokens.json".to_string(),
        source_path: absolute_path(source_path).display().to_string(),
        hash: tokens_hash(&tokens),
    })
}

pub fn rebuild(req: RebuildRequest) -> RebuildReport {
    let trace_id = trace_id();
    let composition_path = absolute_path(&req.composition_path);
    let render_source_path = render_source_path(&composition_path);
    let mut composition = match read_composition(&composition_path) {
        Ok(composition) => composition,
        Err(error) => {
            return failed(
                trace_id,
                composition_path,
                render_source_path,
                "",
                "",
                error,
            );
        }
    };
    let previous_theme_hash = match composition.theme.as_ref() {
        Some(theme) => theme.hash.clone(),
        None => String::new(),
    };
    let Some(theme) = composition.theme.clone() else {
        return compile_report(
            trace_id,
            composition_path,
            render_source_path,
            "",
            "",
            req.strict_binary,
        );
    };
    let project_root = match composition_path.parent() {
        Some(parent) => parent.to_path_buf(),
        None => PathBuf::from("."),
    };
    let new_theme = match copy_tokens(Path::new(&theme.source_path), &project_root) {
        Ok(theme) => theme,
        Err(error) => {
            return failed(
                trace_id,
                composition_path,
                render_source_path,
                "",
                &previous_theme_hash,
                CompileError::new(
                    error.body.code,
                    "$.theme.source_path",
                    error.body.message,
                    error.body.hint,
                ),
            );
        }
    };
    if new_theme.hash == previous_theme_hash {
        return RebuildReport {
            ok: true,
            trace_id,
            stage: "rebuild",
            composition_path,
            render_source_path,
            theme_hash: new_theme.hash,
            previous_theme_hash,
            skipped: true,
            errors: Vec::new(),
        };
    }
    composition.theme = Some(new_theme.clone());
    if let Err(error) = write_composition(&composition_path, &composition) {
        return failed(
            trace_id,
            composition_path,
            render_source_path,
            &new_theme.hash,
            &previous_theme_hash,
            error,
        );
    }
    compile_report(
        trace_id,
        composition_path,
        render_source_path,
        &new_theme.hash,
        &previous_theme_hash,
        req.strict_binary,
    )
}

fn write_materialized_tokens(
    tokens: &BrandTokens,
    source_path: &Path,
    project_root: &Path,
) -> Result<(), NextFrameError> {
    let dir = project_root.join("tokens");
    fs::create_dir_all(&dir).map_err(|err| write_error(&dir, err))?;
    fs::write(dir.join("tokens.css"), tokens_css(tokens, source_path)?)
        .map_err(|err| write_error(&dir, err))?;
    let json = serde_json::to_string_pretty(tokens).map_err(|err| {
        NextFrameError::new(
            NextFrameErrorCode::OutDirWriteFailed,
            format!("serialize brand tokens failed: {err}"),
            "next step · inspect brand token values",
        )
    })?;
    fs::write(dir.join("tokens.json"), json + "\n").map_err(|err| write_error(&dir, err))
}

fn tokens_css(tokens: &BrandTokens, source_path: &Path) -> Result<String, NextFrameError> {
    if is_css_path(source_path) {
        return fs::read_to_string(source_path).map_err(|err| {
            NextFrameError::new(
                NextFrameErrorCode::BrandTokenMissing,
                format!(
                    "read brand token CSS {} failed: {err}",
                    source_path.display()
                ),
                "next step · pass an existing --brand-tokens CSS file",
            )
        });
    }
    Ok(tokens.to_css())
}

fn is_css_path(path: &Path) -> bool {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) => extension.eq_ignore_ascii_case("css"),
        None => false,
    }
}

fn read_composition(path: &Path) -> Result<CompositionDocument, CompileError> {
    let text = fs::read_to_string(path).map_err(|err| {
        CompileError::new(
            "COMPOSITION_NOT_FOUND",
            "$",
            format!("read composition failed: {err}"),
            "next step · pass an existing composition.json path",
        )
    })?;
    serde_json::from_str(&text).map_err(|err| {
        CompileError::new(
            "INVALID_COMPOSITION",
            "$",
            format!("composition JSON is invalid: {err}"),
            "next step · rerun capy nextframe compose-poster",
        )
    })
}

fn write_composition(path: &Path, composition: &CompositionDocument) -> Result<(), CompileError> {
    let text = serde_json::to_string_pretty(composition).map_err(|err| {
        CompileError::new(
            "COMPILE_FAILED",
            "$",
            format!("serialize composition failed: {err}"),
            "next step · inspect composition JSON values",
        )
    })?;
    fs::write(path, text + "\n").map_err(|err| {
        CompileError::new(
            "COMPILE_FAILED",
            "$.composition_path",
            format!("write composition failed: {err}"),
            "next step · check composition file permissions",
        )
    })
}

fn compile_report(
    trace_id: String,
    composition_path: PathBuf,
    render_source_path: PathBuf,
    theme_hash: &str,
    previous_theme_hash: &str,
    strict_binary: bool,
) -> RebuildReport {
    let compile = compile_composition(CompileCompositionRequest {
        composition_path: composition_path.clone(),
        strict_binary,
    });
    RebuildReport {
        ok: compile.ok,
        trace_id,
        stage: "rebuild",
        composition_path,
        render_source_path,
        theme_hash: theme_hash.to_string(),
        previous_theme_hash: previous_theme_hash.to_string(),
        skipped: false,
        errors: compile.errors,
    }
}

fn failed(
    trace_id: String,
    composition_path: PathBuf,
    render_source_path: PathBuf,
    theme_hash: &str,
    previous_theme_hash: &str,
    error: CompileError,
) -> RebuildReport {
    RebuildReport {
        ok: false,
        trace_id,
        stage: "rebuild",
        composition_path,
        render_source_path,
        theme_hash: theme_hash.to_string(),
        previous_theme_hash: previous_theme_hash.to_string(),
        skipped: false,
        errors: vec![error],
    }
}

fn write_error(path: &Path, err: std::io::Error) -> NextFrameError {
    NextFrameError::new(
        NextFrameErrorCode::OutDirWriteFailed,
        format!("write brand tokens under {} failed: {err}", path.display()),
        "next step · choose a writable --out directory",
    )
}

fn render_source_path(composition_path: &Path) -> PathBuf {
    match composition_path.parent() {
        Some(parent) => parent.join("render_source.json"),
        None => PathBuf::from("render_source.json"),
    }
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        match std::env::current_dir() {
            Ok(cwd) => cwd.join(path),
            Err(_) => PathBuf::from(path),
        }
    }
}

fn trace_id() -> String {
    let millis = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis(),
        Err(_) => 0,
    };
    format!("rebuild-{millis}-{}", std::process::id())
}

fn is_false(value: &bool) -> bool {
    !*value
}
