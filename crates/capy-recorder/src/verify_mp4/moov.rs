use super::atoms::ascii_code;
use super::{h264, VerifyError};

#[derive(Debug, Default)]
pub(super) struct MoovInfo {
    pub movie_timescale: u32,
    pub duration_units: u64,
    pub media_timescale: u32,
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub color_primaries: String,
    pub transfer: String,
    pub matrix: String,
    pub has_b_frames: bool,
    pub avg_sample_duration: u64,
    pub sample_count: u64,
}

pub(super) fn parse_moov(body: &[u8]) -> Result<MoovInfo, VerifyError> {
    let mut info = MoovInfo::default();

    for (atype, sub) in &iter_children(body)? {
        match atype {
            b"mvhd" => {
                let (ts, dur) = parse_mvhd(sub)?;
                info.movie_timescale = ts;
                info.duration_units = dur;
            }
            b"trak" if info.codec.is_empty() => parse_trak(sub, &mut info)?,
            _ => {}
        }
    }

    if info.codec.is_empty() {
        return Err(VerifyError::Malformed("no video trak found".into()));
    }
    Ok(info)
}

type Mp4Child<'a> = ([u8; 4], &'a [u8]);

fn iter_children(body: &[u8]) -> Result<Vec<Mp4Child<'_>>, VerifyError> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 8 <= body.len() {
        let size32 = u32::from_be_bytes([body[i], body[i + 1], body[i + 2], body[i + 3]]);
        let at = [body[i + 4], body[i + 5], body[i + 6], body[i + 7]];
        let (size, hlen) = child_size(body, i, size32)?;
        if size < hlen || i + size > body.len() {
            return Err(VerifyError::Malformed(format!(
                "child atom size {size} exceeds parent at {i}"
            )));
        }
        out.push((at, &body[i + hlen..i + size]));
        i += size;
    }
    Ok(out)
}

fn child_size(body: &[u8], i: usize, size32: u32) -> Result<(usize, usize), VerifyError> {
    if size32 == 1 {
        if i + 16 > body.len() {
            return Err(VerifyError::Malformed("child largesize truncated".into()));
        }
        let mut s = [0u8; 8];
        s.copy_from_slice(&body[i + 8..i + 16]);
        Ok((u64::from_be_bytes(s) as usize, 16))
    } else if size32 == 0 {
        Ok((body.len() - i, 8))
    } else {
        Ok((size32 as usize, 8))
    }
}

fn parse_mvhd(body: &[u8]) -> Result<(u32, u64), VerifyError> {
    if body.len() < 4 {
        return Err(VerifyError::Malformed("mvhd too short".into()));
    }
    let version = body[0];
    if version == 1 {
        if body.len() < 32 {
            return Err(VerifyError::Malformed("mvhd v1 too short".into()));
        }
        let ts = u32::from_be_bytes([body[20], body[21], body[22], body[23]]);
        let mut dur = [0u8; 8];
        dur.copy_from_slice(&body[24..32]);
        Ok((ts, u64::from_be_bytes(dur)))
    } else {
        if body.len() < 20 {
            return Err(VerifyError::Malformed("mvhd v0 too short".into()));
        }
        let ts = u32::from_be_bytes([body[12], body[13], body[14], body[15]]);
        let dur = u32::from_be_bytes([body[16], body[17], body[18], body[19]]);
        Ok((ts, u64::from(dur)))
    }
}

fn parse_trak(body: &[u8], info: &mut MoovInfo) -> Result<(), VerifyError> {
    for (atype, sub) in &iter_children(body)? {
        if atype == b"mdia" {
            parse_mdia(sub, info)?;
        }
    }
    Ok(())
}

fn parse_mdia(body: &[u8], info: &mut MoovInfo) -> Result<(), VerifyError> {
    let children = iter_children(body)?;
    let mut is_video = false;
    for (atype, sub) in &children {
        match atype {
            b"mdhd" => info.media_timescale = parse_mdhd_timescale(sub)?,
            b"hdlr" => is_video = hdlr_is_video(sub),
            b"minf" if is_video => parse_minf(sub, info)?,
            _ => {}
        }
    }

    if !is_video {
        let handler_after_minf = children
            .iter()
            .any(|(atype, sub)| atype == b"hdlr" && hdlr_is_video(sub));
        if handler_after_minf {
            for (atype, sub) in &children {
                if atype == b"minf" {
                    parse_minf(sub, info)?;
                }
            }
        } else {
            info.media_timescale = 0;
        }
    }
    Ok(())
}

fn parse_mdhd_timescale(body: &[u8]) -> Result<u32, VerifyError> {
    if body.len() < 4 {
        return Err(VerifyError::Malformed("mdhd too short".into()));
    }
    let version = body[0];
    let ts_off = if version == 1 { 20 } else { 12 };
    if body.len() < ts_off + 4 {
        return Err(VerifyError::Malformed("mdhd truncated".into()));
    }
    Ok(u32::from_be_bytes([
        body[ts_off],
        body[ts_off + 1],
        body[ts_off + 2],
        body[ts_off + 3],
    ]))
}

fn hdlr_is_video(body: &[u8]) -> bool {
    body.len() >= 12 && &body[8..12] == b"vide"
}

fn parse_minf(body: &[u8], info: &mut MoovInfo) -> Result<(), VerifyError> {
    for (atype, sub) in &iter_children(body)? {
        if atype == b"stbl" {
            parse_stbl(sub, info)?;
        }
    }
    Ok(())
}

