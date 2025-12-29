// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Renders styled text using `styled_text` + `styled_text_parley`, then paints glyph outlines using
//! Skrifa and Tiny-Skia.
//!
//! Note: Emoji rendering is not currently implemented in this example. See the swash example if
//! you need emoji rendering.

#![expect(clippy::cast_possible_truncation, reason = "Deferred")]

use core::ops::Range;

use parley::{
    Alignment, AlignmentOptions, FontContext, GlyphRun, Layout, LayoutContext, PositionedLayoutItem,
};
use skrifa::{
    GlyphId, MetadataProvider, OutlineGlyph,
    instance::{LocationRef, NormalizedCoord, Size},
    outline::{DrawSettings, OutlinePen},
    raw::FontRef as ReadFontsRef,
};
use styled_text::StyledText;
use styled_text_parley::build_layout_from_styled_text;
use text_style::{FontSize, InlineStyle, Setting, Settings, Specified, Tag};
use text_style_resolve::{ComputedInlineStyle, ComputedParagraphStyle};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, PixmapMut, Rect, Transform};

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
    let background_color = Color::from_rgba8(250, 250, 252, 255);
    let padding = 24;

    let base_inline = ComputedInlineStyle::default().with_font_size_px(18.0);
    let base_paragraph = ComputedParagraphStyle::default();
    let mut styled = StyledText::new(text.as_str(), base_inline, base_paragraph);

    let r_styled_text = find_range(&text, "StyledText");
    styled
        .apply_span(
            r_styled_text,
            InlineStyle::new()
                .font_size(Specified::Value(FontSize::Px(42.0)))
                .underline(Specified::Value(true)),
        )
        .unwrap();

    styled
        .apply_span(
            find_range(&text, "Parley"),
            InlineStyle::new()
                .font_size(Specified::Value(FontSize::Px(30.0)))
                .strikethrough(Specified::Value(true)),
        )
        .unwrap();

    styled
        .apply_span(
            find_range(&text, "BIG"),
            InlineStyle::new().font_size(Specified::Value(FontSize::Em(2.0))),
        )
        .unwrap();

    styled
        .apply_span(
            find_range(&text, "tiny"),
            InlineStyle::new().font_size(Specified::Value(FontSize::Px(12.0))),
        )
        .unwrap();

    styled
        .apply_span(
            find_range(&text, "underline"),
            InlineStyle::new().underline(Specified::Value(true)),
        )
        .unwrap();

    styled
        .apply_span(
            find_range(&text, "strike"),
            InlineStyle::new().strikethrough(Specified::Value(true)),
        )
        .unwrap();

    styled
        .apply_span(
            find_range(&text, "OpenType settings"),
            InlineStyle::new().font_features(Specified::Value(Settings::list(vec![
                Setting::new(Tag::from_bytes(*b"liga"), 0),
                Setting::new(Tag::from_bytes(*b"kern"), 0),
            ]))),
        )
        .unwrap();

    styled
        .apply_span(
            find_range(&text, "rich-text"),
            InlineStyle::new().font_variations(Specified::Value(Settings::list(vec![
                Setting::new(Tag::from_bytes(*b"wght"), 750.0),
            ]))),
        )
        .unwrap();

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
    )
    .unwrap();

    layout.break_all_lines(max_advance);
    layout.align(max_advance, Alignment::Start, AlignmentOptions::default());

    let width = layout.width().ceil() as u32;
    let height = layout.height().ceil() as u32;
    let padded_width = width + padding * 2;
    let padded_height = height + padding * 2;

    let mut img = Pixmap::new(padded_width, padded_height).unwrap();
    img.fill(background_color);
    let mut pen = TinySkiaPen::new(img.as_mut());

    for line in layout.lines() {
        for item in line.items() {
            match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => {
                    render_glyph_run(&glyph_run, &mut pen, padding);
                }
                PositionedLayoutItem::InlineBox(inline_box) => {
                    pen.set_origin(inline_box.x + padding as f32, inline_box.y + padding as f32);
                    pen.set_color(foreground_color);
                    pen.fill_rect(inline_box.width, inline_box.height);
                }
            }
        }
    }

    let output_path = {
        let path = std::path::PathBuf::from(file!());
        let mut path = std::fs::canonicalize(path).unwrap();
        path.pop();
        path.pop();
        path.pop();
        path.push("_output");
        drop(std::fs::create_dir(path.clone()));
        path.push("tiny_skia_render_styled_text.png");
        path
    };
    img.save_png(output_path).unwrap();
}

