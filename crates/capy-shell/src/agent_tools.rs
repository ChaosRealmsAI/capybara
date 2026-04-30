use serde_json::Value;

pub(crate) fn agent_tool_env(config: &Value) -> Vec<(String, String)> {
    let mut env = Vec::new();
    if let Some(log_path) = config_str(config, "capyToolLog") {
        env.push(("CAPY_TOOL_CALL_LOG".to_string(), log_path));
    }
    env
}

pub(crate) fn claude_append_system_prompt(
    existing: Option<String>,
    config: &Value,
) -> Option<String> {
    combine_tool_contract(existing, config)
}

pub(crate) fn codex_developer_instructions(
    existing: Option<String>,
    config: &Value,
) -> Option<String> {
    combine_codex_contracts(existing, config)
}

fn combine_tool_contract(existing: Option<String>, config: &Value) -> Option<String> {
    let contract = config_bool(config, "capyCanvasTools").then(capy_canvas_tool_contract);
    match (existing, contract) {
        (Some(existing), Some(contract)) => {
            Some(format!("{}\n\n{}", existing.trim_end(), contract))
        }
        (Some(existing), None) => Some(existing),
        (None, Some(contract)) => Some(contract.to_string()),
        (None, None) => None,
    }
}

fn combine_codex_contracts(existing: Option<String>, config: &Value) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(existing) = existing {
        parts.push(existing.trim_end().to_string());
    }
    if config_bool(config, "capyProjectInstructions") {
        parts.push(capy_codex_communication_contract().to_string());
    }
    if config_bool(config, "capyCanvasTools") {
        parts.push(capy_canvas_tool_contract().to_string());
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

fn capy_codex_communication_contract() -> &'static str {
    r#"Capybara desktop communication contract.

You are replying inside Capybara's right-side Planner chat. The user is a PM/founder and usually wants concise Chinese communication, direct status, and readable deliverables.

Reply format:
- Use Markdown for structure: short headings, bullets, numbered steps, code fences, and concise tables when helpful.
- Keep raw logs short. Summarize first, then include only the important lines.
- For any non-trivial status report, plan, comparison, diagnosis, or handoff, include one fenced `html` block after the Markdown summary.
- The fenced `html` block is a chat preview card, not a web app. Write only a semantic body fragment: sections, headings, paragraphs, lists, tables, stats, and badges.
- The Capybara app injects the visual CSS. Do not include `<style>`, inline `style=""`, scripts, external assets, network calls, forms, tracking, iframes, images, or SVG.
- Keep the HTML compact enough for the right-side chat: one card/table/timeline is usually better than a long document.
- Prefer these built-in classes when useful: `capy-card`, `capy-section`, `capy-kicker`, `capy-title`, `capy-subtitle`, `capy-grid`, `capy-stat`, `capy-table`, `capy-timeline`, `capy-step`, `capy-badges`, `capy-badge`, `capy-note`, `capy-risk`, `capy-callout`, `is-good`, `is-warn`, `is-bad`, `is-muted`.
- Preserve paths, commands, model names, and technical identifiers literally."#
}

fn capy_canvas_tool_contract() -> &'static str {
    r#"Capybara Canvas CLI tools are available.

When the user asks you to create, place, move, or inspect canvas content, use shell commands against the project-local CLI. Do not use browser devtools, arbitrary JavaScript, DOM mutation, or direct provider SDK calls for canvas changes.

Required workflow for understanding the user's selected image or selected region:
1. Run `target/debug/capy canvas snapshot` and inspect `canvas.selectedNode`.
2. For whole-image context, run `target/debug/capy canvas context export --selected --out <output-dir>`.
3. For a user-selected local area, run `target/debug/capy canvas context export --region <x,y,w,h> --out <output-dir>` if the prompt gives coordinates; otherwise use the live UI region by omitting `--region`.
4. Read `<output-dir>/context.json` before answering. Reference the `context_id`, source node id, and relevant attachment paths in your reply.

Required workflow for generated images:
1. Run `target/debug/capy canvas snapshot` and inspect the selected node.
2. If a selected node exists, place the new image at `x = selected.geometry.x + selected.geometry.w + 48` and `y = selected.geometry.y`.
3. Use `target/debug/capy canvas generate-image --dry-run --x <x> --y <y> --title "Generated image" --out <output-dir> --name <slug> "<five-section prompt>"` unless the user explicitly asks for a live provider call.
4. For a live provider call, run `target/debug/capy image doctor`, `target/debug/capy image balance`, then `target/debug/capy canvas generate-image --live ...`.
5. Reply with the inserted node id, source_path, provider, and placement.

The image prompt must include five labeled sections: Scene, Subject, Important details, Use case, Constraints. If the user gives a short request, expand it into those five sections before calling the CLI."#
}

fn config_str(config: &Value, key: &str) -> Option<String> {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn config_bool(config: &Value, key: &str) -> bool {
    config.get(key).and_then(Value::as_bool).unwrap_or(false)
}
