use std::collections::BTreeMap;

use serde_json::Value;

use crate::{PosterError, Result};

use super::{PosterDocumentV1, PosterLayerV1, PosterPageV1};

pub fn render_page_svg(document: &PosterDocumentV1, page: &PosterPageV1) -> Result<String> {
    let (width, height) = document.viewport.size().ok_or_else(|| {
        PosterError::Validation("document viewport must include positive w/h".to_string())
    })?;
    let mut renderer = SvgRenderer::new(document, width, height);
    renderer.push_background(page, width, height);
    let mut layers: Vec<&PosterLayerV1> = page
        .layers
        .iter()
        .filter(|layer| layer.visible != Some(false))
        .collect();
    layers.sort_by_key(|layer| layer.z);
    for layer in layers {
        renderer.push_layer(layer)?;
    }
    Ok(renderer.finish())
}

struct SvgRenderer<'a> {
    document: &'a PosterDocumentV1,
    width: u32,
    height: u32,
    defs: Vec<String>,
    body: Vec<String>,
    counter: usize,
}

impl<'a> SvgRenderer<'a> {
    fn new(document: &'a PosterDocumentV1, width: u32, height: u32) -> Self {
        Self {
            document,
            width,
            height,
            defs: Vec::new(),
            body: Vec::new(),
            counter: 0,
        }
    }

    fn push_background(&mut self, page: &PosterPageV1, width: u32, height: u32) {
        let fill = if page.background.trim().is_empty() {
            string_value(&self.document.theme, "background")
                .unwrap_or_else(|| "#fffaf0".to_string())
        } else {
            page.background.clone()
        };
        let fill = self.paint(&fill);
        self.body.push(format!(
            r#"<rect x="0" y="0" width="{width}" height="{height}" fill="{fill}"/>"#
        ));
    }

