use std::time::{Duration, Instant};

use capy_shell_mac::MacHeadlessShell;

use super::{RecordError, FRAME_SEEK_TIMEOUT};

pub(crate) async fn wait_for_video_state_ready(
    shell: &MacHeadlessShell,
    expected_t: f64,
) -> Result<(), RecordError> {
    let started = Instant::now();
    loop {
        let raw = shell
            .eval_sync(
                "return JSON.stringify((window.__nf && typeof window.__nf.getVideoState === 'function') \
                 ? window.__nf.getVideoState() : { count: 0, clips: [] });",
            )
            .await?;
        let value = parse_json_result(raw, "video-state")?;
        if video_state_is_ready(&value, expected_t)? {
            return Ok(());
        }
        if started.elapsed() >= FRAME_SEEK_TIMEOUT {
            return Err(RecordError::FrameReadyTimeout(format!(
                "video-state not ready after {}ms at expected_t={expected_t:.6}",
                FRAME_SEEK_TIMEOUT.as_millis()
            )));
        }
        tokio::time::sleep(Duration::from_millis(16)).await;
    }
}

pub(crate) async fn wait_for_export_seek_ready(
    shell: &MacHeadlessShell,
    expected_t: f64,
    min_seq_exclusive: u64,
) -> Result<serde_json::Value, RecordError> {
    let started = Instant::now();
    loop {
        let raw = shell
            .eval_sync("return window.__nf_read_seek_export();")
            .await?;
        let value = parse_json_result(raw, "export seek result")?;
        let runtime_seq = js_number_as_u64(value.get("seq")).unwrap_or(0);
        if runtime_seq > min_seq_exclusive {
            verify_frame_ready(&value, expected_t, Some(min_seq_exclusive))?;
            return Ok(value);
        }
        if started.elapsed() >= FRAME_SEEK_TIMEOUT {
            return Err(RecordError::FrameReadyTimeout(format!(
                "export seek not ready after {}ms at expected_t={expected_t:.6}",
                FRAME_SEEK_TIMEOUT.as_millis()
            )));
        }
        tokio::time::sleep(Duration::from_millis(4)).await;
    }
}

pub(crate) fn verify_frame_ready(
    value: &serde_json::Value,
    expected_t: f64,
    min_seq_exclusive: Option<u64>,
) -> Result<(), RecordError> {
    let obj = value.as_object().ok_or_else(|| {
        RecordError::FrameReadyContract(format!(
            "expected object at expected_t={expected_t:.6} · got: {value}"
        ))
    })?;

    let ready = obj
        .get("frameReady")
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| {
            RecordError::FrameReadyContract(format!(
                "missing frameReady boolean at expected_t={expected_t:.6}"
            ))
        })?;
    if !ready {
        return Err(RecordError::FrameReadyContract(format!(
            "frameReady=false at expected_t={expected_t:.6}"
        )));
    }

    let received_t = obj
        .get("t")
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| {
            RecordError::FrameReadyContract(format!(
                "missing t (f64) at expected_t={expected_t:.6}"
            ))
        })?;
    if (received_t - expected_t).abs() > 0.01 {
        return Err(RecordError::FrameReadyContract(format!(
            "t mismatch: sent {expected_t:.6} got {received_t:.6}"
        )));
    }

    let runtime_seq = js_number_as_u64(obj.get("seq")).ok_or_else(|| {
        RecordError::FrameReadyContract(format!("missing seq at expected_t={expected_t:.6}"))
    })?;
    if let Some(min_seq_exclusive) = min_seq_exclusive {
        if runtime_seq <= min_seq_exclusive {
            return Err(RecordError::FrameReadyContract(format!(
                "stale seq: expected > {min_seq_exclusive} got {runtime_seq} at expected_t={expected_t:.6}"
            )));
        }
    }

    Ok(())
}

pub(crate) fn parse_json_result(
    value: serde_json::Value,
    context: &str,
) -> Result<serde_json::Value, RecordError> {
    if let Some(s) = value.as_str() {
        serde_json::from_str::<serde_json::Value>(s).map_err(|e| {
            RecordError::FrameReadyContract(format!(
                "{context} returned non-JSON string: {e} · raw={s}"
            ))
        })
    } else {
        Ok(value)
    }
}

