// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A simple example that lays out some text using Parley, extracts outlines using Skrifa and
//! then paints those outlines using Tiny-Skia.
//!
//! Note: Emoji rendering is not currently implemented in this example. See the swash example
//! if you need emoji rendering.

use crate::{GlyphRun, Layout, PositionedLayoutItem};
use peniko::kurbo;
use skrifa::{
    GlyphId, MetadataProvider, OutlineGlyph,
    instance::{LocationRef, NormalizedCoord, Size},
    outline::{DrawSettings, OutlinePen},
    raw::FontRef as ReadFontsRef,
};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, PixmapMut, Rect, Transform};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ColorBrush {
    pub(crate) color: Color,
}

impl ColorBrush {
    pub(crate) fn new(color: peniko::Color) -> Self {
        let rgba8 = color.to_rgba8();
        Self {
            color: Color::from_rgba8(rgba8.r, rgba8.g, rgba8.b, rgba8.a),
        }
    }
}

impl Default for ColorBrush {
    fn default() -> Self {
        Self {
            color: Color::BLACK,
        }
    }
}

pub(crate) struct RenderingConfig {
    pub background_color: Color,
    pub padding_color: Color,
    pub inline_box_color: Color,
    pub cursor_color: Color,
    /// The selection color is chosen based on line index.
    pub selection_colors: [Color; 2],

    /// The width of the pixmap in pixels, excluding padding.
    pub size: Option<kurbo::Size>,
}

fn draw_rect(pen: &mut TinySkiaPen<'_>, x: f32, y: f32, width: f32, height: f32, color: Color) {
    pen.set_origin(x, y);
    pen.set_color(color);
    pen.fill_rect(width, height);
}

/// Render the layout to a [`Pixmap`].
///
/// If given [`RenderingConfig::size`] is not specified, [`Layout::width`] and [`Layout::height`]
/// are used.
pub(crate) fn render_layout(
    config: &RenderingConfig,
    layout: &Layout<ColorBrush>,
    cursor_rect: Option<crate::Rect>,
    selection_rects: &[(crate::Rect, usize)],
) -> Pixmap {
    let padding = 20;
    let width = config
        .size
        .map(|size| size.width as f32)
        .unwrap_or(layout.width())
        .ceil() as u32;
    let height = config
        .size
        .map(|size| size.height as f32)
        .unwrap_or(layout.height())
        .ceil() as u32;
    let padded_width = width + padding * 2;
    let padded_height = height + padding * 2;
    let fpadding = padding as f32;

    let mut img = Pixmap::new(padded_width, padded_height).unwrap();

    img.fill(config.padding_color);

    let mut pen = TinySkiaPen::new(img.as_mut());

    draw_rect(
        &mut pen,
        fpadding,
        fpadding,
        width as f32,
        height as f32,
        config.background_color,
    );

    for (rect, lidx) in selection_rects {
        draw_rect(
            &mut pen,
            fpadding + rect.x0 as f32,
            fpadding + rect.y0 as f32,
            rect.width() as f32,
            rect.height() as f32,
            config.selection_colors[lidx % 2],
        );
    }

    if let Some(rect) = cursor_rect {
        draw_rect(
            &mut pen,
            fpadding + rect.x0 as f32,
            fpadding + rect.y0 as f32,
            rect.width() as f32,
            rect.height() as f32,
            config.cursor_color,
        );
    }

    // Render each glyph run
    for line in layout.lines() {
        for item in line.items() {
            match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => {
                    render_glyph_run(&glyph_run, &mut pen, padding);
                }
                PositionedLayoutItem::InlineBox(inline_box) => {
                    draw_rect(
                        &mut pen,
                        inline_box.x + fpadding,
                        inline_box.y + fpadding,
                        inline_box.width,
                        inline_box.height,
                        config.inline_box_color,
                    );
                }
            };
        }
    }
    img
}

fn render_glyph_run(glyph_run: &GlyphRun<'_, ColorBrush>, pen: &mut TinySkiaPen<'_>, padding: u32) {
    // Resolve properties of the GlyphRun
    let mut run_x = glyph_run.offset();
    let run_y = glyph_run.baseline();
    let style = glyph_run.style();
    let brush = style.brush;

    // Get the "Run" from the "GlyphRun"
    let run = glyph_run.run();

    // Resolve properties of the Run
    let font = run.font();
    let font_size = run.font_size();

    let normalized_coords = run
        .normalized_coords()
        .iter()
        .map(|coord| NormalizedCoord::from_bits(*coord))
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

        let glyph_id = GlyphId::from(glyph.id as u16);
        if let Some(glyph_outline) = outlines.get(glyph_id) {
            pen.set_origin(glyph_x, glyph_y);
            pen.set_color(brush.color);
            pen.draw_glyph(&glyph_outline, font_size, &normalized_coords);
        }
    }

    // Draw decorations: underline & strikethrough
    let style = glyph_run.style();
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

impl TinySkiaPen<'_> {
    fn new(pixmap: PixmapMut<'_>) -> TinySkiaPen<'_> {
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

    fn set_color(&mut self, color: Color) {
        self.paint.set_color(color);
    }

    fn fill_rect(&mut self, width: f32, height: f32) {
        let rect = Rect::from_xywh(self.x, self.y, width, height).expect("Invalid rect");
        self.pixmap
            .fill_rect(rect, &self.paint, Transform::identity(), None);
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
