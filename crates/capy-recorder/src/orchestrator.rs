//! 时间切片并行录制 orchestrator。
//!
//! Historical: v1.15 time-sliced parallel recording orchestrator.
//!
//! 父进程职责（ADR-061）:
//! 1. probe bundle duration through the CEF OSR recorder path
//! 2. 按 --parallel N 平分 total_frames 为 N 段 · frame-index 半开区间
//! 3. spawn N 个子进程 capy-recorder · 各带 `--frame-range start,end` + 独立 `segment_i.mp4`
//! 4. wait 全部子进程完成 · 错误聚合
//! 5. ffmpeg concat demuxer `-c copy` 合并为最终 output · 零重编码
//!
//! 降级路径: `parallel <= 1` 或 `duration < 6s` 直接走单进程 CEF OSR record config。
//! 子进程路径: caller 已设 `cfg.frame_range = Some(...)` · 本函数不被调用。

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::events::{emit, Event};
use crate::record_loop::{RecordConfig, RecordError};

/// 并行模式最低启用阈值 (ms) · 低于此走单进程 (N 进程 boot ~1s × N 吃掉收益)。
pub const PARALLEL_MIN_DURATION_MS: u64 = 6000;

/// 4K 走 orchestrator 时的默认并发度。
/// Historical: v1.56 4K parallel default.
pub const PARALLEL_DEFAULT_4K: usize = 2;

/// 并行上限 · >4 会放大 CEF/GPU/VT 压力，直接拒绝。
/// Historical: v1.56 parallel cap.
pub const PARALLEL_MAX: usize = 4;

/// 4K 稳定上限 · x3/x4 会把 CEF/GPU surface 压力放大到不可承诺。
pub const PARALLEL_MAX_4K: usize = 2;

pub fn default_parallel_for_viewport(width: u32, height: u32) -> usize {
    match (width, height) {
        (3840, 2160) => PARALLEL_DEFAULT_4K,
        _ => 1,
    }
}

pub fn validate_requested_parallel(parallel: usize) -> Result<usize, RecordError> {
    if parallel == 0 {
        return Err(RecordError::PipelineError(
            "parallel must be >= 1".to_string(),
        ));
    }
    if parallel > PARALLEL_MAX {
        return Err(RecordError::PipelineError(format!(
            "parallel must be <= {PARALLEL_MAX} (got {parallel})"
        )));
    }
    Ok(parallel)
}

pub fn resolve_requested_parallel(
    requested: Option<usize>,
    width: u32,
    height: u32,
) -> Result<usize, RecordError> {
    let parallel = requested.unwrap_or_else(|| default_parallel_for_viewport(width, height));
    validate_requested_parallel(parallel)?;
    if (width, height) == (3840, 2160) && parallel > PARALLEL_MAX_4K {
        return Err(RecordError::PipelineError(format!(
            "4k export supports parallel 1 or {PARALLEL_MAX_4K} only (got x{parallel})"
        )));
    }
    Ok(parallel)
}

fn parallel_min_duration_ms() -> u64 {
    std::env::var("NF_PARALLEL_MIN_MS")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(PARALLEL_MIN_DURATION_MS)
}

pub(crate) fn compute_frame_ranges(total_frames: u64, parallel: usize) -> Vec<(u64, u64)> {
    if total_frames == 0 || parallel == 0 {
        return Vec::new();
    }

    let step = total_frames / parallel as u64;
    let mut ranges = Vec::with_capacity(parallel);
    let mut cursor = 0u64;
    for i in 0..parallel {
        let end = if i + 1 == parallel {
            total_frames
        } else {
            cursor + step
        };
        ranges.push((cursor, end));
        cursor = end;
    }
    debug_assert_eq!(cursor, total_frames);
    ranges
}

pub(crate) fn should_downgrade_to_serial(duration_sec: f64, parallel: usize) -> bool {
    should_downgrade_to_serial_with_min(duration_sec, parallel, PARALLEL_MIN_DURATION_MS)
}

fn should_downgrade_to_serial_with_min(
    duration_sec: f64,
    parallel: usize,
    min_duration_ms: u64,
) -> bool {
    parallel > 1 && duration_sec * 1000.0 < min_duration_ms as f64
}

