// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A simple example that lays out some text using Parley, extracts outlines using Skrifa and
//! then paints those outlines using Tiny-Skia.

use std::path::PathBuf;

use parley::layout::{Alignment, GlyphRun, Layout};
use parley::style::{FontStack, FontWeight, StyleProperty};
use parley::{FontContext, LayoutContext};
use peniko::Color as PenikoColor;
use skrifa::instance::{LocationRef, NormalizedCoord, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::FontRef as ReadFontsRef;
use skrifa::{GlyphId, MetadataProvider, OutlineGlyph};
use tiny_skia::{
    Color as TinySkiaColor, FillRule, Paint, PathBuilder, Pixmap, PixmapMut, Transform,
};

fn main() {
    // The text we are going to style and lay out
    let text = String::from(
        "Some text here. Let's make it a bit longer so that line wrapping kicks in 😊. And also some اللغة العربية arabic text.",
    );

    // The display scale for HiDPI rendering
    let display_scale = 1.0;

    // The width for line wrapping
    let max_advance = Some(200.0 * display_scale);

    // Colours for rendering
    let foreground_color = PenikoColor::rgb8(0, 0, 0);
    let background_color = PenikoColor::rgb8(255, 255, 255);

    // Padding around the output image
    let padding = 20;

    // Create a FontContext, LayoutContext
    //
    // These are both intended to be constructed rarely (perhaps even once per app (or once per thread))
    // and provide caches and scratch space to avoid allocations
    let mut font_cx = FontContext::default();
    let mut layout_cx = LayoutContext::new();

    // Create a RangedBuilder
    let mut builder = layout_cx.ranged_builder(&mut font_cx, &text, display_scale);

    // Set default text colour styles (set foreground text color)
    let brush_style = StyleProperty::Brush(foreground_color);
    builder.push_default(&brush_style);

    // Set default font family
    let font_stack = FontStack::Source("system-ui");
    let font_stack_style = StyleProperty::FontStack(font_stack);
    builder.push_default(&font_stack_style);
    builder.push_default(&StyleProperty::LineHeight(1.3));
    builder.push_default(&StyleProperty::FontSize(16.0));

    // Set the first 4 characters to bold
    let bold = FontWeight::new(600.0);
    let bold_style = StyleProperty::FontWeight(bold);
    builder.push(&bold_style, 0..4);

    // Build the builder into a Layout
    let mut layout: Layout<PenikoColor> = builder.build();

    // Perform layout (including bidi resolution and shaping) with start alignment
    layout.break_all_lines(max_advance, Alignment::Start);
    let width = layout.width().ceil() as u32;
    let height = layout.height().ceil() as u32;
    let padded_width = width + padding * 2;
    let padded_height = height + padding * 2;

    // Create TinySkia Pixmap
    let mut img = Pixmap::new(padded_width, padded_height).unwrap();

    // Fill background color
    img.fill(to_tiny_skia(background_color));

    // Wrap Pixmap in a type that implements skrifa::OutlinePen
    let mut pen = TinySkiaPen::new(img.as_mut());

    // Render each glyph run
    for line in layout.lines() {
        for glyph_run in line.glyph_runs() {
            render_glyph_run(&glyph_run, &mut pen, padding);
        }
    }

    // Write image to PNG file in examples/_output dir
    let output_path: PathBuf = {
        let path = std::path::PathBuf::from(file!());
        let mut path = std::fs::canonicalize(path).unwrap();
        path.pop();
        path.pop();
        path.pop();
        path.push("_output");
        let _ = std::fs::create_dir(path.clone());
        path.push("tiny_skia_render.png");
        path
    };
    img.save_png(output_path).unwrap();
}

fn to_tiny_skia(color: PenikoColor) -> TinySkiaColor {
    TinySkiaColor::from_rgba8(color.r, color.g, color.b, color.a)
}

fn render_glyph_run(glyph_run: &GlyphRun<PenikoColor>, pen: &mut TinySkiaPen<'_>, padding: u32) {
    // Resolve properties of the GlyphRun
    let mut run_x = glyph_run.offset();
    let run_y = glyph_run.baseline();
    let style = glyph_run.style();
    let color = style.brush;

    // Get the "Run" from the "GlyphRun"
    let run = glyph_run.run();

    // Resolve properties of the Run
    let font = run.font();
    let font_size = run.font_size();

    let normalized_coords = run
        .normalized_coords()
        .iter()
        .map(|coord| skrifa::instance::NormalizedCoord::from_bits(*coord))
        .collect::<Vec<_>>();

    // Get glyph outlines using Skrifa. This can be cached in production code.
    let font_collection_ref = font.data.as_ref();
    let font_ref = ReadFontsRef::from_index(font_collection_ref, font.index).unwrap();
    let outlines = font_ref.outline_glyphs();

    // Iterates over the glyphs in the GlyphRun
    for glyph in glyph_run.glyphs() {
        let glyph_x = run_x + glyph.x + padding as f32;
        let glyph_y = run_y - glyph.y + padding as f32;
        run_x += glyph.advance;

        let glyph_id = GlyphId::from(glyph.id);
        let glyph_outline = outlines.get(glyph_id).unwrap();

        pen.set_origin(glyph_x, glyph_y);
        pen.set_color(to_tiny_skia(color));
        pen.draw_glyph(&glyph_outline, font_size, &normalized_coords);
    }
}

struct TinySkiaPen<'a> {
    pixmap: PixmapMut<'a>,
    x: f32,
    y: f32,
    paint: Paint<'static>,
    open_path: PathBuilder,
}

impl TinySkiaPen<'_> {
    fn new(pixmap: PixmapMut) -> TinySkiaPen {
        TinySkiaPen {
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

    fn set_color(&mut self, color: TinySkiaColor) {
        self.paint.set_color(color);
    }

    fn draw_glyph(
        &mut self,
        glyph: &OutlineGlyph<'_>,
        size: f32,
        normalized_coords: &[NormalizedCoord],
    ) {
        let location_ref = LocationRef::new(normalized_coords);
        let settings = DrawSettings::unhinted(Size::new(size), location_ref);
        glyph.draw(settings, self).unwrap();

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
