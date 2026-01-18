// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Renders styled text using `styled_text` + `styled_text_parley`, then paints glyph outlines using
//! Vello CPU through Parley Draw.
//!
//! Note: Emoji rendering is not currently implemented in this example. See the swash example if
//! you need emoji rendering.

#![expect(clippy::cast_possible_truncation, reason = "Deferred")]

use core::ops::Range;

use parley::{
    Alignment, AlignmentOptions, FontContext, GlyphRun, Layout, LayoutContext, PositionedLayoutItem,
};
use parley_draw::{GlyphCaches, GlyphRunBuilder};
use styled_text::{
    ComputedInlineStyle, ComputedParagraphStyle, FontFeature, FontFeatures, FontSize,
    FontVariation, FontVariations, InlineStyle, StyledText, Tag,
};
use styled_text_parley::build_layout_from_styled_text;
use vello_cpu::{RenderContext, kurbo, peniko::Color};

#[derive(Clone, Copy, Debug, PartialEq)]
struct ColorBrush {
    color: Color,
}

impl Default for ColorBrush {
    fn default() -> Self {
        Self {
            color: Color::BLACK,
        }
    }
}

fn find_range(text: &str, needle: &str) -> Range<usize> {
    let start = text
        .find(needle)
        .unwrap_or_else(|| panic!("missing substring {needle:?}"));
    start..start + needle.len()
}

fn main() {
    let text = concat!(
        "StyledText + Parley\n",
        "A small, no_std + alloc rich-text layer.\n",
        "\n",
        "BIG, tiny, underline, strike, and OpenType settings.\n",
        "Some bidirectional sample: English العربية.\n"
    )
    .to_string();

    let display_scale = 1.0;
    let quantize = true;
    let max_advance = Some(520.0 * display_scale);

    let foreground_color = Color::BLACK;
    let background_color = Color::from_rgb8(250, 250, 252);
    let padding: u16 = 24;

    let base_inline = ComputedInlineStyle::default().with_font_size_px(18.0);
    let base_paragraph = ComputedParagraphStyle::default();
    let mut styled = StyledText::new(text.as_str(), base_inline, base_paragraph);

    let styled_text_style = InlineStyle::new()
        .with_font_size(FontSize::Px(42.0))
        .with_underline(true);
    let parley_style = InlineStyle::new()
        .with_font_size(FontSize::Px(30.0))
        .with_strikethrough(true);
    let big_style = InlineStyle::new().with_font_size(FontSize::Em(2.0));
    let tiny_style = InlineStyle::new().with_font_size(FontSize::Px(12.0));
    let underline_style = InlineStyle::new().with_underline(true);
    let strike_style = InlineStyle::new().with_strikethrough(true);
    let opentype_features_style = InlineStyle::new().with_font_features(FontFeatures::list(vec![
        FontFeature::new(Tag::new(b"liga"), 0),
        FontFeature::new(Tag::new(b"kern"), 0),
    ]));
    let variation_style =
        InlineStyle::new().with_font_variations(FontVariations::list(vec![FontVariation::new(
            Tag::new(b"wght"),
            750.0,
        )]));

    styled.apply_span(
        styled.range(find_range(&text, "StyledText")).unwrap(),
        styled_text_style,
    );

    styled.apply_span(
        styled.range(find_range(&text, "Parley")).unwrap(),
        parley_style,
    );

    styled.apply_span(styled.range(find_range(&text, "BIG")).unwrap(), big_style);

    styled.apply_span(styled.range(find_range(&text, "tiny")).unwrap(), tiny_style);

    styled.apply_span(
        styled.range(find_range(&text, "underline")).unwrap(),
        underline_style,
    );

    styled.apply_span(
        styled.range(find_range(&text, "strike")).unwrap(),
        strike_style,
    );

    styled.apply_span(
        styled
            .range(find_range(&text, "OpenType settings"))
            .unwrap(),
        opentype_features_style,
    );

    styled.apply_span(
        styled.range(find_range(&text, "rich-text")).unwrap(),
        variation_style,
    );

    let mut font_cx = FontContext::new();
    let mut layout_cx = LayoutContext::new();

    let foreground_brush = ColorBrush {
        color: foreground_color,
    };
    let mut layout: Layout<ColorBrush> = build_layout_from_styled_text(
        &mut layout_cx,
        &mut font_cx,
        &styled,
        display_scale,
        quantize,
        foreground_brush,
    );

    layout.break_all_lines(max_advance);
    layout.align(max_advance, Alignment::Start, AlignmentOptions::default());

    let width = layout.width().ceil() as u16;
    let height = layout.height().ceil() as u16;
    let padded_width = width + padding * 2;
    let padded_height = height + padding * 2;

    let mut renderer = RenderContext::new(padded_width, padded_height);
    let mut glyph_caches = GlyphCaches::new();

    renderer.set_paint(background_color);
    renderer.fill_rect(&kurbo::Rect::new(
        0.0,
        0.0,
        padded_width as f64,
        padded_height as f64,
    ));
    renderer.set_transform(kurbo::Affine::translate(kurbo::Vec2::new(
        padding as f64,
        padding as f64,
    )));

    for line in layout.lines() {
        for item in line.items() {
            match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => {
                    render_glyph_run(&mut renderer, &mut glyph_caches, &glyph_run);
                }
                PositionedLayoutItem::InlineBox(inline_box) => {
                    renderer.set_paint(foreground_color);
                    let (x0, y0) = (inline_box.x as f64, inline_box.y as f64);
                    let (x1, y1) = (x0 + inline_box.width as f64, y0 + inline_box.height as f64);
                    renderer.fill_rect(&kurbo::Rect::new(x0, y0, x1, y1));
                }
            }
        }
    }

    let mut pixmap = vello_cpu::Pixmap::new(padded_width, padded_height);
    renderer.render_to_pixmap(&mut pixmap);
    glyph_caches.maintain();

    let output_path = {
        let path = std::path::PathBuf::from(file!());
        let mut path = std::fs::canonicalize(path).unwrap();
        path.pop();
        path.pop();
        path.pop();
        path.push("_output");
        drop(std::fs::create_dir(path.clone()));
        path.push("vello_cpu_render_styled_text.png");
        path
    };
    let png = pixmap.into_png().unwrap();
    std::fs::write(output_path, png).unwrap();
}

