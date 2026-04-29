//! Pure-Rust MP4 atom verifier · product-built self-verification.
//!
//! Recording quality checks must be internal to the product so a fresh AI
//! session can verify output without external `mediainfo` or `ffprobe`.

use serde::Serialize;
use std::fs::File;
use std::path::Path;

mod atoms;
mod h264;
mod moov;

use atoms::scan_top_level;
use moov::parse_moov;

/// fps tolerance window (± 0.1%).
const FPS_TOLERANCE: f64 = 0.001;

/// Bitrate tolerance window (± 15%) · encoder sees target as a soft ceiling.
const BITRATE_TOLERANCE: f64 = 0.15;

/// Full verdict of one MP4 file · serializable for JSON output.
#[derive(Debug, Clone, Serialize)]
pub struct Mp4Verdict {
    pub file: String,
    pub file_size: u64,
    pub moov_front: bool,
    pub moov_offset: u64,
    pub mdat_offset: u64,
    pub ftyp_brand: String,
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate: f64,
    pub bit_rate: u64,
    pub color_primaries: String,
    pub transfer: String,
    pub ycbcr_matrix: String,
    pub has_b_frames: bool,
    pub time_scale: u32,
    pub duration_ms: u64,
    pub sample_count: u64,
}

/// One MP4 assertion result.
#[derive(Debug, Clone, Serialize)]
pub struct Assertion {
    pub name: String,
    pub expected: String,
    pub actual: String,
    pub pass: bool,
}

/// Errors from `verify`.
#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("io: {0}")]
    Io(String),
    #[error("malformed mp4: {0}")]
    Malformed(String),
    #[error("unsupported codec: {0}")]
    UnsupportedCodec(String),
}

/// Verify an MP4 file · returns verdict + list of product assertions.
pub fn verify(
    path: &Path,
    expect_fps: u32,
    expect_bitrate: Option<u32>,
) -> Result<(Mp4Verdict, Vec<Assertion>), VerifyError> {
    let mut file = File::open(path).map_err(|e| VerifyError::Io(format!("{e}")))?;
    let file_size = file
        .metadata()
        .map_err(|e| VerifyError::Io(format!("metadata: {e}")))?
        .len();

    let top = scan_top_level(&mut file, file_size)?;
    let info = parse_moov(&top.moov_bytes)?;
    let duration_ms = duration_ms(info.duration_units, info.movie_timescale);
    let frame_rate = frame_rate(info.media_timescale, info.avg_sample_duration);
    let bit_rate = bit_rate(top.mdat_size, duration_ms);

    let verdict = Mp4Verdict {
        file: path.display().to_string(),
        file_size,
        moov_front: top.moov_offset < top.mdat_offset,
        moov_offset: top.moov_offset,
        mdat_offset: top.mdat_offset,
        ftyp_brand: top.ftyp_brand,
        codec: info.codec.clone(),
        width: info.width,
        height: info.height,
        frame_rate,
        bit_rate,
        color_primaries: info.color_primaries.clone(),
        transfer: info.transfer.clone(),
        ycbcr_matrix: info.matrix.clone(),
        has_b_frames: info.has_b_frames,
        time_scale: info.media_timescale,
        duration_ms,
        sample_count: info.sample_count,
    };

    Ok((verdict.clone(), assertions(verdict, expect_fps, expect_bitrate)))
}

fn duration_ms(duration_units: u64, movie_timescale: u32) -> u64 {
    if movie_timescale == 0 {
        0
    } else {
        (duration_units * 1000) / u64::from(movie_timescale)
    }
}

fn frame_rate(media_timescale: u32, avg_sample_duration: u64) -> f64 {
    if avg_sample_duration == 0 || media_timescale == 0 {
        0.0
    } else {
        f64::from(media_timescale) / (avg_sample_duration as f64)
    }
}

fn bit_rate(mdat_size: u64, duration_ms: u64) -> u64 {
    if duration_ms == 0 {
        0
    } else {
        (mdat_size.saturating_mul(8).saturating_mul(1000)) / duration_ms
    }
}

fn assertions(
    verdict: Mp4Verdict,
    expect_fps: u32,
    expect_bitrate: Option<u32>,
) -> Vec<Assertion> {
    let mut asserts = Vec::with_capacity(6);
    asserts.push(Assertion {
        name: "moov_front".into(),
        expected: "moov_offset < mdat_offset".into(),
        actual: format!("moov@{} mdat@{}", verdict.moov_offset, verdict.mdat_offset),
        pass: verdict.moov_front,
    });
    asserts.push(contains_assertion(
        "color_primaries_bt709",
        "contains \"709\"",
        &verdict.color_primaries,
    ));
    asserts.push(contains_assertion(
        "transfer_bt709",
        "contains \"709\"",
        &verdict.transfer,
    ));
    asserts.push(fps_assertion(verdict.frame_rate, expect_fps));
    asserts.push(bitrate_assertion(verdict.bit_rate, expect_bitrate));
    asserts.push(Assertion {
        name: "codec_avc1".into(),
        expected: "avc1 (H.264)".into(),
        actual: verdict.codec.clone(),
        pass: verdict.codec == "avc1" || verdict.codec.to_uppercase().contains("H.264"),
    });
    asserts
}

fn contains_assertion(name: &str, expected: &str, actual: &str) -> Assertion {
    Assertion {
        name: name.into(),
        expected: expected.into(),
        actual: actual.into(),
        pass: actual.contains("709"),
    }
}

fn fps_assertion(actual: f64, expect_fps: u32) -> Assertion {
    let fps_expected = f64::from(expect_fps);
    let fps_delta = (actual - fps_expected).abs();
    let fps_tol = fps_expected * FPS_TOLERANCE;
    Assertion {
        name: "frame_rate".into(),
        expected: format!("{fps_expected:.3} ± {:.3} (0.1%)", fps_tol),
        actual: format!("{actual:.3}"),
        pass: actual > 0.0 && fps_delta <= fps_tol,
    }
}

fn bitrate_assertion(actual: u64, expect_bitrate: Option<u32>) -> Assertion {
    match expect_bitrate {
        Some(exp) => {
            let exp_f = f64::from(exp);
            let lo = exp_f * (1.0 - BITRATE_TOLERANCE);
            let hi = exp_f * (1.0 + BITRATE_TOLERANCE);
            let actual_f = actual as f64;
            Assertion {
                name: "bit_rate".into(),
                expected: format!("{exp} bps ± 15% (range {:.0}..{:.0})", lo, hi),
                actual: format!("{actual} bps"),
                pass: actual_f >= lo && actual_f <= hi,
            }
        }
        None => Assertion {
            name: "bit_rate".into(),
            expected: "skipped (no --expect-bitrate)".into(),
            actual: format!("{actual} bps"),
            pass: true,
        },
    }
}
