use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};

use crate::packager::{Result, ScrollMediaError};

#[derive(Debug, Clone)]
pub struct ServeOptions {
    pub root: PathBuf,
    pub host: String,
    pub port: u16,
}

pub fn serve_static(options: ServeOptions) -> Result<()> {
    let root = options
        .root
        .canonicalize()
        .map_err(|err| ScrollMediaError::Message(format!("serve root not found: {err}")))?;
    let listener = TcpListener::bind((options.host.as_str(), options.port)).map_err(|err| {
        ScrollMediaError::Message(format!(
            "bind {}:{} failed: {err}",
            options.host, options.port
        ))
    })?;
    println!(
        "capy media serve http://{}:{}/ -> {}",
        options.host,
        options.port,
        root.display()
    );
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(err) = handle_stream(stream, &root) {
                    eprintln!("{err}");
                }
            }
            Err(err) => eprintln!("accept failed: {err}"),
        }
    }
    Ok(())
}

fn handle_stream(mut stream: TcpStream, root: &Path) -> Result<()> {
    let mut buffer = [0_u8; 8192];
    let read = stream
        .read(&mut buffer)
        .map_err(|err| ScrollMediaError::Message(format!("read request failed: {err}")))?;
    if read == 0 {
        return Ok(());
    }
    let request = String::from_utf8_lossy(&buffer[..read]);
    let mut lines = request.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| ScrollMediaError::Message("empty HTTP request".to_string()))?;
    let parts = request_line.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 2 {
        return write_status(&mut stream, 400, "Bad Request", "Bad request");
    }
    let method = parts[0];
    if method != "GET" && method != "HEAD" {
        return write_status(&mut stream, 405, "Method Not Allowed", "Method not allowed");
    }
    let range = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(key, _)| key.eq_ignore_ascii_case("range"))
        .map(|(_, value)| value.trim().to_string());
    let path = match request_path(root, parts[1]) {
        Some(path) => path,
        None => return write_status(&mut stream, 403, "Forbidden", "Forbidden"),
    };
    if !path.is_file() {
        return write_status(&mut stream, 404, "Not Found", "Not found");
    }
    write_file_response(&mut stream, &path, method == "HEAD", range.as_deref())
}

fn request_path(root: &Path, raw: &str) -> Option<PathBuf> {
    let path_part = raw.split('?').next().unwrap_or(raw);
    let decoded = percent_decode(path_part)?;
    let relative = decoded.strip_prefix('/').unwrap_or(&decoded);
    let relative = if relative.is_empty() {
        "index.html"
    } else {
        relative
    };
    let mut clean = PathBuf::new();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(root.join(clean))
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return None;
            }
            let high = hex_value(bytes[index + 1])?;
            let low = hex_value(bytes[index + 2])?;
            out.push((high << 4) | low);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn write_file_response(
    stream: &mut TcpStream,
    path: &Path,
    head_only: bool,
    range: Option<&str>,
) -> Result<()> {
    let metadata = fs::metadata(path)
        .map_err(|err| ScrollMediaError::Message(format!("read metadata failed: {err}")))?;
    let total = metadata.len();
    let mime = mime_for_path(path);
    if let Some(range) = range {
        let Some((start, end)) = parse_range(range, total) else {
            return write_headers(
                stream,
                416,
                "Range Not Satisfiable",
                &[
                    ("Accept-Ranges", "bytes".to_string()),
                    ("Content-Range", format!("bytes */{total}")),
                    ("Content-Length", "0".to_string()),
                ],
            );
        };
        write_headers(
            stream,
            206,
            "Partial Content",
            &[
                ("Accept-Ranges", "bytes".to_string()),
                (
                    "Cache-Control",
                    "public, max-age=31536000, immutable".to_string(),
                ),
                ("Content-Type", mime.to_string()),
                ("Content-Length", (end - start + 1).to_string()),
                ("Content-Range", format!("bytes {start}-{end}/{total}")),
            ],
        )?;
        if !head_only {
            stream_file_range(stream, path, start, end)?;
        }
        return Ok(());
    }
    write_headers(
        stream,
        200,
        "OK",
        &[
            ("Accept-Ranges", "bytes".to_string()),
            (
                "Cache-Control",
                "public, max-age=31536000, immutable".to_string(),
            ),
            ("Content-Type", mime.to_string()),
            ("Content-Length", total.to_string()),
        ],
    )?;
    if !head_only {
        let mut file = File::open(path)
            .map_err(|err| ScrollMediaError::Message(format!("open file failed: {err}")))?;
        std::io::copy(&mut file, stream)
            .map_err(|err| ScrollMediaError::Message(format!("stream file failed: {err}")))?;
    }
    Ok(())
}

