// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A simple example that lays out some text using Parley, extracts outlines using Skrifa and
//! then paints those outlines using Vello CPU through Parley Draw.

#![expect(clippy::cast_possible_truncation, reason = "Deferred")]

use parley::{
    Alignment, AlignmentOptions, FontContext, FontWeight, GenericFamily, GlyphRun, InlineBox,
    Layout, LayoutContext, LineHeight, PositionedLayoutItem, StyleProperty,
};
use parley_draw::{GlyphCaches, GlyphRunBuilder};
use vello_cpu::{Pixmap, RenderContext, kurbo, peniko::Color};

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

fn main() {
    // The text we are going to style and lay out
    let text = String::from(
        "Some text here. Let's make it a bit longer so that line wrapping kicks in 😊. And also some اللغة العربية arabic text.\nThis is underline and strikethrough text",
    );

    // The display scale for HiDPI rendering
    let display_scale = 1.0;

    // Whether to automatically align the output to pixel boundaries, to avoid blurry text.
    let quantize = true;

    // The width for line wrapping
    let max_advance = Some(200.0 * display_scale);

    // Colours for rendering
    let foreground_color = Color::BLACK;
    let background_color = Color::WHITE;

    // Padding around the output image
    let padding = 20;

    // Create a FontContext, LayoutContext
    //
    // These are both intended to be constructed rarely (perhaps even once per app (or once per thread))
    // and provide caches and scratch space to avoid allocations
    let mut font_cx = FontContext::new();
    let mut layout_cx = LayoutContext::new();

    // Create a RangedBuilder
    let mut builder = layout_cx.ranged_builder(&mut font_cx, &text, display_scale, quantize);

    // Set default text colour styles (set foreground text color)
    let foreground_brush = ColorBrush {
        color: foreground_color,
    };
    let brush_style = StyleProperty::Brush(foreground_brush);
    builder.push_default(brush_style);

    // Set default font family
    builder.push_default(GenericFamily::SystemUi);
    builder.push_default(LineHeight::FontSizeRelative(1.3));
    builder.push_default(StyleProperty::FontSize(16.0));

    // Set the first 4 characters to bold
    let bold = FontWeight::new(600.0);
    builder.push(StyleProperty::FontWeight(bold), 0..4);

    // Set the underline & strikethrough style
    builder.push(StyleProperty::Underline(true), 141..150);
    builder.push(StyleProperty::Strikethrough(true), 155..168);

    builder.push_inline_box(InlineBox {
        id: 0,
        index: 40,
        width: 50.0,
        height: 50.0,
    });

    // Build the builder into a Layout
    let mut layout: Layout<ColorBrush> = builder.build(&text);

    // Perform layout (including bidi resolution and shaping) with start alignment
    layout.break_all_lines(max_advance);
    layout.align(max_advance, Alignment::Start, AlignmentOptions::default());
    let width = layout.width().ceil() as u16;
    let height = layout.height().ceil() as u16;
    let padded_width = width + padding * 2;
    let padded_height = height + padding * 2;

    // The renderer and glyph caches should be created once per app (or per thread).
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

    // Render each glyph run
    for line in layout.lines() {
        for item in line.items() {
            match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => {
                    renderer.set_paint(glyph_run.style().brush.color);
                    let run = glyph_run.run();
                    GlyphRunBuilder::new(run.font().clone(), *renderer.transform(), &mut renderer)
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
                            &mut glyph_caches,
                        );

                    let style = glyph_run.style();
                    if let Some(decoration) = &style.underline {
                        let offset = decoration.offset.unwrap_or(run.metrics().underline_offset);
                        let size = decoration.size.unwrap_or(run.metrics().underline_size);

                        render_decoration(
                            &mut renderer,
                            &decoration.brush,
                            &glyph_run,
                            offset,
                            size,
                        );
                    }
                    if let Some(decoration) = &style.strikethrough {
                        let offset = decoration
                            .offset
                            .unwrap_or(run.metrics().strikethrough_offset);
                        let size = decoration.size.unwrap_or(run.metrics().strikethrough_size);

                        render_decoration(
                            &mut renderer,
                            &decoration.brush,
                            &glyph_run,
                            offset,
                            size,
                        );
                    }
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

    let mut pixmap = Pixmap::new(padded_width, padded_height);
    renderer.render_to_pixmap(&mut pixmap);
    // After rendering, we must `maintain` the glyph caches to evict unused cache entries.
    glyph_caches.maintain();

    // Write image to PNG file in examples/_output dir
    let output_path = {
        let path = std::path::PathBuf::from(file!());
        let mut path = std::fs::canonicalize(path).unwrap();
        path.pop();
        path.pop();
        path.pop();
        path.push("_output");
        drop(std::fs::create_dir(path.clone()));
        path.push("vello_cpu_render.png");
        path
    };
    let png = pixmap.into_png().unwrap();
    std::fs::write(output_path, png).unwrap();
}

fn render_decoration(
    renderer: &mut RenderContext,
    brush: &ColorBrush,
    glyph_run: &GlyphRun<'_, ColorBrush>,
    offset: f32,
    size: f32,
) {
    renderer.set_paint(brush.color);
    let y = glyph_run.baseline() - offset;
    let x = glyph_run.offset();
    let x1 = x + glyph_run.advance();
    let y1 = y + size;
    renderer.fill_rect(&kurbo::Rect::new(x as f64, y as f64, x1 as f64, y1 as f64));
}