fn render_glyph_run(glyph_run: &GlyphRun<'_, ColorBrush>, pen: &mut TinySkiaPen<'_>, padding: u32) {
    let mut run_x = glyph_run.offset();
    let run_y = glyph_run.baseline();
    let style = glyph_run.style();
    let brush = style.brush;

    let run = glyph_run.run();
    let font = run.font();
    let font_size = run.font_size();

    let normalized_coords = run
        .normalized_coords()
        .iter()
        .map(|coord| NormalizedCoord::from_bits(*coord))
        .collect::<Vec<_>>();

    let font_collection_ref = font.data.as_ref();
    let font_ref = ReadFontsRef::from_index(font_collection_ref, font.index).unwrap();
    let outlines = font_ref.outline_glyphs();

    for glyph in glyph_run.glyphs() {
        let glyph_x = run_x + glyph.x + padding as f32;
        let glyph_y = run_y - glyph.y + padding as f32;
        run_x += glyph.advance;

        let glyph_id = GlyphId::from(glyph.id);
        if let Some(glyph_outline) = outlines.get(glyph_id) {
            pen.set_origin(glyph_x, glyph_y);
            pen.set_color(brush.color);
            pen.draw_glyph(&glyph_outline, font_size, &normalized_coords);
        }
    }

    let run_metrics = run.metrics();
    if let Some(decoration) = &style.underline {
        let offset = decoration.offset.unwrap_or(run_metrics.underline_offset);
        let size = decoration.size.unwrap_or(run_metrics.underline_size);
        render_decoration(pen, glyph_run, decoration.brush, offset, size, padding);
    }
    if let Some(decoration) = &style.strikethrough {
        let offset = decoration
            .offset
            .unwrap_or(run_metrics.strikethrough_offset);
        let size = decoration.size.unwrap_or(run_metrics.strikethrough_size);
        render_decoration(pen, glyph_run, decoration.brush, offset, size, padding);
    }
}

fn render_decoration(
    pen: &mut TinySkiaPen<'_>,
    glyph_run: &GlyphRun<'_, ColorBrush>,
    brush: ColorBrush,
    offset: f32,
    width: f32,
    padding: u32,
) {
    let y = glyph_run.baseline() - offset + padding as f32;
    let x = glyph_run.offset() + padding as f32;
    pen.set_color(brush.color);
    pen.set_origin(x, y);
    pen.fill_rect(glyph_run.advance(), width);
}

struct TinySkiaPen<'a> {
    pixmap: PixmapMut<'a>,
    x: f32,
    y: f32,
    paint: Paint<'static>,
    open_path: PathBuilder,
}

impl<'a> TinySkiaPen<'a> {
    fn new(pixmap: PixmapMut<'a>) -> Self {
        Self {
            pixmap,
            x: 0.0,
            y: 0.0,
            paint: Paint::default(),
            open_path: PathBuilder::new(),
        }
    }

    fn set_origin(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    fn set_color(&mut self, color: Color) {
        self.paint.set_color(color);
    }

    fn fill_rect(&mut self, width: f32, height: f32) {
        let rect = Rect::from_xywh(self.x, self.y, width, height).unwrap();
        self.pixmap
            .fill_rect(rect, &self.paint, Transform::identity(), None);
    }

    fn draw_glyph(
        &mut self,
        glyph_outline: &OutlineGlyph<'_>,
        font_size: f32,
        normalized_coords: &[NormalizedCoord],
    ) {
        let location_ref = LocationRef::new(normalized_coords);
        let settings = DrawSettings::unhinted(Size::new(font_size), location_ref);
        glyph_outline.draw(settings, self).unwrap();

        let builder = core::mem::replace(&mut self.open_path, PathBuilder::new());
        if let Some(path) = builder.finish() {
            self.pixmap.fill_path(
                &path,
                &self.paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
}

impl OutlinePen for TinySkiaPen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.open_path.move_to(self.x + x, self.y - y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.open_path.line_to(self.x + x, self.y - y);
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.open_path
            .quad_to(self.x + cx0, self.y - cy0, self.x + x, self.y - y);
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.open_path.cubic_to(
            self.x + cx0,
            self.y - cy0,
            self.x + cx1,
            self.y - cy1,
            self.x + x,
            self.y - y,
        );
    }

    fn close(&mut self) {
        self.open_path.close();
    }
}
