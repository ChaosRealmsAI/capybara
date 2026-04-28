use std::path::PathBuf;

use serde_json::{Value, json};

use crate::ipc_client;

pub(crate) fn verify(window: Option<String>, capture_out: Option<PathBuf>) -> Result<(), String> {
    let capture_out =
        capture_out.ok_or_else(|| "--profile desktop requires --capture-out=<png>".to_string())?;
    let capture_out = absolute_path(capture_out)?;
    let state = request_data(
        "state-query",
        json!({ "key": "app.ready", "window": window.clone(), "verify": true }),
    )?;
    ensure(
        state.get("value").and_then(Value::as_bool) == Some(true),
        "desktop verify failed: app.ready is not true",
    )?;

    let browser = request_data(
        "devtools-eval",
        json!({
            "window": window.clone(),
            "eval": "({browser:document.documentElement.dataset.capyBrowser,native:document.documentElement.dataset.capybaraNative,ready:document.readyState,title:document.title,topbar:!!document.querySelector('.topbar'),ua:navigator.userAgent})"
        }),
    )?;
    ensure(
        browser.get("browser").and_then(Value::as_str) == Some("cef"),
        "desktop verify failed: browser identity is not cef",
    )?;
    ensure(
        browser
            .get("ua")
            .and_then(Value::as_str)
            .is_some_and(|ua| ua.contains("Chrome")),
        "desktop verify failed: user agent does not contain Chrome",
    )?;
    ensure(
        browser.get("topbar").and_then(Value::as_bool) == Some(true),
        "desktop verify failed: .topbar is missing",
    )?;

    let bridge = request_data(
        "devtools-eval",
        json!({
            "window": window.clone(),
            "eval": "({ipc:typeof window.ipc?.postMessage,bridge:!!window.jsBridge,native:document.documentElement.dataset.capybaraNative})"
        }),
    )?;
    ensure(
        bridge.get("ipc").and_then(Value::as_str) == Some("function")
            && bridge.get("bridge").and_then(Value::as_bool) == Some(true),
        "desktop verify failed: JS bridge is not ready",
    )?;

    let topbar = request_data(
        "devtools-query",
        json!({ "query": ".topbar", "get": "bounding-rect", "window": window.clone() }),
    )?;
    let rect = topbar.get("value").unwrap_or(&Value::Null);
    ensure(
        rect.get("width")
            .and_then(Value::as_f64)
            .unwrap_or_default()
            > 0.0
            && rect
                .get("height")
                .and_then(Value::as_f64)
                .unwrap_or_default()
                > 0.0,
        "desktop verify failed: .topbar has empty bounds",
    )?;

    let console = request_data(
        "devtools-eval",
        json!({
            "window": window.clone(),
            "eval": "({consoleEvents:(window.__capyConsoleEvents||[]).slice(-20),pageErrors:window.__capyPageErrors||[]})"
        }),
    )?;
    let page_errors = console
        .get("pageErrors")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    ensure(
        page_errors == 0,
        "desktop verify failed: page errors are present",
    )?;

    let capture = request_data(
        "capture",
        json!({ "out": capture_out.display().to_string(), "window": window }),
    )?;
    ensure(
        capture
            .get("bytes")
            .and_then(Value::as_u64)
            .unwrap_or_default()
            > 100_000,
        "desktop verify failed: native capture is too small",
    )?;

    let summary = json!({
        "ok": true,
        "profile": "desktop",
        "socket": ipc_client::socket_path().display().to_string(),
        "checks": {
            "app_ready": state,
            "browser": browser,
            "bridge": bridge,
            "topbar": topbar,
            "console": console,
            "capture": capture
        }
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&summary).map_err(|err| err.to_string())?
    );
    Ok(())
}

fn request_data(op: &str, params: Value) -> Result<Value, String> {
    let request = ipc_client::request(op, params);
    let response = ipc_client::send(request)?;
    if response.ok {
        return Ok(response.data.unwrap_or(Value::Null));
    }
    Err(response
        .error
        .map(|value| value.to_string())
        .unwrap_or_else(|| "capy IPC request failed".to_string()))
}

fn ensure(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.to_string())
    }
}

fn absolute_path(path: PathBuf) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(path);
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .map_err(|err| format!("read cwd failed: {err}"))
}