    fn push_layer(&mut self, layer: &PosterLayerV1) -> Result<()> {
        match layer.kind.as_str() {
            "text" => {
                self.body.push(text_layer(layer));
                Ok(())
            }
            "shape" => {
                let fill =
                    style_value(&layer.style, "fill").unwrap_or_else(|| "transparent".to_string());
                let fill = self.paint(&fill);
                self.body.push(shape_layer(layer, &fill));
                Ok(())
            }
            "image" => {
                self.body.push(image_layer(self.document, layer));
                Ok(())
            }
            "component" => {
                self.body.push(component_layer(self.document, layer)?);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn paint(&mut self, raw: &str) -> String {
        if !raw.trim_start().starts_with("linear-gradient") {
            return xml_attr(raw);
        }
        let colors = hex_colors(raw);
        if colors.len() < 2 {
            return "#fffaf0".to_string();
        }
        self.counter += 1;
        let id = format!("grad-{}", self.counter);
        let last = colors.len().saturating_sub(1).max(1);
        let stops = colors
            .iter()
            .enumerate()
            .map(|(index, color)| {
                let offset = (index * 100) / last;
                format!(r#"<stop offset="{offset}%" stop-color="{color}"/>"#)
            })
            .collect::<String>();
        self.defs.push(format!(
            r#"<linearGradient id="{id}" x1="0%" y1="0%" x2="100%" y2="100%">{stops}</linearGradient>"#
        ));
        format!("url(#{id})")
    }

    fn finish(self) -> String {
        let defs = if self.defs.is_empty() {
            String::new()
        } else {
            format!("<defs>{}</defs>", self.defs.join(""))
        };
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">{defs}{body}</svg>"#,
            w = self.width,
            h = self.height,
            body = self.body.join("")
        )
    }
}

fn text_layer(layer: &PosterLayerV1) -> String {
    let b = layer.bounds;
    let size = number_style(&layer.style, "fontSize").unwrap_or(32.0);
    let line_height = number_style(&layer.style, "lineHeight").unwrap_or(1.08) * size;
    let color = style_value(&layer.style, "color").unwrap_or_else(|| "#1c1917".to_string());
    let weight = style_value(&layer.style, "fontWeight").unwrap_or_else(|| "700".to_string());
    let family = style_value(&layer.style, "fontFamily")
        .unwrap_or_else(|| "PingFang SC, Source Han Sans CN, Arial, sans-serif".to_string());
    let tspans = layer
        .text
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let dy = if index == 0 { 0.0 } else { line_height };
            format!(
                r#"<tspan x="{x}" dy="{dy}">{text}</tspan>"#,
                x = n(b.x),
                dy = n(dy),
                text = xml_text(line)
            )
        })
        .collect::<String>();
    format!(
        r#"<text id="{id}" x="{x}" y="{y}" width="{w}" height="{h}" fill="{color}" font-size="{size}" font-weight="{weight}" font-family="{family}">{tspans}</text>"#,
        id = xml_attr(&layer.id),
        x = n(b.x),
        y = n(b.y + size),
        w = n(b.w),
        h = n(b.h),
        color = xml_attr(&color),
        size = n(size),
        weight = xml_attr(&weight),
        family = xml_attr(&family),
    )
}

fn shape_layer(layer: &PosterLayerV1, fill: &str) -> String {
    let b = layer.bounds;
    let radius = number_style(&layer.style, "radius").unwrap_or(0.0);
    if layer.shape == "ellipse" {
        return format!(
            r#"<ellipse id="{id}" cx="{cx}" cy="{cy}" rx="{rx}" ry="{ry}" fill="{fill}"/>"#,
            id = xml_attr(&layer.id),
            cx = n(b.x + b.w / 2.0),
            cy = n(b.y + b.h / 2.0),
            rx = n(b.w / 2.0),
            ry = n(b.h / 2.0),
        );
    }
    format!(
        r#"<rect id="{id}" x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{fill}"/>"#,
        id = xml_attr(&layer.id),
        x = n(b.x),
        y = n(b.y),
        w = n(b.w),
        h = n(b.h),
        r = n(radius),
    )
}

fn image_layer(document: &PosterDocumentV1, layer: &PosterLayerV1) -> String {
    let b = layer.bounds;
    let asset_key = if layer.asset_ref.trim().is_empty() {
        layer.asset_id.as_str()
    } else {
        layer.asset_ref.as_str()
    };
    let src = document
        .assets
        .get(asset_key)
        .map(|asset| asset.src.as_str())
        .unwrap_or("");
    format!(
        r#"<image id="{id}" href="{src}" x="{x}" y="{y}" width="{w}" height="{h}" preserveAspectRatio="xMidYMid meet"/>"#,
        id = xml_attr(&layer.id),
        src = xml_attr(src),
        x = n(b.x),
        y = n(b.y),
        w = n(b.w),
        h = n(b.h),
    )
}

fn component_layer(document: &PosterDocumentV1, layer: &PosterLayerV1) -> Result<String> {
    let component = document
        .components
        .get(layer.component.as_str())
        .ok_or_else(|| {
            PosterError::Validation(format!(
                "component layer '{}' references missing component",
                layer.id
            ))
        })?;
    let template = component.svg_template().ok_or_else(|| {
        PosterError::Export(format!(
            "component '{}' needs a svg export template for static export",
            layer.component
        ))
    })?;
    let b = layer.bounds;
    let content = apply_template(template, document, layer);
    Ok(format!(
        r#"<svg id="{id}" x="{x}" y="{y}" width="{w}" height="{h}" viewBox="0 0 {w} {h}">{content}</svg>"#,
        id = xml_attr(&layer.id),
        x = n(b.x),
        y = n(b.y),
        w = n(b.w),
        h = n(b.h),
    ))
}

fn apply_template(template: &str, document: &PosterDocumentV1, layer: &PosterLayerV1) -> String {
    let mut out = template.to_string();
    for (key, value) in &layer.params {
        out = out.replace(
            &format!("{{{{params.{key}}}}}"),
            &xml_text(&value_to_string(value)),
        );
    }
    for (key, value) in &document.theme {
        out = out.replace(
            &format!("{{{{theme.{key}}}}}"),
            &xml_attr(&value_to_string(value)),
        );
    }
    out.replace("{{layer.id}}", &xml_attr(&layer.id))
}

fn style_value(style: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    style
        .get(key)
        .map(value_to_string)
        .filter(|value| !value.is_empty())
}

fn string_value(map: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    map.get(key)
        .map(value_to_string)
        .filter(|value| !value.is_empty())
}

fn number_style(style: &BTreeMap<String, Value>, key: &str) -> Option<f64> {
    style.get(key).and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
    })
}

fn value_to_string(value: &Value) -> String {
    value
        .as_str()
        .map(ToString::to_string)
        .unwrap_or_else(|| value.to_string().trim_matches('"').to_string())
}

fn hex_colors(raw: &str) -> Vec<String> {
    raw.split(|ch: char| !(ch.is_ascii_hexdigit() || ch == '#'))
        .filter(|part| part.starts_with('#') && matches!(part.len(), 4 | 7))
        .map(ToString::to_string)
        .collect()
}

fn n(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded.fract() == 0.0 {
        format!("{rounded:.0}")
    } else {
        format!("{rounded:.2}")
    }
}

fn xml_attr(value: &str) -> String {
    xml_text(value).replace('"', "&quot;")
}

fn xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
