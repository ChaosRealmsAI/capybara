use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use super::VerifyError;

pub(super) struct TopLevel {
    pub ftyp_brand: String,
    pub moov_offset: u64,
    pub mdat_offset: u64,
    pub mdat_size: u64,
    pub moov_bytes: Vec<u8>,
}

pub(super) fn scan_top_level(file: &mut File, file_size: u64) -> Result<TopLevel, VerifyError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|e| VerifyError::Io(format!("seek0: {e}")))?;

    let mut offset = 0u64;
    let mut ftyp_brand = String::new();
    let mut moov_offset = None;
    let mut mdat_offset = None;
    let mut mdat_size = 0u64;
    let mut moov_bytes = Vec::new();
    let mut header = [0u8; 16];

    while offset + 8 <= file_size {
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| VerifyError::Io(format!("seek {offset}: {e}")))?;
        let n = read_fully(file, &mut header[..8])?;
        if n < 8 {
            break;
        }

        let size32 = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
        let atom_type = [header[4], header[5], header[6], header[7]];
        let (atom_size, header_len) = atom_size(file, file_size, offset, size32, &mut header)?;
        validate_atom_size(offset, atom_size, header_len)?;

        match &atom_type {
            b"ftyp" => read_ftyp_brand(file, &mut ftyp_brand)?,
            b"moov" => {
                moov_offset = Some(offset);
                moov_bytes = read_atom_body(file, atom_size - header_len, "moov")?;
            }
            b"mdat" if mdat_offset.is_none() => {
                mdat_offset = Some(offset);
                mdat_size = atom_size.saturating_sub(header_len);
            }
            _ => {}
        }

        let next = offset.saturating_add(atom_size);
        if next <= offset {
            return Err(VerifyError::Malformed(format!(
                "non-advancing atom at {offset}"
            )));
        }
        offset = next;
    }

    let moov_offset = moov_offset.ok_or_else(|| VerifyError::Malformed("no moov atom".into()))?;
    let mdat_offset = mdat_offset.ok_or_else(|| VerifyError::Malformed("no mdat atom".into()))?;
    if moov_bytes.is_empty() {
        return Err(VerifyError::Malformed("empty moov body".into()));
    }

    Ok(TopLevel {
        ftyp_brand,
        moov_offset,
        mdat_offset,
        mdat_size,
        moov_bytes,
    })
}

fn atom_size(
    file: &mut File,
    file_size: u64,
    offset: u64,
    size32: u32,
    header: &mut [u8; 16],
) -> Result<(u64, u64), VerifyError> {
    if size32 == 1 {
        let n2 = read_fully(file, &mut header[8..16])?;
        if n2 < 8 {
            return Err(VerifyError::Malformed(format!(
                "truncated largesize at {offset}"
            )));
        }
        let mut sz64 = [0u8; 8];
        sz64.copy_from_slice(&header[8..16]);
        Ok((u64::from_be_bytes(sz64), 16))
    } else if size32 == 0 {
        Ok((file_size - offset, 8))
    } else {
        Ok((u64::from(size32), 8))
    }
}

fn validate_atom_size(offset: u64, atom_size: u64, header_len: u64) -> Result<(), VerifyError> {
    if atom_size < header_len {
        return Err(VerifyError::Malformed(format!(
            "atom size {atom_size} < header {header_len} at {offset}"
        )));
    }
    Ok(())
}

fn read_ftyp_brand(file: &mut File, ftyp_brand: &mut String) -> Result<(), VerifyError> {
    let mut brand = [0u8; 4];
    if read_fully(file, &mut brand)? == 4 {
        *ftyp_brand = ascii_code(&brand);
    }
    Ok(())
}

fn read_atom_body(file: &mut File, body_len: u64, label: &str) -> Result<Vec<u8>, VerifyError> {
    let Ok(body_len_usize) = usize::try_from(body_len) else {
        return Err(VerifyError::Malformed(format!(
            "{label} too large: {body_len}"
        )));
    };
    let mut buf = vec![0u8; body_len_usize];
    let n = read_fully(file, &mut buf)?;
    if n < body_len_usize {
        return Err(VerifyError::Malformed(format!(
            "short {label} read: {n}/{body_len_usize}"
        )));
    }
    Ok(buf)
}

fn read_fully(file: &mut File, buf: &mut [u8]) -> Result<usize, VerifyError> {
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) => return Err(VerifyError::Io(format!("read: {e}"))),
        }
    }
    Ok(filled)
}

pub(super) fn ascii_code(b: &[u8; 4]) -> String {
    let mut s = String::with_capacity(4);
    for c in b {
        if c.is_ascii_graphic() || *c == b' ' {
            s.push(*c as char);
        } else {
            s.push('?');
        }
    }
    s
}
