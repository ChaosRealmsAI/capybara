mod catalog;
mod docs;
mod media_docs;

use catalog::{
    AGENT_TOPICS, CANVAS_TOPICS, CAPY_TOPICS, CHAT_TOPICS, CLIPS_TOPICS, CUTOUT_TOPICS,
    GAME_ASSETS_TOPICS, IMAGE_TOPICS, MEDIA_TOPICS, TIMELINE_TOPICS,
};

pub fn print_capy_topic(topic: Option<&str>) -> Result<(), String> {
    print_topic(topic, CAPY_TOPICS, "capy help")
}

pub fn print_image_topic(topic: Option<&str>) -> Result<(), String> {
    print_topic(topic, IMAGE_TOPICS, "capy image help")
}

pub fn print_cutout_topic(topic: Option<&str>) -> Result<(), String> {
    print_topic(topic, CUTOUT_TOPICS, "capy cutout help")
}

pub fn print_game_assets_topic(topic: Option<&str>) -> Result<(), String> {
    print_topic(topic, GAME_ASSETS_TOPICS, "capy game-assets help")
}

pub fn print_canvas_topic(topic: Option<&str>) -> Result<(), String> {
    print_topic(topic, CANVAS_TOPICS, "capy canvas help")
}

pub fn print_chat_topic(topic: Option<&str>) -> Result<(), String> {
    print_topic(topic, CHAT_TOPICS, "capy chat help")
}

pub fn print_agent_topic(topic: Option<&str>) -> Result<(), String> {
    print_topic(topic, AGENT_TOPICS, "capy agent help")
}

pub fn print_clips_topic(topic: Option<&str>) -> Result<(), String> {
    print_topic(topic, CLIPS_TOPICS, "capy clips help")
}

pub fn print_media_topic(topic: Option<&str>) -> Result<(), String> {
    print_topic(topic, MEDIA_TOPICS, "capy media help")
}

pub fn print_timeline_topic(topic: Option<&str>) -> Result<(), String> {
    print_topic(topic, TIMELINE_TOPICS, "capy timeline help")
}

fn print_topic(topic: Option<&str>, topics: &[HelpTopic], command: &str) -> Result<(), String> {
    let Some(topic) = topic else {
        println!("{}", topic_index(topics, command));
        return Ok(());
    };
    let normalized = topic.trim().to_ascii_lowercase().replace('_', "-");
    if let Some(help) = topics.iter().find(|item| {
        item.name == normalized || item.aliases.iter().any(|alias| *alias == normalized)
    }) {
        println!("{}", help.body.trim());
        return Ok(());
    }
    Err(format!(
        "unknown help topic `{topic}`. Available topics: {}",
        topics
            .iter()
            .map(|item| item.name)
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn topic_index(topics: &[HelpTopic], command: &str) -> String {
    let mut lines = vec!["Available self-contained help topics:".to_string()];
    for topic in topics {
        lines.push(format!("  {:<20} {}", topic.name, topic.summary));
    }
    lines.push(String::new());
    lines.push(format!("Run `{command} <topic>`."));
    lines.join("\n")
}

struct HelpTopic {
    name: &'static str,
    aliases: &'static [&'static str],
    summary: &'static str,
    body: &'static str,
}
