//! Font loading and text rendering with skrifa + vello draw_glyphs.
//!
//! Supports multiple font families (SansSerif, Serif, Mono, Handwritten),
//! bold/italic variants, and Chinese fallback.

use std::sync::Arc;

use vello::kurbo::Affine;
use vello::peniko::{Blob, Color, Fill, FontData};
use vello::{Glyph, Scene};

use crate::state::FontFamily;

/// A single loaded font variant (bytes + FontData for vello).
#[derive(Clone)]
struct FontVariant {
    data: FontData,
    bytes: Arc<Vec<u8>>,
}

impl FontVariant {
    fn load_from_bytes(bytes: &[u8]) -> Self {
        let arc = Arc::new(bytes.to_vec());
        let data = FontData::new(Blob::from(arc.as_ref().clone()), 0);
        Self { data, bytes: arc }
    }

    fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// All loaded fonts: per-family variants + CJK fallback.
#[derive(Clone)]
pub struct FontPair {
    // Legacy public fields for backward compat
    pub en: FontData,
    pub en_bytes: Arc<Vec<u8>>,
    pub zh: FontData,
    pub zh_bytes: Arc<Vec<u8>>,

    // Font family variants
    sans: FontVariant,
    sans_bold: FontVariant,
    serif: FontVariant,
    mono: FontVariant,
    mono_bold: FontVariant,
    handwritten: FontVariant,
    zh_fallback: FontVariant,
}

impl std::fmt::Debug for FontPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontPair")
            .field("en_bytes_len", &self.en_bytes.len())
            .field("zh_bytes_len", &self.zh_bytes.len())
            .finish()
    }
}

impl FontPair {
    /// Load fonts from caller-provided bytes. Empty data is accepted for headless tests.
    pub fn load_from_bytes(
        sans: &[u8],
        serif: &[u8],
        mono: &[u8],
        handwritten: &[u8],
        zh: &[u8],
    ) -> Self {
        let sans = FontVariant::load_from_bytes(sans);
        let sans_bold = sans.clone();
        let serif = FontVariant::load_from_bytes(serif);
        let mono = FontVariant::load_from_bytes(mono);
        let mono_bold = mono.clone();
        let handwritten = if handwritten.is_empty() {
            sans.clone()
        } else {
            FontVariant::load_from_bytes(handwritten)
        };
        let zh_fallback = FontVariant::load_from_bytes(zh);
        let zh_font = zh_fallback.data.clone();
        let zh_arc = zh_fallback.bytes.clone();

        Self {
            en: sans.data.clone(),
            en_bytes: sans.bytes.clone(),
            zh: zh_font,
            zh_bytes: zh_arc,
            sans,
            sans_bold,
            serif,
            mono,
            mono_bold,
            handwritten,
            zh_fallback,
        }
    }

    /// Get the font variant for a given family + bold/italic combination.
    fn variant(&self, family: FontFamily, bold: bool, _italic: bool) -> &FontVariant {
        match family {
            FontFamily::SansSerif => {
                if bold {
                    &self.sans_bold
                } else {
                    &self.sans
                }
            }
            FontFamily::Serif => &self.serif,
            FontFamily::Mono => {
                if bold {
                    &self.mono_bold
                } else {
                    &self.mono
                }
            }
            FontFamily::Handwritten => &self.handwritten,
        }
    }
}

/// Draw text with default sans-serif style (backward-compatible).
#[allow(clippy::too_many_arguments)]
pub fn draw_text(
    scene: &mut Scene,
    fonts: &FontPair,
    text: &str,
    x: f64,
    y: f64,
    size: f32,
    color: Color,
    transform: Affine,
) -> f64 {
    draw_text_styled(
        scene,
        fonts,
        text,
        x,
        y,
        size,
        color,
        transform,
        FontFamily::SansSerif,
        false,
        false,
    )
}

