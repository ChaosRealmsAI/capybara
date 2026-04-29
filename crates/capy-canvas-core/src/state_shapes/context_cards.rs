use crate::content_card;
use crate::state::{AppState, CanvasContentKind, Shape, ShapeKind};

impl AppState {
    pub fn selected_context(&self) -> crate::shape::CanvasSelectionContext {
        let items = self
            .selected
            .iter()
            .filter_map(|&idx| self.shapes.get(idx).map(|shape| shape.selection_item(idx)))
            .collect::<Vec<_>>();
        crate::shape::CanvasSelectionContext {
            selected_count: items.len(),
            items,
        }
    }

    pub fn selected_context_text(&self) -> String {
        let context = self.selected_context();
        if context.items.is_empty() {
            return String::new();
        }
        context
            .items
            .iter()
            .map(selection_item_text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn create_content_card(
        &mut self,
        kind: CanvasContentKind,
        title: impl Into<String>,
        x: f64,
        y: f64,
    ) -> usize {
        self.push_undo();
        let title = normalize_card_title(kind, title.into());
        let mut shape = Shape::new(ShapeKind::StickyNote, x, y, content_card::fill_color(kind));
        shape.w = 320.0;
        shape.h = 170.0;
        shape.stroke_color = content_card::stroke_color(kind);
        shape.stroke_width = 1.6;
        shape.font_size = 18.0;
        shape.text = format!("{}\n{}", title, content_card::subtitle(kind));
        shape.metadata.content_kind = Some(kind);
        shape.metadata.title = Some(title);
        shape.metadata.status = Some("briefing".to_string());
        shape.metadata.next_action = Some(content_card::default_next_action(kind).to_string());
        let idx = self.add_shape(shape);
        let id = self.shapes[idx].id;
        self.shapes[idx].metadata.editor_route =
            Some(format!("capy://canvas/{}/{id}", kind.as_str()));
        self.selected = vec![idx];
        idx
    }

    pub fn create_poster_document_card(
        &mut self,
        title: impl Into<String>,
        x: f64,
        y: f64,
        source_path: impl Into<String>,
    ) -> usize {
        let idx = self.create_content_card(CanvasContentKind::Poster, title, x, y);
        let source_path = source_path.into();
        let shape = &mut self.shapes[idx];
        shape.w = 420.0;
        shape.h = 277.0;
        shape.text = format!(
            "{}\n{}",
            shape.display_title(),
            content_card::subtitle(CanvasContentKind::Poster)
        );
        shape.metadata.status = Some("ready".to_string());
        if !source_path.trim().is_empty() {
            shape.metadata.source_path = Some(source_path.trim().to_string());
        }
        shape.metadata.refs = vec![
            "poster-json-source".to_string(),
            "html-renderer-output".to_string(),
        ];
        shape.metadata.next_action =
            Some(content_card::default_next_action(CanvasContentKind::Poster).to_string());
        idx
    }
}

fn normalize_card_title(kind: CanvasContentKind, title: String) -> String {
    if title.trim().is_empty() {
        content_card::default_title(kind).to_string()
    } else {
        title.trim().to_string()
    }
}

fn selection_item_text(item: &crate::shape::CanvasSelectionItem) -> String {
    let mut lines = vec![
        format!(
            "- {} [{} · id={}]",
            item.title,
            item.content_kind.as_str(),
            item.id
        ),
        format!(
            "  geometry: x={} y={} w={} h={}",
            item.geometry.x, item.geometry.y, item.geometry.w, item.geometry.h
        ),
    ];
    push_optional_line(
        &mut lines,
        "text",
        Some(item.text.trim()).filter(|v| !v.is_empty()),
    );
    push_optional_line(&mut lines, "status", item.status.as_deref());
    push_optional_line(&mut lines, "owner", item.owner.as_deref());
    if !item.refs.is_empty() {
        lines.push(format!("  refs: {}", item.refs.join(", ")));
    }
    push_optional_line(&mut lines, "next", item.next_action.as_deref());
    push_optional_line(&mut lines, "editor", item.editor_route.as_deref());
    push_optional_line(&mut lines, "source", item.source_path.as_deref());
    push_optional_line(
        &mut lines,
        "generation_provider",
        item.generation_provider.as_deref(),
    );
    push_optional_line(
        &mut lines,
        "generation_prompt",
        item.generation_prompt.as_deref(),
    );
    lines.join("\n")
}

fn push_optional_line(lines: &mut Vec<String>, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        lines.push(format!("  {label}: {value}"));
    }
}
