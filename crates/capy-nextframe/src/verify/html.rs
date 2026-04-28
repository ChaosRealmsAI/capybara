use std::path::{Path, PathBuf};

use crate::verify::report::{VerifyError, VerifyReport};

pub fn render_index(report: &VerifyReport) -> Result<String, VerifyError> {
    let report_json = serde_json::to_string_pretty(report).map_err(|err| {
        VerifyError::new(
            "EVIDENCE_SERIALIZE_FAILED",
            "$",
            format!("serialize verify report failed: {err}"),
            "next step · rerun capy nextframe verify-export",
        )
    })?;
    Ok(document_html(report, &report_json))
}

fn document_html(report: &VerifyReport, report_json: &str) -> String {
    let cards = stage_cards(report);
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · Verify Export</title>
<style>{style}</style>
</head>
<body>
<main>
  {header}
  <section class="grid" aria-label="Verify export stages">{cards}</section>
  <details><summary>Full JSON report</summary><pre>{report_json}</pre></details>
</main>
</body>
</html>
"#,
        title = escape_html(&report.title),
        style = STYLE,
        header = header_html(report),
        cards = cards,
        report_json = escape_html(report_json),
    )
}

fn header_html(report: &VerifyReport) -> String {
    let status = if report.ok { "pass" } else { "fail" };
    format!(
        r#"<header>
  <div>
    <h1>{title}</h1>
    <p class="meta">{composition_path}</p>
  </div>
  <div class="badge {status}">{verdict}</div>
</header>"#,
        title = escape_html(&report.title),
        composition_path = escape_html(&report.composition_path.display().to_string()),
        status = status,
        verdict = escape_html(&report.verdict),
    )
}

fn stage_cards(report: &VerifyReport) -> String {
    let snapshot_href = relative_href(
        &report.stages.snapshot.snapshot_path,
        &report.evidence_root,
        &report.composition_path,
    );
    let export_href = relative_href(
        &report.stages.export.output_path,
        &report.evidence_root,
        &report.composition_path,
    );
    [
        stage_card(
            "validate",
            report.stages.validate.ok,
            &validate_fields(report),
            None,
        ),
        stage_card(
            "compile",
            report.stages.compile.ok,
            &compile_fields(report),
            None,
        ),
        stage_card(
            "snapshot",
            report.stages.snapshot.ok,
            &snapshot_fields(report),
            Some(Media::Image(snapshot_href)),
        ),
        stage_card(
            "export",
            report.stages.export.ok,
            &export_fields(report),
            Some(Media::Video(export_href)),
        ),
    ]
    .join("\n")
}

fn validate_fields(report: &VerifyReport) -> Vec<Field> {
    vec![
        field(
            "track_count",
            report.stages.validate.track_count.to_string(),
        ),
        field(
            "asset_count",
            report.stages.validate.asset_count.to_string(),
        ),
        field("components", report.stages.validate.components.join(", ")),
    ]
}

fn compile_fields(report: &VerifyReport) -> Vec<Field> {
    vec![
        field(
            "render_source_path",
            report
                .stages
                .compile
                .render_source_path
                .display()
                .to_string(),
        ),
        field("compile_mode", report.stages.compile.compile_mode.clone()),
        field(
            "render_source_schema",
            report.stages.compile.render_source_schema.clone(),
        ),
    ]
}

fn snapshot_fields(report: &VerifyReport) -> Vec<Field> {
    vec![
        field(
            "snapshot_path",
            report.stages.snapshot.snapshot_path.display().to_string(),
        ),
        field("byte_size", report.stages.snapshot.byte_size.to_string()),
        field(
            "dimensions",
            format!(
                "{}x{}",
                report.stages.snapshot.width, report.stages.snapshot.height
            ),
        ),
    ]
}

fn export_fields(report: &VerifyReport) -> Vec<Field> {
    vec![
        field(
            "output_path",
            report.stages.export.output_path.display().to_string(),
        ),
        field("byte_size", report.stages.export.byte_size.to_string()),
        field("frame_count", report.stages.export.frame_count.to_string()),
    ]
}