/// 运行并行录制流水线 · 父进程入口。
pub async fn run_parallel(cfg: RecordConfig, parallel: usize) -> Result<(), RecordError> {
    let t0 = Instant::now();
    let requested_parallel = validate_requested_parallel(parallel)?;
    let min_duration_ms = parallel_min_duration_ms();

    // 1. probe duration · 启一次 shell · 快速读 + close。
    let duration_ms = probe_duration(&cfg).await?;
    let frame_dur_ms = 1000.0_f64 / f64::from(cfg.fps);
    let total_frames = ((duration_ms as f64) / frame_dur_ms).round() as u64;
    if total_frames == 0 {
        return Err(RecordError::NoFrames);
    }

    // 2. 降级判断 · 短视频 / parallel=1 走单进程。
    let duration_sec = duration_ms as f64 / 1000.0;
    let downgrade_for_duration = if min_duration_ms == PARALLEL_MIN_DURATION_MS {
        should_downgrade_to_serial(duration_sec, requested_parallel)
    } else {
        should_downgrade_to_serial_with_min(duration_sec, requested_parallel, min_duration_ms)
    };
    let effective_n = if requested_parallel <= 1 || downgrade_for_duration {
        1
    } else {
        requested_parallel
    };
    if effective_n == 1 {
        // 降级 · 直接跑 record_loop (cfg.frame_range 为 None · 等价全 range)。
        eprintln!(
            "[v1.15 orchestrator] parallel={requested_parallel} · duration_ms={duration_ms} · \
             degrading to single-process (duration<{min_duration_ms}ms or parallel<=1)"
        );
        return crate::export_api::run_record_config(cfg)
            .await
            .map(|_stats| ());
    }

    emit(Event::RecordParallelStart {
        parallel: effective_n,
        total_frames,
        duration_ms,
    });

    // 3. 平分 N 段 · frame-index 半开区间。
    let ranges = compute_frame_ranges(total_frames, effective_n);

    // 4. spawn N 子进程 · 输出到临时 segment_i.mp4。
    //
    // Historical: v1.44.1 · 找 capy-recorder binary:
    //   Historical: v1.15 假设 current_exe = capy-recorder · 直接 spawn 自己;
    //   Historical: v1.44+ 从 nf-shell lib 调用时 current_exe = nf-shell · 不能直接 spawn
    //   (nf-shell 的 main 走 event_loop · 不跑 record_loop).
    // 策略:优先找同目录的 capy-recorder · 兜底用 current_exe (单 binary 场景兼容).
    let self_exe = resolve_recorder_binary()?;
    let tmp_dir = cfg.output.parent().unwrap_or(Path::new("."));
    let stem = cfg
        .output
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "out".to_string());

    let segment_paths: Vec<PathBuf> = (0..effective_n)
        .map(|i| tmp_dir.join(format!("{stem}.seg{i:02}.mp4")))
        .collect();

    let segment_progress = Arc::new(Mutex::new(vec![0_u64; effective_n]));
    let mut children = Vec::with_capacity(effective_n);
    for (i, (start, end)) in ranges.iter().enumerate() {
        let seg_path = &segment_paths[i];
        let bitrate_str = format!("{}", cfg.bitrate_bps);
        let fps_str = format!("{}", cfg.fps);
        let max_dur_str = format!("{}", cfg.max_duration_s);
        let range_str = format!("{start},{end}");
        let res_str = match (cfg.width, cfg.height) {
            (1920, 1080) => "1080p".to_string(),
            (3840, 2160) => "4k".to_string(),
            _ => format!("{}x{}", cfg.width, cfg.height),
        };
        emit(Event::RecordSegmentStart {
            idx: i,
            start: *start,
            end: *end,
            output: seg_path.display().to_string(),
        });
        let mut child = Command::new(&self_exe)
            .arg(&cfg.bundle)
            .arg("-o")
            .arg(seg_path)
            .arg("--fps")
            .arg(&fps_str)
            .arg("--bitrate")
            .arg(&bitrate_str)
            .arg("--max-duration")
            .arg(&max_dur_str)
            .arg("--res")
            .arg(&res_str)
            .arg("--frame-range")
            .arg(&range_str)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| RecordError::PipelineError(format!("spawn segment {i}: {e}")))?;
        let stdout_handle = child.stdout.take().map(|stdout| {
            spawn_segment_progress_reader(
                i,
                *start,
                end.saturating_sub(*start),
                total_frames,
                Arc::clone(&segment_progress),
                stdout,
            )
        });
        children.push((i, *start, *end, child, stdout_handle));
    }

    // 5. wait · 聚合错误。
    for (i, start, end, mut child, stdout_handle) in children {
        let status = child
            .wait()
            .map_err(|e| RecordError::PipelineError(format!("wait segment {i}: {e}")))?;
        if let Some(handle) = stdout_handle {
            let _ = handle.join();
        }
        if !status.success() {
            // 收 stderr 供 debug。
            let mut stderr_bytes = Vec::new();
            if let Some(mut s) = child.stderr.take() {
                use std::io::Read;
                let _ = s.read_to_end(&mut stderr_bytes);
            }
            let msg = String::from_utf8_lossy(&stderr_bytes).into_owned();
            return Err(RecordError::PipelineError(format!(
                "segment {i} exited non-zero (code {:?}): {msg}",
                status.code()
            )));
        }
        emit_segment_progress(
            &segment_progress,
            i,
            end.saturating_sub(start),
            end.saturating_sub(start),
            total_frames,
        );
        emit(Event::RecordSegmentDone {
            idx: i,
            start,
            end,
            output: segment_paths[i].display().to_string(),
        });
    }

    // 6. ffmpeg concat demuxer · -c copy · 零重编码。
    emit(Event::RecordConcatStart {
        segments: segment_paths
            .iter()
            .map(|p| p.display().to_string())
            .collect(),
    });
    let list_path = tmp_dir.join(format!("{stem}.concat.txt"));
    let list_content = segment_paths
        .iter()
        .map(|p| {
            // ffmpeg concat 语法 · 绝对路径安全 · 单引号包裹防空格。
            let abs = p.canonicalize().unwrap_or_else(|_| p.clone());
            format!("file '{}'", abs.display())
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&list_path, list_content)
        .map_err(|e| RecordError::PipelineError(format!("write concat list: {e}")))?;

    let concat_status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-f")
        .arg("concat")
        .arg("-safe")
        .arg("0")
        .arg("-i")
        .arg(&list_path)
        .arg("-c")
        .arg("copy")
        .arg("-movflags")
        .arg("+faststart")
        .arg(&cfg.output)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| RecordError::PipelineError(format!("spawn ffmpeg concat: {e}")))?;
    if !concat_status.success() {
        return Err(RecordError::PipelineError(format!(
            "ffmpeg concat failed (code {:?})",
            concat_status.code()
        )));
    }

    // 7. 清理 segments · 不留临时文件 (保留 concat.txt 便于 debug)。
    for p in &segment_paths {
        let _ = std::fs::remove_file(p);
    }

    // 8. 总结事件。
    let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let size_bytes = std::fs::metadata(&cfg.output).map(|m| m.len()).unwrap_or(0);
    emit(Event::RecordDone {
        out: cfg.output.clone(),
        duration_ms,
        size_bytes,
        moov_front: true,
    });
    emit(Event::RecordParallelDone {
        parallel: effective_n,
        wall_time_ms: elapsed_ms,
    });

    Ok(())
}

