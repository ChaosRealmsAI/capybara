use capy_shell_mac::{DesktopShell, MacHeadlessShell};

use super::{EXPORT_SEEK_SETTLE, SnapshotError};

pub(super) async fn seek_runtime(
    shell: &MacHeadlessShell,
    t_ms: u64,
) -> Result<serde_json::Value, SnapshotError> {
    let has_export_seek_bridge = shell
        .eval_sync("return !!(window.__nf_seek_export && window.__nf_read_seek_export);")
        .await
        .map_err(|e| SnapshotError::JsCall(format!("{e}")))?
        .as_bool()
        == Some(true);
    let has_video_state_probe = shell
        .eval_sync("return !!(window.__nf && typeof window.__nf.getVideoState === 'function');")
        .await
        .map_err(|e| SnapshotError::JsCall(format!("{e}")))?
        .as_bool()
        == Some(true);

    if has_export_seek_bridge {
        let script = format!("window.__nf_seek_export({t_ms});");
        shell
            .eval_fire_and_forget(&script)
            .map_err(|e| SnapshotError::JsCall(format!("{e}")))?;
        if has_video_state_probe {
            wait_for_video_state_ready(shell, t_ms).await?;
            wait_for_export_seek_ready(shell, t_ms, 0).await
        } else {
            shell.pump_for(EXPORT_SEEK_SETTLE);
            Ok(serde_json::json!({
                "t": t_ms,
                "frameReady": true,
                "seq": 1
            }))
        }
    } else {
        let script = format!("return JSON.stringify(await window.__nf.seek({t_ms}));");
        let v_raw = shell
            .call_async(&script)
            .await
            .map_err(|e| SnapshotError::JsCall(format!("{e}")))?;
        parse_seek_result(v_raw)
    }
}

async fn wait_for_video_state_ready(
    shell: &MacHeadlessShell,
    t_ms: u64,
) -> Result<(), SnapshotError> {
    let started = std::time::Instant::now();
    loop {
        let raw = shell
            .eval_sync(
                "return JSON.stringify((window.__nf && typeof window.__nf.getVideoState === 'function') \
                 ? window.__nf.getVideoState() : { count: 0, clips: [] });",
            )
            .await
            .map_err(|e| SnapshotError::JsCall(format!("video-state poll: {e}")))?;
        let value = parse_seek_result(raw)?;
        if video_state_is_ready(&value, t_ms)? {
            return Ok(());
        }
        if started.elapsed() >= std::time::Duration::from_secs(5) {
            return Err(SnapshotError::FrameReadyContract(format!(
                "video-state not ready at t_ms={t_ms} after 5000ms · payload={value}"
            )));
        }
        tokio::time::sleep(std::time::Duration::from_millis(16)).await;
    }
}

async fn wait_for_export_seek_ready(
    shell: &MacHeadlessShell,
    t_ms: u64,
    min_seq_exclusive: u64,
) -> Result<serde_json::Value, SnapshotError> {
    let started = std::time::Instant::now();
    loop {
        let raw = shell
            .eval_sync("return window.__nf_read_seek_export();")
            .await
            .map_err(|e| SnapshotError::JsCall(format!("export seek poll: {e}")))?;
        let value = parse_seek_result(raw)?;
        let runtime_seq = value
            .get("seq")
            .and_then(|v| v.as_u64())
            .or_else(|| {
                value
                    .get("seq")
                    .and_then(|v| v.as_f64())
                    .filter(|v| v.is_finite() && *v >= 0.0 && v.fract() == 0.0)
                    .map(|v| v as u64)
            })
            .unwrap_or(0);
        if runtime_seq > min_seq_exclusive {
            return Ok(value);
        }
        if started.elapsed() >= std::time::Duration::from_secs(5) {
            return Err(SnapshotError::FrameReadyContract(format!(
                "export seek not ready at t_ms={t_ms} after 5000ms"
            )));
        }
        tokio::time::sleep(std::time::Duration::from_millis(4)).await;
    }
}

fn parse_seek_result(value: serde_json::Value) -> Result<serde_json::Value, SnapshotError> {
    if let Some(s) = value.as_str() {
        serde_json::from_str::<serde_json::Value>(s).map_err(|e| {
            SnapshotError::FrameReadyContract(format!(
                "seek returned non-JSON string: {e} · raw={s}"
            ))
        })
    } else {
        Ok(value)
    }
}

fn video_state_is_ready(value: &serde_json::Value, t_ms: u64) -> Result<bool, SnapshotError> {
    let Some(obj) = value.as_object() else {
        return Err(SnapshotError::FrameReadyContract(format!(
            "video-state expected object at t_ms={t_ms} · got {value}"
        )));
    };
    let count = json_u64(obj.get("count")).unwrap_or(0);
    if count == 0 {
        return Ok(true);
    }
    let Some(clips) = obj.get("clips").and_then(serde_json::Value::as_array) else {
        return Err(SnapshotError::FrameReadyContract(format!(
            "video-state missing clips at t_ms={t_ms} · payload={value}"
        )));
    };
    let target_ms = t_ms as i64;
    for clip in clips {
        let Some(clip_obj) = clip.as_object() else {
            return Err(SnapshotError::FrameReadyContract(format!(
                "video-state clip not object at t_ms={t_ms} · payload={clip}"
            )));
        };
        let frame_ready = clip_obj
            .get("frame_ready")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let ready_state = json_u64(clip_obj.get("ready_state")).unwrap_or(0);
        let current_time_ms = clip_obj
            .get("current_time_ms")
            .and_then(serde_json::Value::as_i64)
            .or_else(|| {
                clip_obj
                    .get("current_time_ms")
                    .and_then(serde_json::Value::as_f64)
                    .filter(|v| v.is_finite())
                    .map(|v| v.round() as i64)
            })
            .unwrap_or(-1);
        if !frame_ready || ready_state < 2 {
            return Ok(false);
        }
        if (current_time_ms - target_ms).abs() > 80 {
            return Ok(false);
        }
    }
    Ok(true)
}

fn json_u64(value: Option<&serde_json::Value>) -> Option<u64> {
    value.and_then(|v| {
        v.as_u64().or_else(|| {
            v.as_f64()
                .filter(|v| v.is_finite() && *v >= 0.0 && v.fract() == 0.0)
                .map(|v| v as u64)
        })
    })
}