pub(crate) fn video_state_is_ready(
    value: &serde_json::Value,
    expected_t: f64,
) -> Result<bool, RecordError> {
    let Some(obj) = value.as_object() else {
        return Err(RecordError::FrameReadyContract(format!(
            "video-state expected object at expected_t={expected_t:.6} · got: {value}"
        )));
    };
    let count = js_number_as_u64(obj.get("count")).unwrap_or(0);
    if count == 0 {
        return Ok(true);
    }
    let Some(clips) = obj.get("clips").and_then(serde_json::Value::as_array) else {
        return Err(RecordError::FrameReadyContract(format!(
            "video-state missing clips at expected_t={expected_t:.6} · payload={value}"
        )));
    };
    let target_ms = expected_t.round() as i64;
    for clip in clips {
        let Some(clip_obj) = clip.as_object() else {
            return Err(RecordError::FrameReadyContract(format!(
                "video-state clip not object at expected_t={expected_t:.6} · payload={clip}"
            )));
        };
        let frame_ready = clip_obj
            .get("frame_ready")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let ready_state = js_number_as_u64(clip_obj.get("ready_state")).unwrap_or(0);
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

pub fn js_number_as_u64(v: Option<&serde_json::Value>) -> Option<u64> {
    let v = v?;
    if let Some(u) = v.as_u64() {
        return Some(u);
    }
    if let Some(i) = v.as_i64() {
        if i >= 0 {
            return Some(i as u64);
        }
    }
    if let Some(f) = v.as_f64() {
        if f.is_finite() && f >= 0.0 && f.fract() == 0.0 && f <= u64::MAX as f64 {
            return Some(f as u64);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(payload: &str) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::from_str(payload)
    }

    #[test]
    fn seek_result_frame_ready_false_rejected() -> Result<(), serde_json::Error> {
        let payload = parsed(r#"{"t": 0, "frameReady": false, "seq": 0}"#)?;
        let result = verify_frame_ready(&payload, 0.0, None);
        assert!(matches!(result, Err(RecordError::FrameReadyContract(_))));
        Ok(())
    }

    #[test]
    fn seek_result_missing_seq_rejected() -> Result<(), serde_json::Error> {
        let payload = parsed(r#"{"t": 0, "frameReady": true}"#)?;
        let result = verify_frame_ready(&payload, 0.0, None);
        assert!(matches!(result, Err(RecordError::FrameReadyContract(_))));
        Ok(())
    }

    #[test]
    fn seek_result_t_out_of_tolerance_rejected() -> Result<(), serde_json::Error> {
        let payload = parsed(r#"{"t": 100, "frameReady": true, "seq": 1}"#)?;
        let result = verify_frame_ready(&payload, 0.0, None);
        assert!(matches!(result, Err(RecordError::FrameReadyContract(_))));
        Ok(())
    }

    #[test]
    fn parse_json_result_malformed_rejected() {
        let raw = serde_json::Value::String("{not json".to_string());
        let result = parse_json_result(raw, "seek result");
        assert!(matches!(result, Err(RecordError::FrameReadyContract(_))));
    }

    #[test]
    fn seek_result_stale_seq_rejected() -> Result<(), serde_json::Error> {
        let payload = parsed(r#"{"t": 0, "frameReady": true, "seq": 3}"#)?;
        let result = verify_frame_ready(&payload, 0.0, Some(5));
        assert!(matches!(result, Err(RecordError::FrameReadyContract(_))));
        Ok(())
    }

    #[test]
    fn video_state_malformed_timeout() -> Result<(), serde_json::Error> {
        let payload = parsed(r#"{"count": 1, "clips": {"bad": true}}"#)?;
        let result = video_state_is_ready(&payload, 0.0);
        assert!(matches!(result, Err(RecordError::FrameReadyContract(_))));
        Ok(())
    }
}