fn parse_stbl(body: &[u8], info: &mut MoovInfo) -> Result<(), VerifyError> {
    for (atype, sub) in &iter_children(body)? {
        match atype {
            b"stsd" => parse_stsd(sub, info)?,
            b"stts" => parse_stts(sub, info)?,
            b"ctts" => parse_ctts(sub, info)?,
            _ => {}
        }
    }
    Ok(())
}

fn parse_stsd(body: &[u8], info: &mut MoovInfo) -> Result<(), VerifyError> {
    if body.len() < 8 {
        return Err(VerifyError::Malformed("stsd too short".into()));
    }
    let i = 8usize;
    if i + 8 > body.len() {
        return Ok(());
    }
    let size = u32::from_be_bytes([body[i], body[i + 1], body[i + 2], body[i + 3]]) as usize;
    let at = [body[i + 4], body[i + 5], body[i + 6], body[i + 7]];
    if size < 8 || i + size > body.len() {
        return Err(VerifyError::Malformed("stsd entry size".into()));
    }
    let entry_body = &body[i + 8..i + size];
    info.codec = ascii_code(&at);
    if entry_body.len() >= 32 {
        info.width = u32::from(u16::from_be_bytes([entry_body[24], entry_body[25]]));
        info.height = u32::from(u16::from_be_bytes([entry_body[26], entry_body[27]]));
    }
    parse_visual_sample_entry_boxes(entry_body, info);
    Ok(())
}

fn parse_visual_sample_entry_boxes(entry_body: &[u8], info: &mut MoovInfo) {
    const VIS_HDR: usize = 78;
    if entry_body.len() <= VIS_HDR {
        return;
    }
    if let Ok(children) = iter_children(&entry_body[VIS_HDR..]) {
        for (ctype, cbody) in &children {
            match ctype {
                b"avcC" => parse_avc_c(cbody, info),
                b"colr" => parse_colr(cbody, info),
                _ => {}
            }
        }
    }
}

fn parse_colr(body: &[u8], info: &mut MoovInfo) {
    if body.len() < 4 {
        return;
    }
    let kind = [body[0], body[1], body[2], body[3]];
    if (&kind == b"nclx" || &kind == b"nclc") && body.len() >= 10 {
        let prim = u16::from_be_bytes([body[4], body[5]]);
        let tf = u16::from_be_bytes([body[6], body[7]]);
        let mat = u16::from_be_bytes([body[8], body[9]]);
        if info.color_primaries.is_empty() {
            info.color_primaries = describe_primaries(prim);
        }
        if info.transfer.is_empty() {
            info.transfer = describe_transfer(tf);
        }
        if info.matrix.is_empty() {
            info.matrix = describe_matrix(mat);
        }
    }
}

pub(super) fn describe_primaries(code: u16) -> String {
    match code {
        1 => "bt709".into(),
        9 => "bt2020".into(),
        other => format!("code{other}"),
    }
}

pub(super) fn describe_transfer(code: u16) -> String {
    match code {
        1 => "bt709".into(),
        16 => "smpte2084".into(),
        18 => "hlg".into(),
        other => format!("code{other}"),
    }
}

pub(super) fn describe_matrix(code: u16) -> String {
    match code {
        1 => "bt709".into(),
        9 => "bt2020ncl".into(),
        other => format!("code{other}"),
    }
}

fn parse_avc_c(body: &[u8], info: &mut MoovInfo) {
    if body.len() < 6 {
        return;
    }
    let num_sps = body[5] & 0x1F;
    if num_sps == 0 {
        return;
    }
    let mut i = 6usize;
    if i + 2 > body.len() {
        return;
    }
    let sps_len = u16::from_be_bytes([body[i], body[i + 1]]) as usize;
    i += 2;
    if i + sps_len > body.len() || sps_len == 0 || sps_len < 2 {
        return;
    }
    let rbsp = h264::strip_emulation_prevention(&body[i + 1..i + sps_len]);
    h264::parse_sps_vui(&rbsp, info);
}

fn parse_stts(body: &[u8], info: &mut MoovInfo) -> Result<(), VerifyError> {
    if body.len() < 8 {
        return Err(VerifyError::Malformed("stts too short".into()));
    }
    let count = u32::from_be_bytes([body[4], body[5], body[6], body[7]]) as usize;
    let mut total_samples = 0u64;
    let mut total_duration = 0u64;
    let mut i = 8usize;
    for _ in 0..count {
        if i + 8 > body.len() {
            return Err(VerifyError::Malformed("stts entry truncated".into()));
        }
        let n = u32::from_be_bytes([body[i], body[i + 1], body[i + 2], body[i + 3]]) as u64;
        let d = u32::from_be_bytes([body[i + 4], body[i + 5], body[i + 6], body[i + 7]]) as u64;
        total_samples = total_samples.saturating_add(n);
        total_duration = total_duration.saturating_add(n.saturating_mul(d));
        i += 8;
    }
    info.sample_count = total_samples;
    info.avg_sample_duration = if total_samples == 0 {
        0
    } else {
        total_duration / total_samples
    };
    Ok(())
}

fn parse_ctts(body: &[u8], info: &mut MoovInfo) -> Result<(), VerifyError> {
    if body.len() < 8 {
        return Ok(());
    }
    let count = u32::from_be_bytes([body[4], body[5], body[6], body[7]]) as usize;
    let mut i = 8usize;
    for _ in 0..count {
        if i + 8 > body.len() {
            break;
        }
        let off = u32::from_be_bytes([body[i + 4], body[i + 5], body[i + 6], body[i + 7]]);
        if off != 0 {
            info.has_b_frames = true;
            return Ok(());
        }
        i += 8;
    }
    Ok(())
}
