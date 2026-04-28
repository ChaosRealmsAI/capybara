use std::fs;
use std::path::{Path, PathBuf};

use crate::asset::scan::AssetReference;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopiedAsset {
    pub original_path: String,
    pub relative_path: String,
    pub byte_size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferencedAsset {
    pub original_path: String,
    pub byte_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingAsset {
    pub original_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyOutcome {
    Copied(CopiedAsset),
    Referenced(ReferencedAsset),
    Missing(MissingAsset),
}

pub fn copy_asset(
    reference: &AssetReference,
    base_dir: &Path,
    project_root: &Path,
    max_copy_bytes: u64,
) -> CopyOutcome {
    let source = match AssetSource::from_reference(reference, base_dir) {
        Some(source) => source,
        None => {
            return CopyOutcome::Missing(MissingAsset {
                original_path: resolve_path(&reference.src, base_dir),
            });
        }
    };
    if source.byte_size > max_copy_bytes {
        return CopyOutcome::Referenced(ReferencedAsset {
            original_path: source.original_path,
            byte_size: source.byte_size,
        });
    }

    let relative_path = format!(
        "assets/{}",
        sanitized_filename(
            &reference.asset_id,
            source.extension.as_deref(),
            &reference.asset_type
        )
    );
    let destination = project_root.join(&relative_path);
    if let Some(parent) = destination.parent() {
        if fs::create_dir_all(parent).is_err() {
            return CopyOutcome::Missing(MissingAsset {
                original_path: PathBuf::from(source.original_path),
            });
        }
    }
    if fs::write(&destination, &source.bytes).is_err() {
        return CopyOutcome::Missing(MissingAsset {
            original_path: PathBuf::from(source.original_path),
        });
    }

    CopyOutcome::Copied(CopiedAsset {
        original_path: source.original_path,
        relative_path,
        byte_size: source.byte_size,
        sha256: format!("sha256-{}", sha256_hex(&source.bytes)),
    })
}

struct AssetSource {
    original_path: String,
    bytes: Vec<u8>,
    byte_size: u64,
    extension: Option<String>,
}

impl AssetSource {
    fn from_reference(reference: &AssetReference, base_dir: &Path) -> Option<Self> {
        if reference.src.starts_with("data:") {
            return inline_source(&reference.src);
        }
        let path = resolve_path(&reference.src, base_dir);
        if !path.is_file() {
            return None;
        }
        let metadata = fs::metadata(&path).ok()?;
        let bytes = fs::read(&path).ok()?;
        Some(Self {
            original_path: absolute_display(&path),
            bytes,
            byte_size: metadata.len(),
            extension: extension_from_path(&path),
        })
    }
}

fn resolve_path(src: &str, base_dir: &Path) -> PathBuf {
    if let Some(fixture) = src.strip_prefix("fixture://") {
        return PathBuf::from("fixtures").join(fixture.trim_start_matches('/'));
    }
    let path = PathBuf::from(src);
    if path.is_absolute() {
        path
    } else {
        base_dir.join(path)
    }
}

fn inline_source(src: &str) -> Option<AssetSource> {
    let (meta, data) = src.strip_prefix("data:")?.split_once(',')?;
    let bytes = if meta.ends_with(";base64") {
        return None;
    } else {
        percent_decode(data)?
    };
    let byte_size = u64::try_from(bytes.len()).ok()?;
    Some(AssetSource {
        original_path: src.to_string(),
        bytes,
        byte_size,
        extension: extension_from_mime(meta),
    })
}

fn percent_decode(input: &str) -> Option<Vec<u8>> {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = bytes.get(index + 1).and_then(|byte| hex_value(*byte))?;
            let low = bytes.get(index + 2).and_then(|byte| hex_value(*byte))?;
            output.push((high << 4) | low);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    Some(output)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn sanitized_filename(asset_id: &str, extension: Option<&str>, asset_type: &str) -> String {
    let mut name = String::new();
    for ch in asset_id.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            name.push(ch);
        } else if ch == '.' {
            name.push('-');
        }
    }
    if name.is_empty() {
        name.push_str("asset");
    }
    let ext = match extension.filter(|value| !value.trim().is_empty()) {
        Some(value) => value.to_string(),
        None => default_extension(asset_type),
    };
    format!("{name}.{ext}")
}

fn default_extension(asset_type: &str) -> String {
    match asset_type {
        "font" => "font".to_string(),
        "svg" => "svg".to_string(),
        _ => "bin".to_string(),
    }
}

fn extension_from_path(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.trim_start_matches('.').to_ascii_lowercase())
        .filter(|extension| !extension.is_empty())
}

fn extension_from_mime(meta: &str) -> Option<String> {
    let mime = match meta.split(';').next() {
        Some(value) => value,
        None => meta,
    };
    match mime {
        "image/svg+xml" => Some("svg".to_string()),
        "image/png" => Some("png".to_string()),
        "image/jpeg" => Some("jpg".to_string()),
        "image/webp" => Some("webp".to_string()),
        "font/woff" => Some("woff".to_string()),
        "font/woff2" => Some("woff2".to_string()),
        "font/ttf" => Some("ttf".to_string()),
        "font/otf" => Some("otf".to_string()),
        _ => None,
    }
}

fn absolute_display(path: &Path) -> String {
    match path.canonicalize() {
        Ok(path) => path,
        Err(_) => path.to_path_buf(),
    }
    .display()
    .to_string()
}

fn sha256_hex(input: &[u8]) -> String {
    let mut state = [
        0x6a09e667_u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = input.to_vec();
    let bit_len = match u64::try_from(message.len()) {
        Ok(len) => len.saturating_mul(8),
        Err(_) => u64::MAX,
    };
    message.push(0x80);
    while (message.len() % 64) != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in message.chunks(64) {
        let mut words = [0_u32; 64];
        let mut index = 0;
        while index < 16 {
            let offset = index * 4;
            words[index] = u32::from_be_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
            index += 1;
        }
        index = 16;
        while index < 64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
            index += 1;
        }

        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g = state[6];
        let mut h = state[7];

        index = 0;
        while index < 64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
            index += 1;
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }

    state
        .iter()
        .map(|word| format!("{word:08x}"))
        .collect::<String>()
}

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

#[cfg(test)]
mod tests {
    use super::sha256_hex;

    #[test]
    fn asset_sha256_matches_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
