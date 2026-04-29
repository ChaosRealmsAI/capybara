use super::{ExportOpts, ExportResolution, ResolvedExportPreset, RUNTIME_IIFE};
use crate::pipeline::VideoCodec;
use crate::record_loop::RecordError;

pub(super) fn resolve_export_preset(
    source_json: &serde_json::Value,
    opts: &ExportOpts,
) -> Result<ResolvedExportPreset, RecordError> {
    let resolution = if let Some(override_resolution) = opts.resolution_override {
        override_resolution
    } else if let Some(raw) = source_json
        .pointer("/meta/export/resolution")
        .and_then(serde_json::Value::as_str)
    {
        ExportResolution::parse_str(raw).ok_or_else(|| {
            RecordError::BundleLoadFailed(format!(
                "meta.export.resolution must be '720p', '1080p' or '4k' (got '{raw}')"
            ))
        })?
    } else {
        match opts.viewport {
            (3840, 2160) => ExportResolution::K4,
            (1280, 720) => ExportResolution::P720,
            _ => ExportResolution::P1080,
        }
    };

    let default_viewport = resolution.viewport();
    let viewport = if opts.resolution_override.is_some()
        || source_json.pointer("/meta/export/resolution").is_some()
    {
        default_viewport
    } else {
        opts.viewport
    };

    let bitrate_bps = if opts.resolution_override.is_some()
        || source_json.pointer("/meta/export/resolution").is_some()
    {
        resolution.bitrate_bps()
    } else {
        opts.bitrate_bps
    };

    let codec = match viewport {
        (3840, 2160) => VideoCodec::HevcMain8,
        _ => resolution.codec(),
    };

    Ok(ResolvedExportPreset {
        viewport,
        bitrate_bps,
        codec,
    })
}

pub(super) fn resolve_stage_background(source_json: &serde_json::Value) -> String {
    let theme = source_json.get("theme").unwrap_or(&serde_json::Value::Null);
    for pointer in ["/background", "/bg", "/colors/background", "/colors/bg"] {
        if let Some(raw) = theme.pointer(pointer).and_then(serde_json::Value::as_str) {
            if let Some(value) = sanitize_stage_background(raw) {
                return value;
            }
        }
    }

    if let Some(css) = theme.get("css").and_then(serde_json::Value::as_str) {
        if let Some(raw) = extract_css_custom_property(css, "--capy-timeline-bg") {
            if let Some(value) = sanitize_stage_background(raw) {
                return value;
            }
        }
    }

    "#000".to_string()
}

fn extract_css_custom_property<'a>(css: &'a str, name: &str) -> Option<&'a str> {
    let start = css.find(name)?;
    let rest = &css[start + name.len()..];
    let colon = rest.find(':')?;
    let rest = &rest[colon + 1..];
    let end = rest.find(';')?;
    Some(rest[..end].trim())
}

fn sanitize_stage_background(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() || value.len() > 96 || value.contains([';', '{', '}']) {
        return None;
    }
    let lower = value.to_ascii_lowercase();
    let named = matches!(
        lower.as_str(),
        "black" | "white" | "transparent" | "canvas" | "currentcolor"
    );
    let functional = lower.starts_with("rgb(")
        || lower.starts_with("rgba(")
        || lower.starts_with("hsl(")
        || lower.starts_with("hsla(");
    let hex = lower.starts_with('#')
        && matches!(lower.len(), 4 | 5 | 7 | 9)
        && lower[1..].chars().all(|ch| ch.is_ascii_hexdigit());
    (named || functional || hex).then(|| value.to_string())
}

