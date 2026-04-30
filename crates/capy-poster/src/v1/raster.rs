use image::RgbaImage;
use std::path::Path;
use std::sync::Arc;

use crate::{PosterError, Result};

pub fn render_svg_to_png(svg: &str, out: &Path) -> Result<RgbaImage> {
    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|source| crate::PosterError::Write {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let tree = parse_svg(svg)?;
    let size = tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or_else(|| PosterError::Export("create PNG pixmap failed".to_string()))?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    pixmap
        .save_png(out)
        .map_err(|err| PosterError::Export(format!("write PNG failed: {err}")))?;
    image::open(out)
        .map(|image| image.to_rgba8())
        .map_err(|err| PosterError::Export(format!("read rendered PNG failed: {err}")))
}

fn parse_svg(svg: &str) -> Result<usvg::Tree> {
    let mut fontdb = usvg::fontdb::Database::new();
    fontdb.load_system_fonts();
    let options = usvg::Options {
        fontdb: Arc::new(fontdb),
        ..Default::default()
    };
    usvg::Tree::from_str(svg, &options)
        .map_err(|err| PosterError::Export(format!("parse SVG failed: {err}")))
}