fn spawn_segment_progress_reader(
    idx: usize,
    segment_start: u64,
    segment_frames: u64,
    total_frames: u64,
    progress: Arc<Mutex<Vec<u64>>>,
    stdout: std::process::ChildStdout,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            match value.get("event").and_then(serde_json::Value::as_str) {
                Some("record.frame") => {
                    let Some(seq) = value.get("seq").and_then(serde_json::Value::as_u64) else {
                        continue;
                    };
                    if seq < segment_start {
                        continue;
                    }
                    let local_frames = seq.saturating_sub(segment_start).saturating_add(1);
                    emit_segment_progress(
                        &progress,
                        idx,
                        local_frames.min(segment_frames),
                        segment_frames,
                        total_frames,
                    );
                }
                Some("record.encode_progress") => {
                    let frames = value
                        .get("frames_encoded")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0);
                    let local_frames = if frames > segment_start {
                        frames.saturating_sub(segment_start)
                    } else {
                        frames
                    };
                    emit_segment_progress(
                        &progress,
                        idx,
                        local_frames.min(segment_frames),
                        segment_frames,
                        total_frames,
                    );
                }
                Some("record.done") => {
                    emit_segment_progress(
                        &progress,
                        idx,
                        segment_frames,
                        segment_frames,
                        total_frames,
                    );
                }
                _ => {}
            }
        }
    })
}