/// 构造自包含 export HTML · 含 runtime + __NF_SOURCE__ + mount。
///
/// 关键点(ADR-064):
/// - runtime-iife.js inline (同 preview 同源)
/// - __NF_SOURCE__ 全 JSON inline (同 preview 同源)
/// - body 是输出像素尺寸 · stage 保持 source 逻辑尺寸并整体缩放，同 preview 坐标系
/// - boot({startAtMs:0}) · autoplay · 后续 record_loop 用 seek(t) 精确驱动
/// - 暴露 window.__nf.{seek, getDuration} 给 recorder 调
pub(super) fn build_export_html(
    source_json: &str,
    tracks_map_json: &str,
    output_viewport: (u32, u32),
    logical_viewport: (u32, u32),
    requested_duration_ms: u64,
    stage_background: &str,
) -> String {
    let (out_w, out_h) = output_viewport;
    let (logical_w, logical_h) = logical_viewport;
    let fit = fit_stage_in_output(output_viewport, logical_viewport);
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width={out_w},height={out_h},initial-scale=1,user-scalable=no">
<title>nf-export</title>
<style>
html,body{{margin:0;padding:0;background:#000;width:{out_w}px;height:{out_h}px;overflow:hidden}}
#nf-output{{position:absolute;inset:0;width:{out_w}px;height:{out_h}px;overflow:hidden;background:#000}}
#nf-stage{{position:absolute;top:{stage_top:.6}px;left:{stage_left:.6}px;width:{logical_w}px;height:{logical_h}px;transform-origin:top left;transform:scale({stage_scale:.9});background:{stage_background}}}
</style>
</head>
<body>
<div id="nf-output"><div id="nf-stage"></div></div>
<script>
window.__NF_SOURCE__ = {source_json};
window.__NF_TRACKS__ = {tracks_map_json};
{runtime}

(function(){{
  try {{
    var handle = window.NFRuntime.boot({{ stage: '#nf-stage', autoplay: false, startAtMs: 0 }});
    var REQUESTED_DURATION_MS = {requested_duration_ms};
    window.__nf = window.__nf || {{}};
    window.__nf_handle = handle;
    // Historical: recorder 合约 (v1.14 FrameReadyContract):
    // - seek(t_ms) 返 {{t, frameReady:true, seq}} · t 在 0.01ms 容差内等于 t_ms · seq 单调递增
    // - 外部 t 纯驱动 (ADR-045) · 不依赖 RAF
    var _seq = 0;
    window.__nf_last_seek_json = JSON.stringify({{
      t: 0,
      frameReady: true,
      seq: 0
    }});
    window.__nf_last_media_ready_json = JSON.stringify({{
      ok: true,
      active_videos: 0,
      waited_ms: 0,
      clips: []
    }});
    window.__nf_seek_export = function(t_ms) {{
      var t = Number(t_ms) || 0;
      var h = window.__nf_handle;
      try {{
        if (h && typeof h.seek === 'function') h.seek(t);
      }} catch (e) {{
        // track update 抛错不让录制中断 · 记 console · 继续下一帧
        try {{ console.error('[NF-EXPORT] track.update threw at t=' + t, e && e.message); }} catch (_e) {{}}
      }}
      _seq += 1;
      window.__nf_last_seek_json = JSON.stringify({{
        t: t,
        frameReady: true,
        seq: _seq
      }});
      return true;
    }};
    window.__nf_read_seek_export = function() {{
      return String(window.__nf_last_seek_json || 'null');
    }};
    window.__nf_kick_seek_export = function(t_ms) {{
      var t = Number(t_ms) || 0;
      setTimeout(function() {{
        try {{
          window.__nf_seek_export(t);
        }} catch (e) {{
          window.__nf_last_seek_json = JSON.stringify({{
            t: t,
            frameReady: false,
            seq: _seq,
            error: String((e && e.message) || e || 'seek_export failed')
          }});
        }}
      }}, 0);
      return true;
    }};
    window.__nf_wait_media_export = async function(t_ms) {{
      var t = Number(t_ms) || 0;
      if (window.__nf && typeof window.__nf.waitForMediaReady === 'function') {{
        try {{
          var ready = await window.__nf.waitForMediaReady({{
            t_ms: t,
            timeout_ms: 4500
          }});
          window.__nf_last_media_ready_json = JSON.stringify(ready || {{
            ok: true,
            active_videos: 0,
            waited_ms: 0,
            clips: []
          }});
          return true;
        }} catch (e) {{
          window.__nf_last_media_ready_json = JSON.stringify({{
            ok: false,
            error: String((e && e.message) || e || 'waitForMediaReady failed'),
            t_ms: t
          }});
          return false;
        }}
      }}
      window.__nf_last_media_ready_json = JSON.stringify({{
        ok: true,
        active_videos: 0,
        waited_ms: 0,
        clips: []
      }});
      return true;
    }};
    window.__nf_read_media_ready_export = function() {{
      return String(window.__nf_last_media_ready_json || 'null');
    }};
    window.__nf.seek = function(t_ms) {{
      window.__nf_seek_export(t_ms);
      return window.__nf_read_seek_export();
    }};
    window.__nf.getDuration = function() {{
      var h = window.__nf_handle;
      if (h && typeof h.getDuration === 'function') {{
        var handleDuration = Number(h.getDuration()) || 0;
        return REQUESTED_DURATION_MS > 0
          ? Math.min(handleDuration || REQUESTED_DURATION_MS, REQUESTED_DURATION_MS)
          : handleDuration;
      }}
      // fallback · 读 source.meta.duration_ms / duration_ms / max(track.clips.end_ms)
      var src = window.__NF_SOURCE__ || {{}};
      if (src.meta && typeof src.meta.duration_ms === 'number') {{
        return REQUESTED_DURATION_MS > 0
          ? Math.min(src.meta.duration_ms, REQUESTED_DURATION_MS)
          : src.meta.duration_ms;
      }}
      if (typeof src.duration_ms === 'number') {{
        return REQUESTED_DURATION_MS > 0
          ? Math.min(src.duration_ms, REQUESTED_DURATION_MS)
          : src.duration_ms;
      }}
      var max = 0;
      (src.tracks||[]).forEach(function(tr){{ (tr.clips||[]).forEach(function(c){{
        if (typeof c.end_ms === 'number' && c.end_ms > max) max = c.end_ms;
      }}); }});
      if (REQUESTED_DURATION_MS > 0) return REQUESTED_DURATION_MS;
      return max || 5000;
    }};
    console.log('[NF-EXPORT] runtime booted · output {out_w}x{out_h} · logical {logical_w}x{logical_h} · scale {stage_scale:.6}');
  }} catch (e) {{
    console.error('[NF-EXPORT] boot failed:', e);
  }}
}})();
</script>
</body>
</html>
"#,
        runtime = RUNTIME_IIFE,
        stage_left = fit.left,
        stage_top = fit.top,
        stage_scale = fit.scale,
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct StageFit {
    scale: f64,
    left: f64,
    top: f64,
}

fn fit_stage_in_output(output_viewport: (u32, u32), logical_viewport: (u32, u32)) -> StageFit {
    let out_w = f64::from(output_viewport.0.max(1));
    let out_h = f64::from(output_viewport.1.max(1));
    let logical_w = f64::from(logical_viewport.0.max(1));
    let logical_h = f64::from(logical_viewport.1.max(1));
    let scale = (out_w / logical_w).min(out_h / logical_h);
    let scaled_w = logical_w * scale;
    let scaled_h = logical_h * scale;

    StageFit {
        scale,
        left: (out_w - scaled_w) / 2.0,
        top: (out_h - scaled_h) / 2.0,
    }
}


#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{build_export_html, fit_stage_in_output, resolve_stage_background};

    #[test]
    fn stage_background_uses_theme_css_var() {
        let source = json!({
            "theme": {
                "css": ":root { --capy-timeline-bg: #05070a; --capy-timeline-text: #fff; }"
            }
        });

        assert_eq!(resolve_stage_background(&source), "#05070a");
    }

    #[test]
    fn stage_background_rejects_css_injection() {
        let source = json!({
            "theme": {
                "background": "#000; } body { background: red"
            }
        });

        assert_eq!(resolve_stage_background(&source), "#000");
    }

    #[test]
    fn export_stage_scales_logical_viewport_to_4k_without_relayout() {
        let fit = fit_stage_in_output((3840, 2160), (1920, 1080));

        assert_eq!(fit.scale, 2.0);
        assert_eq!(fit.left, 0.0);
        assert_eq!(fit.top, 0.0);
    }

    #[test]
    fn export_html_keeps_source_viewport_as_runtime_stage() {
        let source = json!({
            "schema_version": "capy.timeline.render_source.v1",
            "duration_ms": 1000,
            "viewport": { "w": 1920, "h": 1080 },
            "theme": { "background": "#05070a" },
            "components": {},
            "tracks": [],
            "assets": []
        });
        let html = build_export_html(
            &serde_json::to_string(&source).unwrap(),
            "{}",
            (3840, 2160),
            (1920, 1080),
            1000,
            "#05070a",
        );

        assert!(html.contains("width:3840px;height:2160px"));
        assert!(html.contains("width:1920px;height:1080px"));
        assert!(html.contains("transform:scale(2.000000000)"));
        assert!(html.contains("\"viewport\""));
        assert!(html.contains("\"w\":1920"));
        assert!(html.contains("\"h\":1080"));
        assert!(!html.contains("\"w\":3840"));
        assert!(!html.contains("\"h\":2160"));
    }
}