/// Draw text with full styling: font family, bold, italic.
#[allow(clippy::too_many_arguments)]
pub fn draw_text_styled(
    scene: &mut Scene,
    fonts: &FontPair,
    text: &str,
    x: f64,
    y: f64,
    size: f32,
    color: Color,
    transform: Affine,
    family: FontFamily,
    bold: bool,
    italic: bool,
) -> f64 {
    let primary = fonts.variant(family, bold, italic);
    if text.is_empty() || primary.is_empty() {
        return 0.0;
    }

    let en_ref = skrifa::FontRef::from_index(&primary.bytes, 0)
        .or_else(|_| skrifa::FontRef::new(&primary.bytes))
        .ok();
    let zh_ref = skrifa::FontRef::from_index(&fonts.zh_fallback.bytes, 0)
        .or_else(|_| skrifa::FontRef::new(&fonts.zh_fallback.bytes))
        .ok();

    let en_charmap = en_ref.as_ref().map(|f| {
        use skrifa::MetadataProvider;
        f.charmap()
    });
    let zh_charmap = zh_ref.as_ref().map(|f| {
        use skrifa::MetadataProvider;
        f.charmap()
    });
    let en_metrics = en_ref.as_ref().map(|f| {
        use skrifa::MetadataProvider;
        f.glyph_metrics(
            skrifa::instance::Size::new(size),
            skrifa::instance::LocationRef::default(),
        )
    });
    let zh_metrics = zh_ref.as_ref().map(|f| {
        use skrifa::MetadataProvider;
        f.glyph_metrics(
            skrifa::instance::Size::new(size),
            skrifa::instance::LocationRef::default(),
        )
    });

    let mut runs: Vec<(u8, Vec<Glyph>)> = Vec::new();
    let mut pen_x: f32 = 0.0;

    for ch in text.chars() {
        let (font_idx, glyph_id, advance) =
            resolve_glyph_inline(&en_charmap, &zh_charmap, &en_metrics, &zh_metrics, ch, size);
        let glyph = Glyph {
            id: glyph_id,
            x: pen_x,
            y: 0.0,
        };
        if let Some(last) = runs.last_mut() {
            if last.0 == font_idx {
                last.1.push(glyph);
            } else {
                runs.push((font_idx, vec![glyph]));
            }
        } else {
            runs.push((font_idx, vec![glyph]));
        }
        pen_x += advance;
    }

    // Synthetic italic: 12-degree skew if italic requested
    let skew_tf = if italic {
        Affine::new([1.0, 0.0, -0.21, 1.0, 0.0, 0.0])
    } else {
        Affine::IDENTITY
    };

    let text_tf = transform * Affine::translate((x, y + size as f64 * 0.85)) * skew_tf;
    for (font_idx, glyphs) in &runs {
        let font_data = if *font_idx == 0 {
            &primary.data
        } else {
            &fonts.zh_fallback.data
        };
        scene
            .draw_glyphs(font_data)
            .transform(text_tf)
            .font_size(size)
            .brush(color)
            .draw(Fill::NonZero, glyphs.clone().into_iter());
    }

    // Fake bold: draw again with slight x offset for weight simulation
    if bold {
        let bold_offset = (size * 0.03).max(0.5);
        let bold_tf = transform
            * Affine::translate((x + bold_offset as f64, y + size as f64 * 0.85))
            * skew_tf;
        for (font_idx, glyphs) in runs {
            let font_data = if font_idx == 0 {
                &primary.data
            } else {
                &fonts.zh_fallback.data
            };
            scene
                .draw_glyphs(font_data)
                .transform(bold_tf)
                .font_size(size)
                .brush(color)
                .draw(Fill::NonZero, glyphs.into_iter());
        }
    }

    pen_x as f64
}

/// Measure text width (backward-compatible, default sans-serif).
pub fn measure_text(fonts: &FontPair, text: &str, size: f32) -> f64 {
    measure_text_styled(fonts, text, size, FontFamily::SansSerif, false, false)
}

/// Measure text width with font family/bold/italic.
pub fn measure_text_styled(
    fonts: &FontPair,
    text: &str,
    size: f32,
    family: FontFamily,
    bold: bool,
    italic: bool,
) -> f64 {
    let primary = fonts.variant(family, bold, italic);
    if text.is_empty() || primary.is_empty() {
        return 0.0;
    }

    let en_ref = skrifa::FontRef::from_index(&primary.bytes, 0)
        .or_else(|_| skrifa::FontRef::new(&primary.bytes))
        .ok();
    let zh_ref = skrifa::FontRef::from_index(&fonts.zh_fallback.bytes, 0)
        .or_else(|_| skrifa::FontRef::new(&fonts.zh_fallback.bytes))
        .ok();

    let en_charmap = en_ref.as_ref().map(|f| {
        use skrifa::MetadataProvider;
        f.charmap()
    });
    let zh_charmap = zh_ref.as_ref().map(|f| {
        use skrifa::MetadataProvider;
        f.charmap()
    });
    let en_metrics = en_ref.as_ref().map(|f| {
        use skrifa::MetadataProvider;
        f.glyph_metrics(
            skrifa::instance::Size::new(size),
            skrifa::instance::LocationRef::default(),
        )
    });
    let zh_metrics = zh_ref.as_ref().map(|f| {
        use skrifa::MetadataProvider;
        f.glyph_metrics(
            skrifa::instance::Size::new(size),
            skrifa::instance::LocationRef::default(),
        )
    });

    let mut pen_x: f32 = 0.0;
    for ch in text.chars() {
        let (_, _, advance) =
            resolve_glyph_inline(&en_charmap, &zh_charmap, &en_metrics, &zh_metrics, ch, size);
        pen_x += advance;
    }

    pen_x as f64
}

