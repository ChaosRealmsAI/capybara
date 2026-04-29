use std::process::Command;

use clap::Args;
use serde_json::{Value, json};

#[derive(Debug, Args)]
#[command(after_help = "AI quick start:
  Use `capy doctor` before long AI workflows.
  Required params: none.
  This command is no-spend: it does not start live providers, download models, or require the desktop shell.
  Next topic: `capy help doctor`.")]
pub struct DoctorArgs {}

pub fn handle(_args: DoctorArgs) -> Result<(), String> {
    crate::print_json(&report())
}

fn report() -> Value {
    let socket_path = crate::ipc_client::socket_path();
    let agent = compact_agent_report(capy_shell::agent::doctor());
    let image = serde_json::to_value(capy_image_gen::doctor(
        capy_image_gen::ImageProviderId::ApimartGptImage2,
    ))
    .unwrap_or_else(|error| json!({ "ok": false, "error": error.to_string() }));

    json!({
        "ok": true,
        "kind": "capy-doctor",
        "version": env!("CARGO_PKG_VERSION"),
        "cwd": std::env::current_dir()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|error| format!("cwd unavailable: {error}")),
        "socket": {
            "path": socket_path.display().to_string(),
            "exists": socket_path.exists(),
            "note": "socket is optional for no-spend doctor; run capy shell or capy open for live UI commands"
        },
        "agent": agent,
        "image": image,
        "media_tools": {
            "yt_dlp": tool_status("yt-dlp", &["--version"]),
            "ffmpeg": tool_status("ffmpeg", &["-version"]),
            "ffprobe": tool_status("ffprobe", &["-version"])
        },
        "domain_doctors": [
            "target/debug/capy image doctor",
            "target/debug/capy cutout doctor",
            "target/debug/capy tts doctor",
            "target/debug/capy clips doctor",
            "target/debug/capy timeline doctor"
        ],
        "next_steps": [
            "Use capy help doctor for the health-check workflow.",
            "Use capy help interaction before click/type UI automation.",
            "Use the domain doctor for the workflow you will run next."
        ]
    })
}

fn tool_status(program: &str, args: &[&str]) -> Value {
    match Command::new(program).args(args).output() {
        Ok(output) => json!({
            "available": output.status.success(),
            "stdout": first_line(&String::from_utf8_lossy(&output.stdout)),
            "stderr": first_line(&String::from_utf8_lossy(&output.stderr))
        }),
        Err(error) => json!({
            "available": false,
            "error": error.to_string()
        }),
    }
}

fn compact_agent_report(value: Value) -> Value {
    let Value::Object(map) = value else {
        return value;
    };
    let mut compact = serde_json::Map::new();
    for (key, value) in map {
        compact.insert(key, compact_tool_report(value));
    }
    Value::Object(compact)
}

fn compact_tool_report(value: Value) -> Value {
    let Value::Object(mut map) = value else {
        return value;
    };
    for key in ["version", "error"] {
        if let Some(Value::String(text)) = map.get_mut(key) {
            *text = first_line(text);
        }
    }
    Value::Object(map)
}

fn first_line(value: &str) -> String {
    value.lines().next().unwrap_or_default().trim().to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn first_line_trims_output() {
        assert_eq!(super::first_line("  a  \nb"), "a");
    }

    #[test]
    fn compact_tool_report_keeps_first_line() {
        let value = serde_json::json!({
            "available": true,
            "version": "first\nsecond",
            "error": ""
        });

        assert_eq!(super::compact_tool_report(value)["version"], "first");
    }
}
