use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Args, Subcommand};
use image::GenericImageView;
use serde_json::{Value, json};

use crate::canvas::{absolute_path, canvas_eval, request_data, snapshot};

#[derive(Debug, Args)]
pub(crate) struct CanvasContextArgs {
    #[command(subcommand)]
    pub(crate) command: CanvasContextCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum CanvasContextCommand {
    #[command(about = "Write context.json plus real desktop visual attachments")]
    Export(CanvasContextExportArgs),
}

#[derive(Debug, Args)]
pub(crate) struct CanvasContextExportArgs {
    #[arg(
        long,
        help = "Export the currently selected image as whole-image context"
    )]
    selected: bool,
    #[arg(
        long,
        value_name = "x,y,w,h",
        help = "Canvas-world rectangle for region context; omitted uses the live UI region"
    )]
    region: Option<String>,
    #[arg(long, default_value = "tmp/capy-canvas-context")]
    out: PathBuf,
    #[arg(long)]
    window: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct RectF {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[derive(Debug, Clone, Copy)]
struct RectU32 {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

impl RectF {
    fn normalized(self) -> Self {
        Self {
            x: if self.w < 0.0 {
                self.x + self.w
            } else {
                self.x
            },
            y: if self.h < 0.0 {
                self.y + self.h
            } else {
                self.y
            },
            w: self.w.abs(),
            h: self.h.abs(),
        }
    }

    fn json(self) -> Value {
        json!({
            "x": round2(self.x),
            "y": round2(self.y),
            "w": round2(self.w),
            "h": round2(self.h)
        })
    }
}

pub(crate) fn export_context(args: CanvasContextExportArgs) -> Result<Value, String> {
    let out_dir = absolute_path(args.out)?;
    fs::create_dir_all(&out_dir).map_err(|err| format!("create context dir failed: {err}"))?;

    let initial_snapshot = snapshot(args.window.clone())?;
    let initial_selected = selected_node(&initial_snapshot)?;
    let selected_id = initial_selected
        .get("id")
        .and_then(Value::as_u64)
        .ok_or_else(|| "selected node is missing id".to_string())?;
    focus_selected_node(selected_id, args.window.clone())?;

    let snapshot = snapshot(args.window.clone())?;
    let selected = selected_node(&snapshot)?;
    let selected_id = selected
        .get("id")
        .and_then(Value::as_u64)
        .ok_or_else(|| "selected node is missing id".to_string())?;
    let content_kind = selected
        .get("content_kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if content_kind != "image" {
        return Err(format!(
            "canvas context export requires an image selection; selected node {selected_id} is {content_kind}"
        ));
    }
    let node_bounds = rect_from_value(
        selected
            .get("bounds")
            .or_else(|| selected.get("geometry"))
            .ok_or_else(|| "selected node is missing bounds".to_string())?,
    )?;
    let viewport = snapshot
        .get("canvas")
        .and_then(|canvas| canvas.get("viewport"))
        .ok_or_else(|| "canvas viewport is missing".to_string())?;
    let layout = canvas_eval(&context_layout_script(), args.window.clone())?;
    let canvas_rect = rect_from_value(
        layout
            .get("canvasRect")
            .ok_or_else(|| "canvas layout rect is missing".to_string())?,
    )?;
    let inner_w = layout
        .get("innerWidth")
        .and_then(Value::as_f64)
        .unwrap_or((canvas_rect.x + canvas_rect.w).max(1.0));
    let inner_h = layout
        .get("innerHeight")
        .and_then(Value::as_f64)
        .unwrap_or((canvas_rect.y + canvas_rect.h).max(1.0));

    let window_capture = out_dir.join("window.png");
    let capture_result = request_data(
        "capture",
        json!({
            "out": window_capture.display().to_string(),
            "window": args.window
        }),
    )?;
    let image = image::open(&window_capture)
        .map_err(|err| format!("read built-in app-view capture failed: {err}"))?;
    let (image_w, image_h) = image.dimensions();
    let scale_x = image_w as f64 / inner_w.max(1.0);
    let scale_y = image_h as f64 / inner_h.max(1.0);

    let viewport_path = out_dir.join("viewport.png");
    crop_to_file(
        &image,
        css_rect_to_pixel(canvas_rect, scale_x, scale_y, image_w, image_h)?,
        &viewport_path,
    )?;

    let selected_screen_rect = world_rect_to_window_rect(node_bounds, viewport, canvas_rect)?;
    let selected_node_path = out_dir.join("selected-node.png");
    crop_to_file(
        &image,
        css_rect_to_pixel(selected_screen_rect, scale_x, scale_y, image_w, image_h)?,
        &selected_node_path,
    )?;

    let active_context = layout.get("activeContext").cloned().unwrap_or(Value::Null);
    let region_world =
        if args.selected && args.region.is_none() {
            None
        } else {
            match args.region.as_deref() {
                Some(region) => Some(clamp_rect(parse_rect_arg(region)?, node_bounds).ok_or_else(
                    || "requested region is outside selected image bounds".to_string(),
                )?),
                None => active_context
                    .get("region_bounds_world")
                    .and_then(|value| if value.is_null() { None } else { Some(value) })
                    .map(rect_from_value)
                    .transpose()?
                    .and_then(|region| clamp_rect(region, node_bounds)),
            }
        };
    let export_region = region_world.filter(|region| region.w > 0.0 && region.h > 0.0);
    if args.region.is_none() && !args.selected && export_region.is_none() {
        return Err(
            "missing context target: use --selected or --region x,y,w,h, or draw a live UI region"
                .to_string(),
        );
    }

    let region_path = if let Some(region) = export_region {
        let region_screen_rect = world_rect_to_window_rect(region, viewport, canvas_rect)?;
        let path = out_dir.join("region.png");
        crop_to_file(
            &image,
            css_rect_to_pixel(region_screen_rect, scale_x, scale_y, image_w, image_h)?,
            &path,
        )?;
        Some(path)
    } else {
        None
    };

    let source_path = selected
        .get("source_path")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let source_image_size = source_path
        .as_deref()
        .and_then(|path| image::image_dimensions(path).ok())
        .map(|(w, h)| json!({ "w": w, "h": h }));
    let region_source_pixels = export_region.and_then(|region| {
        let source = source_image_size.as_ref()?;
        let source_w = source.get("w")?.as_f64()?;
        let source_h = source.get("h")?.as_f64()?;
        Some(
            RectF {
                x: ((region.x - node_bounds.x) / node_bounds.w) * source_w,
                y: ((region.y - node_bounds.y) / node_bounds.h) * source_h,
                w: (region.w / node_bounds.w) * source_w,
                h: (region.h / node_bounds.h) * source_h,
            }
            .normalized()
            .json(),
        )
    });

    let kind = if export_region.is_some() {
        "image_region"
    } else {
        "selected_image"
    };
    let region = export_region.map(|region| {
        json!({
            "world": region.json(),
            "source_pixels": region_source_pixels.clone(),
            "node_percent": RectF {
                x: (region.x - node_bounds.x) / node_bounds.w,
                y: (region.y - node_bounds.y) / node_bounds.h,
                w: region.w / node_bounds.w,
                h: region.h / node_bounds.h,
            }
            .normalized()
            .json()
        })
    });
    let context_id = format!("ctx-{}-{selected_id}-{}", kind.replace('_', "-"), now_ms());
    let attachments = json!({
        "window_capture": window_capture.display().to_string(),
        "viewport_png": viewport_path.display().to_string(),
        "selected_node_png": selected_node_path.display().to_string(),
        "region_png": region_path.as_ref().map(|path| path.display().to_string())
    });
    let attachment_paths = [
        Some(window_capture.clone()),
        Some(viewport_path.clone()),
        Some(selected_node_path.clone()),
        region_path.clone(),
    ]
    .into_iter()
    .flatten()
    .map(|path| path.display().to_string())
    .collect::<Vec<_>>();
    let context = json!({
        "schema_version": 1,
        "context_id": context_id,
        "kind": kind,
        "created_at_ms": now_ms(),
        "source": {
            "node": selected,
            "source_path": source_path,
            "source_image_size": source_image_size,
            "canvas_snapshot": {
                "selectedId": snapshot.get("selectedId").cloned().unwrap_or(Value::Null),
                "nodeCount": snapshot.get("canvas").and_then(|canvas| canvas.get("nodeCount")).cloned().unwrap_or(Value::Null),
                "viewport": viewport
            }
        },
        "geometry": {
            "node_world": node_bounds.json(),
            "node_window_css": selected_screen_rect.json(),
            "canvas_window_css": canvas_rect.json(),
            "region_world": export_region.map(RectF::json),
            "region_source_pixels": region_source_pixels
        },
        "region": region,
        "attachments": attachments,
        "attachment_paths": attachment_paths.clone(),
        "capture": capture_result,
        "runtime": {
            "page_errors": layout.get("pageErrors").cloned().unwrap_or_else(|| json!([])),
            "console_errors": layout.get("consoleErrors").cloned().unwrap_or_else(|| json!([])),
            "active_context_preview": active_context
        }
    });
    let context_path = out_dir.join("context.json");
    fs::write(
        &context_path,
        serde_json::to_string_pretty(&context).map_err(|err| err.to_string())?,
    )
    .map_err(|err| format!("write context.json failed: {err}"))?;
    Ok(json!({
        "ok": true,
        "kind": "canvas-context-packet",
        "context_id": context.get("context_id").cloned().unwrap_or(Value::Null),
        "context_kind": kind,
        "out": out_dir.display().to_string(),
        "context_json": context_path.display().to_string(),
        "attachment_paths": attachment_paths,
        "context": context
    }))
}

fn focus_selected_node(id: u64, window: Option<String>) -> Result<(), String> {
    let result = canvas_eval(
        &format!(
            r#"(() => {{
  const focusNode = window.capyWorkbench && window.capyWorkbench.focusNode;
  if (!focusNode) return {{ ok: false, error: "missing capyWorkbench.focusNode" }};
  return {{ ok: Boolean(focusNode({id})) }};
}})()"#
        ),
        window,
    )?;
    if result.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        Ok(())
    } else {
        Err(result
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("selected node could not be focused for context export")
            .to_string())
    }
}