fn stage_card(name: &str, ok: bool, fields: &[Field], media: Option<Media>) -> String {
    let status = if ok { "pass" } else { "fail" };
    let symbol = if ok { "✓" } else { "×" };
    let rows = fields
        .iter()
        .map(|item| {
            format!(
                "<dt>{}</dt><dd>{}</dd>",
                escape_html(&item.name),
                escape_html(&item.value)
            )
        })
        .collect::<String>();
    let media_html = match media {
        Some(Media::Image(href)) if ok => format!(
            r#"<a href="{href}"><img src="{href}" alt="snapshot preview"></a>"#,
            href = escape_html(&href)
        ),
        Some(Media::Video(href)) if ok => format!(
            r#"<video controls src="{href}"></video><p class="meta"><a href="{href}">export.mp4</a></p>"#,
            href = escape_html(&href)
        ),
        Some(Media::Image(href)) => format!(
            r#"<p class="meta"><a href="{href}">snapshot.png</a></p>"#,
            href = escape_html(&href)
        ),
        Some(Media::Video(href)) => format!(
            r#"<p class="meta"><a href="{href}">export.mp4</a></p>"#,
            href = escape_html(&href)
        ),
        None => String::new(),
    };

    format!(
        r#"<article class="stage-card">
  <div class="stage-head"><h2>{name}</h2><span class="status {status}">{symbol} {status}</span></div>
  <dl>{rows}</dl>
  {media_html}
</article>"#,
        name = escape_html(name),
        status = status,
        symbol = symbol,
        rows = rows,
        media_html = media_html
    )
}

fn relative_href(path: &Path, evidence_root: &Path, composition_path: &Path) -> String {
    let composition_root = composition_path.parent().unwrap_or_else(|| Path::new("."));
    if let Ok(stripped) = path.strip_prefix(composition_root) {
        return path_to_href(&PathBuf::from("..").join(stripped));
    }
    if let Ok(stripped) = path.strip_prefix(evidence_root) {
        return path_to_href(stripped);
    }
    path_to_href(path)
}

fn path_to_href(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn field(name: impl Into<String>, value: impl Into<String>) -> Field {
    Field {
        name: name.into(),
        value: value.into(),
    }
}

struct Field {
    name: String,
    value: String,
}

enum Media {
    Image(String),
    Video(String),
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

const STYLE: &str = r#"
:root {
  color-scheme: dark;
  --bg: #13091f;
  --panel: #211132;
  --panel-2: #2b1740;
  --line: #7c3aed;
  --text: #f7f0ff;
  --muted: #cdb9df;
  --pass: #34d399;
  --fail: #fb7185;
  --link: #c4b5fd;
}
* { box-sizing: border-box; }
body {
  margin: 0;
  min-height: 100vh;
  background: radial-gradient(circle at top left, #3b1d58 0, #1b0d2b 34rem, var(--bg) 100%);
  color: var(--text);
  font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}
main {
  width: min(1180px, calc(100vw - 40px));
  margin: 0 auto;
  padding: 36px 0 44px;
}
header {
  display: flex;
  align-items: end;
  justify-content: space-between;
  gap: 20px;
  margin-bottom: 24px;
}
h1 {
  margin: 0 0 8px;
  font-size: 34px;
  line-height: 1.08;
  letter-spacing: 0;
}
.meta {
  margin: 0;
  color: var(--muted);
  overflow-wrap: anywhere;
}
.badge {
  border: 1px solid currentColor;
  border-radius: 999px;
  padding: 8px 14px;
  font-weight: 700;
  text-transform: uppercase;
}
.badge.pass { color: var(--pass); }
.badge.fail { color: var(--fail); }
.grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
}
.stage-card {
  border: 1px solid rgba(196, 181, 253, 0.28);
  border-radius: 8px;
  background: linear-gradient(180deg, rgba(43, 23, 64, 0.94), rgba(33, 17, 50, 0.96));
  padding: 18px;
  min-width: 0;
}
.stage-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  margin-bottom: 14px;
}
h2 {
  margin: 0;
  font-size: 18px;
  letter-spacing: 0;
}
.status {
  color: var(--muted);
  font-weight: 700;
}
.status.pass { color: var(--pass); }
.status.fail { color: var(--fail); }
dl {
  display: grid;
  grid-template-columns: 150px minmax(0, 1fr);
  gap: 8px 12px;
  margin: 0;
}
dt { color: var(--muted); }
dd {
  margin: 0;
  overflow-wrap: anywhere;
}
a { color: var(--link); }
img, video {
  display: block;
  width: 100%;
  margin-top: 14px;
  border-radius: 6px;
  border: 1px solid rgba(196, 181, 253, 0.22);
  background: #0d0715;
}
img { aspect-ratio: 16 / 9; object-fit: cover; }
video { max-height: 320px; }
details {
  margin-top: 18px;
  border: 1px solid rgba(196, 181, 253, 0.28);
  border-radius: 8px;
  background: rgba(19, 9, 31, 0.74);
  padding: 16px;
}
summary {
  cursor: pointer;
  color: var(--link);
  font-weight: 700;
}
pre {
  margin: 16px 0 0;
  overflow: auto;
  color: #eadcff;
  line-height: 1.45;
  font-size: 13px;
}
@media (max-width: 760px) {
  main { width: min(100vw - 24px, 1180px); padding-top: 24px; }
  header { align-items: start; flex-direction: column; }
  .grid { grid-template-columns: 1fr; }
  dl { grid-template-columns: 1fr; }
}
"#;