fn parse_range(value: &str, total: u64) -> Option<(u64, u64)> {
    let raw = value.strip_prefix("bytes=")?;
    let (start_raw, end_raw) = raw.split_once('-')?;
    let start = if start_raw.is_empty() {
        0
    } else {
        start_raw.parse::<u64>().ok()?
    };
    let mut end = if end_raw.is_empty() {
        total.checked_sub(1)?
    } else {
        end_raw.parse::<u64>().ok()?
    };
    if end >= total {
        end = total.checked_sub(1)?;
    }
    if start > end || start >= total {
        return None;
    }
    Some((start, end))
}

fn stream_file_range(stream: &mut TcpStream, path: &Path, start: u64, end: u64) -> Result<()> {
    let mut file = File::open(path)
        .map_err(|err| ScrollMediaError::Message(format!("open file failed: {err}")))?;
    file.seek(SeekFrom::Start(start))
        .map_err(|err| ScrollMediaError::Message(format!("seek file failed: {err}")))?;
    let mut remaining = end - start + 1;
    let mut buffer = [0_u8; 32 * 1024];
    while remaining > 0 {
        let cap = usize::try_from(remaining.min(buffer.len() as u64))
            .map_err(|err| ScrollMediaError::Message(format!("range too large: {err}")))?;
        let read = file
            .read(&mut buffer[..cap])
            .map_err(|err| ScrollMediaError::Message(format!("read file range failed: {err}")))?;
        if read == 0 {
            break;
        }
        stream
            .write_all(&buffer[..read])
            .map_err(|err| ScrollMediaError::Message(format!("write file range failed: {err}")))?;
        remaining -= read as u64;
    }
    Ok(())
}

fn write_status(stream: &mut TcpStream, code: u16, label: &str, body: &str) -> Result<()> {
    write_headers(
        stream,
        code,
        label,
        &[
            ("Content-Type", "text/plain; charset=utf-8".to_string()),
            ("Content-Length", body.len().to_string()),
        ],
    )?;
    stream
        .write_all(body.as_bytes())
        .map_err(|err| ScrollMediaError::Message(format!("write response failed: {err}")))
}

fn write_headers(
    stream: &mut TcpStream,
    code: u16,
    label: &str,
    headers: &[(&str, String)],
) -> Result<()> {
    write!(stream, "HTTP/1.1 {code} {label}\r\n")
        .map_err(|err| ScrollMediaError::Message(format!("write status failed: {err}")))?;
    for (key, value) in headers {
        write!(stream, "{key}: {value}\r\n")
            .map_err(|err| ScrollMediaError::Message(format!("write header failed: {err}")))?;
    }
    write!(stream, "Connection: close\r\n\r\n")
        .map_err(|err| ScrollMediaError::Message(format!("write header end failed: {err}")))
}

fn mime_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|value| value.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("mp4") => "video/mp4",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_http_range() {
        assert_eq!(parse_range("bytes=10-19", 100), Some((10, 19)));
        assert_eq!(parse_range("bytes=10-", 100), Some((10, 99)));
        assert_eq!(parse_range("bytes=100-101", 100), None);
    }

    #[test]
    fn rejects_parent_paths() {
        let root = PathBuf::from("/tmp/root");
        assert!(request_path(&root, "/../secret").is_none());
    }
}