fn selected_node(snapshot: &Value) -> Result<&Value, String> {
    snapshot
        .get("canvas")
        .and_then(|canvas| canvas.get("selectedNode"))
        .filter(|value| !value.is_null())
        .ok_or_else(|| "no selected canvas node".to_string())
}

fn rect_from_value(value: &Value) -> Result<RectF, String> {
    let read = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_f64)
            .ok_or_else(|| format!("rect is missing numeric {key}: {value}"))
    };
    Ok(RectF {
        x: read("x")?,
        y: read("y")?,
        w: read("w").or_else(|_| read("width"))?,
        h: read("h").or_else(|_| read("height"))?,
    }
    .normalized())
}

fn parse_rect_arg(value: &str) -> Result<RectF, String> {
    let parts = value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<f64>()
                .map_err(|err| format!("invalid --region number {part:?}: {err}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if parts.len() != 4 {
        return Err("--region must be x,y,w,h".to_string());
    }
    Ok(RectF {
        x: parts[0],
        y: parts[1],
        w: parts[2],
        h: parts[3],
    }
    .normalized())
}

fn clamp_rect(rect: RectF, bounds: RectF) -> Option<RectF> {
    let x1 = rect.x.max(bounds.x);
    let y1 = rect.y.max(bounds.y);
    let x2 = (rect.x + rect.w).min(bounds.x + bounds.w);
    let y2 = (rect.y + rect.h).min(bounds.y + bounds.h);
    (x2 > x1 && y2 > y1).then_some(RectF {
        x: x1,
        y: y1,
        w: x2 - x1,
        h: y2 - y1,
    })
}

fn world_rect_to_window_rect(
    world: RectF,
    viewport: &Value,
    canvas_rect: RectF,
) -> Result<RectF, String> {
    let zoom = viewport.get("zoom").and_then(Value::as_f64).unwrap_or(1.0);
    let offset = viewport
        .get("camera_offset")
        .ok_or_else(|| "viewport camera_offset is missing".to_string())?;
    let offset_x = offset.get("x").and_then(Value::as_f64).unwrap_or(0.0);
    let offset_y = offset.get("y").and_then(Value::as_f64).unwrap_or(0.0);
    Ok(RectF {
        x: canvas_rect.x + world.x * zoom + offset_x,
        y: canvas_rect.y + world.y * zoom + offset_y,
        w: world.w * zoom,
        h: world.h * zoom,
    }
    .normalized())
}

fn css_rect_to_pixel(
    rect: RectF,
    scale_x: f64,
    scale_y: f64,
    image_w: u32,
    image_h: u32,
) -> Result<RectU32, String> {
    let x1 = (rect.x * scale_x).floor().max(0.0).min(image_w as f64) as u32;
    let y1 = (rect.y * scale_y).floor().max(0.0).min(image_h as f64) as u32;
    let x2 = ((rect.x + rect.w) * scale_x)
        .ceil()
        .max(0.0)
        .min(image_w as f64) as u32;
    let y2 = ((rect.y + rect.h) * scale_y)
        .ceil()
        .max(0.0)
        .min(image_h as f64) as u32;
    let w = x2.saturating_sub(x1);
    let h = y2.saturating_sub(y1);
    if w == 0 || h == 0 {
        return Err(format!(
            "crop rect is empty after clamping: {:?}",
            rect.json()
        ));
    }
    Ok(RectU32 { x: x1, y: y1, w, h })
}

fn crop_to_file(image: &image::DynamicImage, rect: RectU32, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create crop dir failed: {err}"))?;
    }
    let crop = image::imageops::crop_imm(image, rect.x, rect.y, rect.w, rect.h).to_image();
    crop.save(path)
        .map_err(|err| format!("write crop {} failed: {err}", path.display()))
}

fn context_layout_script() -> String {
    r#"(() => {
  const rect = (selector) => {
    const el = document.querySelector(selector);
    if (!el) return { found: false, x: 0, y: 0, w: 0, h: 0, width: 0, height: 0 };
    const box = el.getBoundingClientRect();
    return { found: true, x: box.x, y: box.y, w: box.width, h: box.height, width: box.width, height: box.height };
  };
  const activeContext = window.capyWorkbench?.activeCanvasContext?.() || null;
  return {
    innerWidth: window.innerWidth,
    innerHeight: window.innerHeight,
    devicePixelRatio: window.devicePixelRatio || 1,
    canvasRect: rect('[data-section="canvas-host"]'),
    plannerRect: rect('[data-section="planner-chat"]'),
    contextTitle: document.querySelector('#context-title')?.textContent || '',
    contextMeta: document.querySelector('#context-meta')?.textContent || '',
    activeContext,
    pageErrors: window.__capyPageErrors || [],
    consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
  };
})()"#
        .to_string()
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}
