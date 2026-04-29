use crate::state::CanvasContentKind;

pub(crate) fn default_title(kind: CanvasContentKind) -> &'static str {
    match kind {
        CanvasContentKind::Project => "Project hub",
        CanvasContentKind::Brand => "Brand system",
        CanvasContentKind::Image => "Image direction",
        CanvasContentKind::Poster => "Poster document",
        CanvasContentKind::Video => "Storyboard",
        CanvasContentKind::Web => "Web page",
        CanvasContentKind::Text => "Copy block",
        CanvasContentKind::Audio => "Audio cue",
        CanvasContentKind::ThreeD => "3D object",
        CanvasContentKind::Shape => "Canvas object",
    }
}

pub(crate) fn subtitle(kind: CanvasContentKind) -> &'static str {
    match kind {
        CanvasContentKind::Project => "brief · scope · assets",
        CanvasContentKind::Brand => "logo · palette · mascot",
        CanvasContentKind::Image => "prompt · references · variants",
        CanvasContentKind::Poster => "JSON · layers · HTML preview",
        CanvasContentKind::Video => "shots · motion · export",
        CanvasContentKind::Web => "sections · states · responsive",
        CanvasContentKind::Text => "headline · tone · variants",
        CanvasContentKind::Audio => "voice · music · timing",
        CanvasContentKind::ThreeD => "model · material · view",
        CanvasContentKind::Shape => "layout · relation · note",
    }
}

pub(crate) fn default_next_action(kind: CanvasContentKind) -> &'static str {
    match kind {
        CanvasContentKind::Project => "open project detail and plan next assets",
        CanvasContentKind::Brand => "generate brand directions and lock tokens",
        CanvasContentKind::Image => "generate image variants from references",
        CanvasContentKind::Poster => "edit poster JSON and render HTML preview",
        CanvasContentKind::Video => "expand into storyboard shots",
        CanvasContentKind::Web => "open page editor and draft sections",
        CanvasContentKind::Text => "write copy variants in selected tone",
        CanvasContentKind::Audio => "draft voice or music direction",
        CanvasContentKind::ThreeD => "open 3D detail and define model views",
        CanvasContentKind::Shape => "describe object role in the layout",
    }
}

pub(crate) fn fill_color(kind: CanvasContentKind) -> u32 {
    match kind {
        CanvasContentKind::Project => 0xfff3bf,
        CanvasContentKind::Brand => 0xffedd5,
        CanvasContentKind::Image => 0xfce7f3,
        CanvasContentKind::Poster => 0xfef3c7,
        CanvasContentKind::Video => 0xdbeafe,
        CanvasContentKind::Web => 0xd1fae5,
        CanvasContentKind::Text => 0xede9fe,
        CanvasContentKind::Audio => 0xfbcfe8,
        CanvasContentKind::ThreeD => 0xc7d2fe,
        CanvasContentKind::Shape => 0xe5e7eb,
    }
}

pub(crate) fn stroke_color(kind: CanvasContentKind) -> u32 {
    match kind {
        CanvasContentKind::Project => 0xd97706,
        CanvasContentKind::Brand => 0xf97316,
        CanvasContentKind::Image => 0xdb2777,
        CanvasContentKind::Poster => 0xa16207,
        CanvasContentKind::Video => 0x2563eb,
        CanvasContentKind::Web => 0x059669,
        CanvasContentKind::Text => 0x7c3aed,
        CanvasContentKind::Audio => 0xbe185d,
        CanvasContentKind::ThreeD => 0x4f46e5,
        CanvasContentKind::Shape => 0x64748b,
    }
}