/// Measure prefix width (backward-compatible).
pub fn measure_text_prefix(fonts: &FontPair, text: &str, char_count: usize, size: f32) -> f64 {
    measure_text_prefix_styled(
        fonts,
        text,
        char_count,
        size,
        FontFamily::SansSerif,
        false,
        false,
    )
}

/// Measure prefix width with font styling.
#[allow(clippy::too_many_arguments)]
pub fn measure_text_prefix_styled(
    fonts: &FontPair,
    text: &str,
    char_count: usize,
    size: f32,
    family: FontFamily,
    bold: bool,
    italic: bool,
) -> f64 {
    if char_count == 0 || text.is_empty() {
        return 0.0;
    }
    let primary = fonts.variant(family, bold, italic);
    if primary.is_empty() {
        return 0.0;
    }

    let en_ref = skrifa::FontRef::from_index(&primary.bytes, 0)
        .or_else(|_| skrifa::FontRef::new(&primary.bytes))
        .ok();
    let zh_ref = skrifa::FontRef::from_index(&fonts.zh_fallback.bytes, 0)
        .or_else(|_| skrifa::FontRef::new(&fonts.zh_fallback.bytes))
        .ok();

    let en_charmap = en_ref.as_ref().map(|f| {
        use skrifa::MetadataProvider;
        f.charmap()
    });
    let zh_charmap = zh_ref.as_ref().map(|f| {
        use skrifa::MetadataProvider;
        f.charmap()
    });
    let en_metrics = en_ref.as_ref().map(|f| {
        use skrifa::MetadataProvider;
        f.glyph_metrics(
            skrifa::instance::Size::new(size),
            skrifa::instance::LocationRef::default(),
        )
    });
    let zh_metrics = zh_ref.as_ref().map(|f| {
        use skrifa::MetadataProvider;
        f.glyph_metrics(
            skrifa::instance::Size::new(size),
            skrifa::instance::LocationRef::default(),
        )
    });

    let mut pen_x: f32 = 0.0;
    for (i, ch) in text.chars().enumerate() {
        if i >= char_count {
            break;
        }
        let (_, _, advance) =
            resolve_glyph_inline(&en_charmap, &zh_charmap, &en_metrics, &zh_metrics, ch, size);
        pen_x += advance;
    }

    pen_x as f64
}

// ── Inline glyph resolver (avoids lifetime issues with skrifa) ──

fn resolve_glyph_inline(
    en_charmap: &Option<skrifa::charmap::Charmap<'_>>,
    zh_charmap: &Option<skrifa::charmap::Charmap<'_>>,
    en_metrics: &Option<skrifa::metrics::GlyphMetrics<'_>>,
    zh_metrics: &Option<skrifa::metrics::GlyphMetrics<'_>>,
    ch: char,
    size: f32,
) -> (u8, u32, f32) {
    if let Some(gid) = en_charmap.as_ref().and_then(|cm| cm.map(ch)) {
        let adv = en_metrics
            .as_ref()
            .and_then(|m| m.advance_width(gid))
            .unwrap_or(size * 0.5);
        (0u8, gid.to_u32(), adv)
    } else if let Some(gid) = zh_charmap.as_ref().and_then(|cm| cm.map(ch)) {
        let adv = zh_metrics
            .as_ref()
            .and_then(|m| m.advance_width(gid))
            .unwrap_or(size);
        (1u8, gid.to_u32(), adv)
    } else {
        (0u8, 0, size * 0.5)
    }
}
