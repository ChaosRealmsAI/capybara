use std::fs;
use std::path::{Path, PathBuf};

use image::RgbaImage;
use serde::Serialize;
use serde_json::Value;

use crate::{PosterError, Result};

use super::pdf::{PdfPage, write_pdf};
use super::pptx::{PptxPage, write_pptx};
use super::raster::render_svg_to_png;
use super::svg::render_page_svg;
use super::{PosterDocumentV1, PosterPageV1, validate_document_v1, write_document_json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Svg,
    Png,
    Pdf,
    Pptx,
    Json,
}

impl ExportFormat {
    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "svg" => Ok(Self::Svg),
            "png" => Ok(Self::Png),
            "pdf" => Ok(Self::Pdf),
            "pptx" => Ok(Self::Pptx),
            "json" => Ok(Self::Json),
            other => Err(PosterError::Validation(format!(
                "unsupported poster export format '{other}'"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Svg => "svg",
            Self::Png => "png",
            Self::Pdf => "pdf",
            Self::Pptx => "pptx",
            Self::Json => "json",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExportRequest {
    pub document: PosterDocumentV1,
    pub out_dir: PathBuf,
    pub formats: Vec<ExportFormat>,
    pub page: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportReport {
    pub ok: bool,
    pub schema: &'static str,
    pub document_id: String,
    pub out_dir: PathBuf,
    pub formats: Vec<String>,
    pub pages: Vec<ExportedPage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pdf_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pptx_path: Option<PathBuf>,
    pub manifest_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportedPage {
    pub id: String,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub svg_path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub png_path: Option<PathBuf>,
}

struct RenderedPage {
    id: String,
    title: String,
    width: u32,
    height: u32,
    svg_path: PathBuf,
    png_path: PathBuf,
    image: RgbaImage,
    png_bytes: Vec<u8>,
}

pub fn export_document_value(
    document: Value,
    out_dir: PathBuf,
    formats: Vec<ExportFormat>,
    page: Option<String>,
) -> Result<ExportReport> {
    let document: PosterDocumentV1 = serde_json::from_value(document).map_err(PosterError::Json)?;
    export_document(ExportRequest {
        document,
        out_dir,
        formats,
        page,
    })
}

pub fn export_document(req: ExportRequest) -> Result<ExportReport> {
    validate_document_v1(&req.document)?;
    let formats = normalized_formats(req.formats);
    fs::create_dir_all(&req.out_dir).map_err(|source| PosterError::Write {
        path: req.out_dir.display().to_string(),
        source,
    })?;
    let pages = selected_pages(&req.document, req.page.as_deref())?;
    let mut rendered = Vec::new();
    for page in pages {
        rendered.push(render_page(&req.document, page, &req.out_dir)?);
    }
    let has_png = formats.contains(&ExportFormat::Png)
        || formats.contains(&ExportFormat::Pdf)
        || formats.contains(&ExportFormat::Pptx);
    let page_reports = rendered
        .iter()
        .map(|page| ExportedPage {
            id: page.id.clone(),
            title: page.title.clone(),
            width: page.width,
            height: page.height,
            svg_path: page.svg_path.clone(),
            png_path: has_png.then(|| page.png_path.clone()),
        })
        .collect::<Vec<_>>();
    let json_path = if formats.contains(&ExportFormat::Json) {
        let path = req.out_dir.join("document.json");
        write_document_json(&path, &req.document)?;
        Some(path)
    } else {
        None
    };
    let pdf_path = if formats.contains(&ExportFormat::Pdf) {
        let path = req.out_dir.join("document.pdf");
        let pdf_pages = rendered
            .iter()
            .map(|page| PdfPage {
                width: page.width,
                height: page.height,
                image: &page.image,
            })
            .collect::<Vec<_>>();
        write_pdf(&path, &pdf_pages)?;
        Some(path)
    } else {
        None
    };
    let pptx_path = if formats.contains(&ExportFormat::Pptx) {
        let path = req.out_dir.join("document.pptx");
        let pptx_pages = rendered
            .iter()
            .map(|page| PptxPage {
                title: &page.title,
                width: page.width,
                height: page.height,
                png: &page.png_bytes,
            })
            .collect::<Vec<_>>();
        write_pptx(&path, &pptx_pages)?;
        Some(path)
    } else {
        None
    };
    let mut report = ExportReport {
        ok: true,
        schema: "capy.poster.export.v1",
        document_id: req.document.id.clone(),
        out_dir: req.out_dir.clone(),
        formats: formats
            .iter()
            .map(|format| format.as_str().to_string())
            .collect(),
        pages: page_reports,
        json_path,
        pdf_path,
        pptx_path,
        manifest_path: req.out_dir.join("manifest.json"),
    };
    write_manifest(&report)?;
    report.manifest_path = report
        .manifest_path
        .canonicalize()
        .unwrap_or_else(|_| report.manifest_path.clone());
    Ok(report)
}

fn normalized_formats(mut formats: Vec<ExportFormat>) -> Vec<ExportFormat> {
    if formats.is_empty() {
        formats = vec![
            ExportFormat::Svg,
            ExportFormat::Png,
            ExportFormat::Pdf,
            ExportFormat::Pptx,
            ExportFormat::Json,
        ];
    }
    if !formats.contains(&ExportFormat::Svg) {
        formats.push(ExportFormat::Svg);
    }
    formats.sort_by_key(|format| format.as_str());
    formats.dedup();
    formats
}

fn selected_pages<'a>(
    document: &'a PosterDocumentV1,
    page: Option<&str>,
) -> Result<Vec<&'a PosterPageV1>> {
    let Some(page) = page.filter(|value| !value.is_empty() && *value != "all") else {
        return Ok(document.pages.iter().collect());
    };
    document
        .pages
        .iter()
        .find(|item| item.id == page)
        .map(|item| vec![item])
        .ok_or_else(|| PosterError::Validation(format!("page '{page}' not found")))
}

fn render_page(
    document: &PosterDocumentV1,
    page: &PosterPageV1,
    out_dir: &Path,
) -> Result<RenderedPage> {
    let (width, height) = document.viewport.size().ok_or_else(|| {
        PosterError::Validation("document viewport must include positive w/h".to_string())
    })?;
    let slug = slug(&page.id);
    let svg = render_page_svg(document, page)?;
    let svg_path = out_dir.join("svg").join(format!("{slug}.svg"));
    write_text(&svg_path, &svg)?;
    let png_path = out_dir.join("png").join(format!("{slug}.png"));
    let image = render_svg_to_png(&svg, &png_path)?;
    let png_bytes = fs::read(&png_path).map_err(|source| PosterError::Read {
        path: png_path.display().to_string(),
        source,
    })?;
    Ok(RenderedPage {
        id: page.id.clone(),
        title: if page.title.is_empty() {
            page.id.clone()
        } else {
            page.title.clone()
        },
        width,
        height,
        svg_path,
        png_path,
        image,
        png_bytes,
    })
}

fn write_text(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| PosterError::Write {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(path, text).map_err(|source| PosterError::Write {
        path: path.display().to_string(),
        source,
    })
}

fn write_manifest(report: &ExportReport) -> Result<()> {
    let text = serde_json::to_string_pretty(report).map_err(PosterError::Json)?;
    write_text(&report.manifest_path, &(text + "\n"))
}

fn slug(value: &str) -> String {
    let slug = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    if slug.trim_matches('-').is_empty() {
        "page".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn exports_v1_document_delivery_files() -> Result<()> {
        let dir = unique_dir("poster-v1-export")?;
        let document: PosterDocumentV1 = serde_json::from_value(json!({
            "schema": "capy.poster.document.v1",
            "id": "unit-poster",
            "title": "Unit Poster",
            "viewport": { "w": 640, "h": 360 },
            "theme": { "background": "#fffaf0", "accent": "#a78bfa" },
            "assets": {},
            "pages": [{
                "id": "cover",
                "title": "Cover",
                "background": "#fffaf0",
                "layers": [
                    { "id": "bg", "kind": "shape", "shape": "rect", "bounds": { "x": 0, "y": 0, "w": 640, "h": 360 }, "z": 0, "style": { "fill": "#fef3c7" } },
                    { "id": "headline", "kind": "text", "text": "REAL SVG", "bounds": { "x": 40, "y": 70, "w": 420, "h": 90 }, "z": 1, "style": { "fontSize": 52, "fontWeight": 900, "color": "#1c1917" } }
                ]
            }]
        }))
        .map_err(PosterError::Json)?;

        let report = export_document(ExportRequest {
            document,
            out_dir: dir.clone(),
            formats: vec![
                ExportFormat::Svg,
                ExportFormat::Png,
                ExportFormat::Pdf,
                ExportFormat::Pptx,
                ExportFormat::Json,
            ],
            page: None,
        })?;

        assert!(report.manifest_path.is_file());
        assert!(report.pages[0].svg_path.is_file());
        assert!(
            report.pages[0]
                .png_path
                .as_ref()
                .is_some_and(|path| path.is_file())
        );
        assert!(report.pdf_path.as_ref().is_some_and(|path| path.is_file()));
        assert!(report.pptx_path.as_ref().is_some_and(|path| path.is_file()));
        let svg = std::fs::read_to_string(&report.pages[0].svg_path).map_err(|source| {
            PosterError::Read {
                path: report.pages[0].svg_path.display().to_string(),
                source,
            }
        })?;
        assert!(svg.contains("REAL SVG"));
        Ok(())
    }

    #[test]
    fn runtime_only_component_fails_static_export() -> Result<()> {
        let dir = unique_dir("poster-v1-component-export")?;
        let document: PosterDocumentV1 = serde_json::from_value(json!({
            "schema": "capy.poster.document.v1",
            "id": "runtime-only",
            "viewport": { "w": 640, "h": 360 },
            "components": { "html.runtime": "export function mount() {}" },
            "pages": [{
                "id": "cover",
                "layers": [{
                    "id": "cmp",
                    "kind": "component",
                    "component": "html.runtime",
                    "bounds": { "x": 0, "y": 0, "w": 640, "h": 360 }
                }]
            }]
        }))
        .map_err(PosterError::Json)?;

        let error = export_document(ExportRequest {
            document,
            out_dir: dir,
            formats: vec![ExportFormat::Svg],
            page: None,
        })
        .err()
        .map(|err| err.to_string());

        assert!(error.is_some_and(|message| message.contains("needs a svg export template")));
        Ok(())
    }

    fn unique_dir(prefix: &str) -> Result<PathBuf> {
        let dir = std::env::temp_dir().join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|err| PosterError::Export(err.to_string()))?
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).map_err(|source| PosterError::Write {
            path: dir.display().to_string(),
            source,
        })?;
        Ok(dir)
    }
}
