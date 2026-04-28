#![allow(dead_code, deprecated)]

use serde::Serialize;
use serde_json::{Value, json};

use crate::component::POSTER_COMPONENT_JS;
use crate::{PosterDocument, Result, validate_document};

const RENDER_SOURCE_SCHEMA: &str = "nf.render_source.v1";
const COMPONENT_ID: &str = "capy.poster-document";

#[derive(Debug, Clone)]
pub struct CompileOptions {
    pub duration_ms: u64,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self { duration_ms: 1000 }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RenderSourceReport {
    pub schema_version: String,
    pub component: String,
    pub duration_ms: u64,
    pub viewport: RenderViewport,
    pub layer_count: usize,
    pub asset_count: usize,
    pub generated_assets: usize,
    pub source: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct RenderViewport {
    pub w: u32,
    pub h: u32,
    pub ratio: String,
}

#[deprecated(note = "use capy-nextframe::compile instead · removed v0.13.14")]
pub(crate) fn compile_render_source(
    document: &PosterDocument,
    options: CompileOptions,
) -> Result<RenderSourceReport> {
    validate_document(document)?;
    let duration_ms = options.duration_ms.max(1);
    let viewport = RenderViewport {
        w: document.canvas.width,
        h: document.canvas.height,
        ratio: ratio(document),
    };
    let generated_assets = document
        .assets
        .values()
        .filter(|asset| asset.provenance.is_some())
        .count();
    let source = json!({
        "schema_version": RENDER_SOURCE_SCHEMA,
        "duration_ms": duration_ms,
        "duration": duration_ms,
        "meta": {
            "name": "Capybara Poster Snapshot",
            "project": "capybara",
            "composition": "poster",
            "version": document.version,
            "render_source_schema": RENDER_SOURCE_SCHEMA,
            "duration_ms": duration_ms,
            "source_document_type": document.doc_type
        },
        "viewport": {
            "w": viewport.w,
            "h": viewport.h,
            "ratio": viewport.ratio
        },
        "theme": {
            "background": document.canvas.background,
            "css": poster_theme_css(&document.canvas.background)
        },
        "assets": asset_manifest(document),
        "components": {
            COMPONENT_ID: POSTER_COMPONENT_JS
        },
        "tracks": [{
            "id": "poster.document",
            "kind": "component",
            "z": 0,
            "clips": [{
                "id": "poster.document.frame",
                "begin": 0,
                "begin_ms": 0,
                "end": duration_ms,
                "end_ms": duration_ms,
                "params": {
                    "component": COMPONENT_ID,
                    "params": {
                        "poster": document
                    },
                    "style": {},
                    "track": {
                        "id": "poster.document",
                        "kind": "component",
                        "source": "capy-poster"
                    }
                }
            }]
        }]
    });
    Ok(RenderSourceReport {
        schema_version: RENDER_SOURCE_SCHEMA.to_string(),
        component: COMPONENT_ID.to_string(),
        duration_ms,
        viewport,
        layer_count: document.layers.len(),
        asset_count: document.assets.len(),
        generated_assets,
        source,
    })
}

fn ratio(document: &PosterDocument) -> String {
    if document.canvas.aspect_ratio.trim().is_empty() {
        format!("{}:{}", document.canvas.width, document.canvas.height)
    } else {
        document.canvas.aspect_ratio.clone()
    }
}

fn poster_theme_css(background: &str) -> String {
    format!(
        ":root {{ --capy-poster-background: {}; }} body {{ background: {}; }}",
        background, background
    )
}

fn asset_manifest(document: &PosterDocument) -> Value {
    Value::Array(
        document
            .assets
            .iter()
            .map(|(id, asset)| {
                json!({
                    "id": id,
                    "type": asset.asset_type,
                    "src": asset.src,
                    "mask": asset.mask,
                    "provenance": asset.provenance
                })
            })
            .collect(),
    )
}
