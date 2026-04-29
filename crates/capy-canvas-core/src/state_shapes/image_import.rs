use std::sync::Arc;

use crate::state::{AppState, Shape, ShapeKind};
use crate::state_shapes::ImageAssetImport;

impl AppState {
    pub fn import_image(&mut self, path: &str, x: f64, y: f64) -> usize {
        self.push_undo();
        let mut shape = Shape::new(ShapeKind::Image, x, y, 0xdddddd);
        shape.w = 200.0;
        shape.h = 150.0;
        shape.text = "IMG".to_string();
        shape.image_path = Some(path.to_string());
        shape.metadata.content_kind = Some(crate::shape::CanvasContentKind::Image);
        shape.metadata.title = std::path::Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .map(ToString::to_string)
            .or_else(|| Some("Image".to_string()));
        shape.metadata.source_path = Some(path.to_string());
        self.add_shape(shape)
    }

    pub fn import_image_bytes(
        &mut self,
        x: f64,
        y: f64,
        rgba: Arc<Vec<u8>>,
        width: u32,
        height: u32,
        mime: String,
    ) -> usize {
        self.import_image_asset_bytes(ImageAssetImport {
            x,
            y,
            rgba,
            width,
            height,
            mime,
            title: None,
            source_path: None,
            generation_provider: None,
            generation_prompt: None,
        })
    }

    pub fn import_image_asset_bytes(&mut self, import: ImageAssetImport) -> usize {
        self.push_undo();
        let (nat_w, nat_h) = (import.width as f64, import.height as f64);
        let scale = image_insert_scale(nat_w, nat_h);
        let mut shape = Shape::new(ShapeKind::Image, import.x, import.y, 0xdddddd);
        shape.w = nat_w * scale;
        shape.h = nat_h * scale;
        shape.text = String::new();
        shape.metadata.content_kind = Some(crate::shape::CanvasContentKind::Image);
        shape.metadata.title = import
            .title
            .filter(|value| !value.trim().is_empty())
            .or_else(|| Some("Image".to_string()));
        shape.metadata.status = Some("ready".to_string());
        shape.metadata.mime = Some(import.mime.clone());
        shape.metadata.source_path = import.source_path.filter(|value| !value.trim().is_empty());
        shape.metadata.generation_provider = import
            .generation_provider
            .filter(|value| !value.trim().is_empty());
        shape.metadata.generation_prompt = import
            .generation_prompt
            .filter(|value| !value.trim().is_empty());
        shape.image = Some(crate::shape::RasterImage {
            mime: import.mime,
            width: import.width,
            height: import.height,
            rgba: Some(import.rgba),
            data_url: None,
        });
        self.add_shape(shape)
    }
}

fn image_insert_scale(nat_w: f64, nat_h: f64) -> f64 {
    const MAX_SHAPE_DIM: f64 = 600.0;
    if nat_w > MAX_SHAPE_DIM || nat_h > MAX_SHAPE_DIM {
        (MAX_SHAPE_DIM / nat_w).min(MAX_SHAPE_DIM / nat_h)
    } else {
        1.0
    }
}
