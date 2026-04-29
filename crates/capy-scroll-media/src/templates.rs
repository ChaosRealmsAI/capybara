mod multi_video;
mod single_video;

pub use multi_video::{multi_video_story_css, multi_video_story_html, multi_video_story_js};
pub use single_video::{demo_html, raw_quality_html, runtime_css, runtime_js, scroll_hq_html};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_supports_hq_clip_selector() {
        let js = runtime_js();
        assert!(js.contains("root.dataset.clip"));
        assert!(js.contains("manifest.hq_clip"));
    }

    #[test]
    fn hq_scroll_entry_loads_hq_clip_without_copy() {
        let html = scroll_hq_html();
        assert!(html.contains("data-clip=\"hq\""));
        assert!(html.contains("data-manifest=\"manifest.json\""));
        assert!(!html.contains("manifest-hq.json"));
    }
}
