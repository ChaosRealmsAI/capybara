use clap::Args;
use serde_json::json;

#[derive(Debug, Args)]
#[command(after_help = "AI quick start:
  Use `capy click --query <css>` after `capy devtools --query <css>` proves the target exists.
  Required params: --query.
  Pitfalls: query is a CSS selector in the active desktop window; keep CAPYBARA_SOCKET consistent.
  Next topic: `capy help interaction`.")]
pub struct ClickArgs {
    #[arg(long, help = "CSS selector for the element to click")]
    query: String,
    #[arg(long, help = "Target window id from capy ps")]
    window: Option<String>,
}

#[derive(Debug, Args)]
#[command(after_help = "AI quick start:
  Use `capy type --query <css> --text <text>` for DOM-safe input.
  Required params: --query and --text.
  Flags: --clear replaces existing input, --submit dispatches change and Enter.
  Pitfalls: query is a CSS selector in the active desktop window; use click first for complex widgets.
  Next topic: `capy help interaction`.")]
pub struct TypeArgs {
    #[arg(long, help = "CSS selector for input, textarea, or contenteditable")]
    query: String,
    #[arg(long, help = "Text to insert")]
    text: String,
    #[arg(long, help = "Clear existing value before inserting text")]
    clear: bool,
    #[arg(long, help = "Dispatch change plus Enter key events after typing")]
    submit: bool,
    #[arg(long, help = "Target window id from capy ps")]
    window: Option<String>,
}

pub fn click(args: ClickArgs) -> Result<(), String> {
    crate::send(
        "devtools-eval",
        json!({
            "eval": click_script(&args.query)?,
            "window": args.window
        }),
    )
}

pub fn type_text(args: TypeArgs) -> Result<(), String> {
    crate::send(
        "devtools-eval",
        json!({
            "eval": type_script(&args.query, &args.text, args.clear, args.submit)?,
            "window": args.window
        }),
    )
}

fn click_script(query: &str) -> Result<String, String> {
    let selector = json_string(query)?;
    Ok(format!(
        r#"(function() {{
  function reply(value) {{ return JSON.stringify(value); }}
  const selector = {selector};
  const el = document.querySelector(selector);
  if (!el) return reply({{ ok: false, error: "selector not found: " + selector }});
  const rect = el.getBoundingClientRect();
  if (!rect.width || !rect.height) return reply({{ ok: false, error: "selector has empty bounds: " + selector }});
  el.scrollIntoView({{ block: "center", inline: "center" }});
  const after = el.getBoundingClientRect();
  const x = after.left + after.width / 2;
  const y = after.top + after.height / 2;
  const eventInit = {{ bubbles: true, cancelable: true, view: window, clientX: x, clientY: y, button: 0, buttons: 1 }};
  if (typeof PointerEvent === "function") el.dispatchEvent(new PointerEvent("pointerdown", eventInit));
  el.dispatchEvent(new MouseEvent("mousedown", eventInit));
  if (typeof PointerEvent === "function") el.dispatchEvent(new PointerEvent("pointerup", {{ ...eventInit, buttons: 0 }}));
  el.dispatchEvent(new MouseEvent("mouseup", {{ ...eventInit, buttons: 0 }}));
  el.dispatchEvent(new MouseEvent("click", {{ ...eventInit, buttons: 0 }}));
  if (typeof el.focus === "function") el.focus();
  return reply({{ ok: true, action: "click", selector, text: (el.innerText || el.value || "").slice(0, 200), rect: {{ x: after.x, y: after.y, width: after.width, height: after.height }} }});
}})()"#
    ))
}

fn type_script(query: &str, text: &str, clear: bool, submit: bool) -> Result<String, String> {
    let selector = json_string(query)?;
    let text = json_string(text)?;
    let clear = json!(clear);
    let submit = json!(submit);
    Ok(format!(
        r#"(function() {{
  function reply(value) {{ return JSON.stringify(value); }}
  const selector = {selector};
  const text = {text};
  const clear = {clear};
  const submit = {submit};
  const el = document.querySelector(selector);
  if (!el) return reply({{ ok: false, error: "selector not found: " + selector }});
  if (typeof el.focus === "function") el.focus();
  const isEditable = el.isContentEditable || el.getAttribute("contenteditable") === "true";
  if ("value" in el) {{
    el.value = clear ? text : String(el.value || "") + text;
  }} else if (isEditable) {{
    el.textContent = clear ? text : String(el.textContent || "") + text;
  }} else {{
    return reply({{ ok: false, error: "selector is not typeable: " + selector }});
  }}
  el.dispatchEvent(new InputEvent("input", {{ bubbles: true, inputType: clear ? "insertReplacementText" : "insertText", data: text }}));
  if (submit) {{
    el.dispatchEvent(new Event("change", {{ bubbles: true }}));
    el.dispatchEvent(new KeyboardEvent("keydown", {{ bubbles: true, cancelable: true, key: "Enter", code: "Enter" }}));
    el.dispatchEvent(new KeyboardEvent("keyup", {{ bubbles: true, cancelable: true, key: "Enter", code: "Enter" }}));
  }}
  return reply({{ ok: true, action: "type", selector, submitted: submit, length: text.length, value: ("value" in el ? String(el.value) : String(el.textContent || "")).slice(0, 500) }});
}})()"#
    ))
}

fn json_string(value: &str) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|error| format!("serialize JavaScript string failed: {error}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn scripts_escape_selector_and_text() -> Result<(), String> {
        let click = super::click_script(r#"[data-x="a"]"#)?;
        assert!(click.contains(r#"[data-x=\"a\"]"#));

        let typed = super::type_script("#prompt", "hello\nworld", true, false)?;
        assert!(typed.contains(r#"hello\nworld"#));
        Ok(())
    }
}