fn render_glyph_run(
    renderer: &mut RenderContext,
    glyph_caches: &mut GlyphCaches,
    glyph_run: &GlyphRun<'_, ColorBrush>,
) {
    renderer.set_paint(glyph_run.style().brush.color);
    let run = glyph_run.run();
    GlyphRunBuilder::new(run.font().clone(), *renderer.transform(), renderer)
        .font_size(run.font_size())
        .hint(true)
        .normalized_coords(run.normalized_coords())
        .fill_glyphs(
            glyph_run
                .positioned_glyphs()
                .map(|glyph| parley_draw::Glyph {
                    id: glyph.id,
                    x: glyph.x,
                    y: glyph.y,
                }),
            glyph_caches,
        );

    let style = glyph_run.style();
    if let Some(decoration) = &style.underline {
        let offset = decoration.offset.unwrap_or(run.metrics().underline_offset);
        let size = decoration.size.unwrap_or(run.metrics().underline_size);
        render_decoration(renderer, &decoration.brush, glyph_run, offset, size);
    }
    if let Some(decoration) = &style.strikethrough {
        let offset = decoration
            .offset
            .unwrap_or(run.metrics().strikethrough_offset);
        let size = decoration.size.unwrap_or(run.metrics().strikethrough_size);
        render_decoration(renderer, &decoration.brush, glyph_run, offset, size);
    }
}

fn render_decoration(
    renderer: &mut RenderContext,
    brush: &ColorBrush,
    glyph_run: &GlyphRun<'_, ColorBrush>,
    offset: f32,
    size: f32,
) {
    renderer.set_paint(brush.color);

    let run = glyph_run.run();
    let x0 = glyph_run.offset();
    let x1 = x0 + run.advance();
    let y = glyph_run.baseline() - offset;

    renderer.fill_rect(&kurbo::Rect::new(
        x0 as f64,
        (y - size * 0.5) as f64,
        x1 as f64,
        (y + size * 0.5) as f64,
    ));
}