fn emit_segment_progress(
    progress: &Arc<Mutex<Vec<u64>>>,
    idx: usize,
    frames: u64,
    segment_frames: u64,
    total_frames: u64,
) {
    let Ok(mut slots) = progress.lock() else {
        return;
    };
    let Some(slot) = slots.get_mut(idx) else {
        return;
    };
    *slot = (*slot).max(frames.min(segment_frames));
    let encoded = slots.iter().copied().sum::<u64>().min(total_frames);
    emit(Event::RecordEncodeProgress {
        frames_encoded: encoded,
        total_frames,
        percent: (encoded as f64 / total_frames as f64 * 100.0).clamp(0.0, 100.0),
    });
}

/// probe bundle 自报 duration (ms) · 启一次 shell · call __nf.getDuration · drop。
///
/// 失败退回 cfg.max_duration_s × 1000 (用户预设 cap)。
async fn probe_duration(cfg: &RecordConfig) -> Result<u64, RecordError> {
    crate::cef_osr::probe_duration(cfg).await
}

/// 解析 capy-recorder binary 路径 · 供 orchestrator spawn 子进程用。
///
/// Historical: v1.44.1 recorder binary resolution.
///
/// 探测顺序:
/// 1. `$NF_RECORDER_BIN` 环境变量 (开发时 override)
/// 2. `current_exe().parent()/capy-recorder` (cargo 默认布局: target/release/{nf-shell,capy-recorder})
/// 3. `current_exe()` 自身 (capy-recorder 单 binary 场景).
///
/// Historical: v1.15 self-spawn compatibility path.
fn resolve_recorder_binary() -> Result<PathBuf, RecordError> {
    if let Ok(env_path) = std::env::var("NF_RECORDER_BIN") {
        let p = PathBuf::from(env_path);
        if p.exists() {
            return Ok(p);
        }
    }
    let current = std::env::current_exe()
        .map_err(|e| RecordError::PipelineError(format!("current_exe: {e}")))?;
    if let Some(parent) = current.parent() {
        let candidate = parent.join("capy-recorder");
        if candidate.exists() {
            return Ok(candidate);
        }
        let candidate_exe = parent.join("capy-recorder.exe"); // windows safety
        if candidate_exe.exists() {
            return Ok(candidate_exe);
        }
    }
    // v1.15 兼容 · 若当前就是 capy-recorder 自己 · 直接用。
    if current.file_stem().and_then(|s| s.to_str()) == Some("capy-recorder") {
        return Ok(current);
    }
    Err(RecordError::PipelineError(format!(
        "capy-recorder binary not found next to {} · set NF_RECORDER_BIN env var",
        current.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::{compute_frame_ranges, should_downgrade_to_serial};

    #[test]
    fn ranges_split_evenly() {
        assert_eq!(
            compute_frame_ranges(120, 4),
            vec![(0, 30), (30, 60), (60, 90), (90, 120)]
        );
    }

    #[test]
    fn ranges_remainder_to_last() {
        assert_eq!(
            compute_frame_ranges(122, 4),
            vec![(0, 30), (30, 60), (60, 90), (90, 122)]
        );
    }

    #[test]
    fn short_video_downgrades() {
        assert!(should_downgrade_to_serial(5.0, 4));
    }

    #[test]
    fn parallel_one_no_downgrade() {
        assert!(!should_downgrade_to_serial(10.0, 1));
    }
}
