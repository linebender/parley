// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A simple renderer that lays out some text using Parley, extracts outlines using Skrifa and
//! then paints those outlines using Tiny-Skia.
//!
//! Note: Emoji rendering is not currently implemented in this example. See the swash example
//! if you need emoji rendering.

use std::collections::HashMap;

use parley::{BoundingBox, GlyphRun, Layout, PositionedLayoutItem};
use parley_draw::{GlyphCaches, GlyphRunBuilder};
use peniko::{
    Color,
    kurbo::{self, BezPath, Rect, Stroke},
};
use vello_cpu::{Pixmap, RenderContext};

use crate::util::env::CLUSTER_INFO_COLOR;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ColorBrush {
    pub(crate) color: Color,
}

impl ColorBrush {
    pub(crate) fn new(color: Color) -> Self {
        Self { color }
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

fn draw_rect(renderer: &mut RenderContext, x: f64, y: f64, width: f64, height: f64, color: Color) {
    renderer.set_paint(color);
    renderer.fill_rect(&Rect::new(x, y, x + width, y + height));
}

fn draw_line<T: Into<f64>>(renderer: &mut RenderContext, x1: T, y1: T, x2: T, y2: T) {
    let mut path = BezPath::new();
    path.move_to((x1.into(), y1.into()));
    path.line_to((x2.into(), y2.into()));
    renderer.set_stroke(Stroke::new(1.0));
    renderer.stroke_path(&path);
}

/// Render the layout to a [`Pixmap`].
///
/// If given [`RenderingConfig::size`] is not specified, [`Layout::width`] and [`Layout::height`]
/// are used.
pub(crate) fn render_layout(
    config: &RenderingConfig,
    layout: &Layout<ColorBrush>,
    cursor_rect: Option<BoundingBox>,
    selection_rects: &[(BoundingBox, usize)],
) -> Pixmap {
    let padding = 20;
    let width = config
        .size
        .map(|size| size.width as f32)
        .unwrap_or(layout.width())
        .ceil() as u16;
    let height = config
        .size
        .map(|size| size.height as f32)
        .unwrap_or(layout.height())
        .ceil() as u16;
    let padded_width = width + padding * 2;
    let padded_height = height + padding * 2;
    let fpadding = padding as f64;

    let mut renderer = RenderContext::new(padded_width, padded_height);
    let mut caches = GlyphCaches::new();
    draw_rect(
        &mut renderer,
        0.0,
        0.0,
        padded_width as f64,
        padded_height as f64,
        config.padding_color,
    );
    draw_rect(
        &mut renderer,
        fpadding,
        fpadding,
        width as f64,
        height as f64,
        config.background_color,
    );

    for (rect, lidx) in selection_rects {
        draw_rect(
            &mut renderer,
            fpadding + rect.x0,
            fpadding + rect.y0,
            rect.width(),
            rect.height(),
            config.selection_colors[lidx % 2],
        );
    }

    if let Some(rect) = cursor_rect {
        draw_rect(
            &mut renderer,
            fpadding + rect.x0,
            fpadding + rect.y0,
            rect.width(),
            rect.height(),
            config.cursor_color,
        );
    }

    // Render each glyph run
    for line in layout.lines() {
        for item in line.items() {
            match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => {
                    render_glyph_run(&glyph_run, &mut renderer, &mut caches, padding);
                }
                PositionedLayoutItem::InlineBox(inline_box) => {
                    draw_rect(
                        &mut renderer,
                        inline_box.x as f64 + fpadding,
                        inline_box.y as f64 + fpadding,
                        inline_box.width as f64,
                        inline_box.height as f64,
                        config.inline_box_color,
                    );
                }
            };
        }
    }

    let mut img = Pixmap::new(padded_width, padded_height);
    renderer.render_to_pixmap(&mut img);
    img
}

/// Render the layout with cluster information including measurement lines and source characters.
pub(crate) fn render_layout_with_clusters(
    config: &RenderingConfig,
    layout: &Layout<ColorBrush>,
    char_layouts: &HashMap<char, Layout<ColorBrush>>,
) -> Pixmap {
    let padding = 20;
    let line_extra_spacing = 60.0; // Extra space between lines for cluster info
    let measurement_line_height = 5.0; // Height below baseline for measurement line
    let char_display_offset = 18.0; // Offset below measurement line for character display

    // Calculate dimensions with extra spacing
    let width = config
        .size
        .map(|size| size.width as f32)
        .unwrap_or(layout.width())
        .ceil() as u16;
    let base_height = layout.height();
    let num_lines = layout.len();
    let height = (base_height + (line_extra_spacing * num_lines as f32)).ceil() as u16;
    let padded_width = width + padding * 2;
    let padded_height = height + padding * 2;
    let fpadding = padding as f64;

    let mut renderer = RenderContext::new(padded_width, padded_height);
    let mut caches = GlyphCaches::new();
    draw_rect(
        &mut renderer,
        0.0,
        0.0,
        padded_width as f64,
        padded_height as f64,
        config.padding_color,
    );
    draw_rect(
        &mut renderer,
        fpadding,
        fpadding,
        width as f64,
        height as f64,
        config.background_color,
    );

    // Render each line with clusters
    let mut y_offset = 0.0;
    for line in layout.lines() {
        let line_y = line.metrics().baseline + y_offset;

        // Render the normal text first
        for item in line.items() {
            match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => {
                    render_glyph_run_with_offset(
                        &glyph_run,
                        &mut renderer,
                        &mut caches,
                        padding,
                        (0.0, y_offset),
                    );
                }
                PositionedLayoutItem::InlineBox(_) => {
                    panic!("Inline boxes are not supported in cluster rendering");
                }
            }
        }

        // Now render cluster information below each line.
        for item in line.items() {
            if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                let run = glyph_run.run();
                let mut x_offset = glyph_run.offset();

                for cluster in run.visual_clusters() {
                    let cluster_width = cluster.advance();

                    // Use the test-specific methods we added to Cluster
                    let source_char = cluster.source_char();
                    let expected_len = source_char.len_utf8() as u8;
                    let actual_len = cluster.text_len();

                    assert_eq!(
                        expected_len, actual_len,
                        "Cluster text_len mismatch for '{}': expected {}, got {}",
                        source_char, expected_len, actual_len
                    );

                    // Draw measurement line
                    let measure_y = line_y as f64 + measurement_line_height + fpadding;
                    let measure_x_start = x_offset as f64 + fpadding;
                    let measure_x_end = x_offset as f64 + cluster_width as f64 + fpadding;

                    // Draw horizontal measurement line
                    renderer.set_paint(CLUSTER_INFO_COLOR);
                    draw_line(
                        &mut renderer,
                        measure_x_start,
                        measure_y,
                        measure_x_end,
                        measure_y,
                    );

                    // Draw small vertical ticks at start and end
                    const TICK_HEIGHT: f64 = 3.0;
                    draw_line(
                        &mut renderer,
                        measure_x_start,
                        measure_y - TICK_HEIGHT,
                        measure_x_start,
                        measure_y + TICK_HEIGHT,
                    );
                    draw_line(
                        &mut renderer,
                        measure_x_end,
                        measure_y - TICK_HEIGHT,
                        measure_x_end,
                        measure_y + TICK_HEIGHT,
                    );

                    // Render the character (skip whitespace and control characters)
                    match source_char {
                        ' ' | '\n' | '\t' => {
                            // Skip rendering whitespace
                        }
                        _ => {
                            // Draw the cluster's character glyphs under the measurement line (these
                            // should appear to be the same as the character in the source text).
                            let char_layout = char_layouts.get(&source_char).unwrap();
                            let line = char_layout.lines().next().unwrap();
                            let item = line.items().next().unwrap();
                            let glyph_run = match item {
                                PositionedLayoutItem::GlyphRun(glyph_run) => glyph_run,
                                PositionedLayoutItem::InlineBox(_) => {
                                    panic!("Inline boxes are not supported in cluster rendering");
                                }
                            };

                            // Center each "reference" glyph within the tick marks
                            let char_x_offset =
                                x_offset + ((cluster_width - line.metrics().advance) / 2.0);
                            let char_y_offset = line_y + measurement_line_height as f32
                                - line.metrics().baseline
                                + char_display_offset;
                            render_glyph_run_with_offset(
                                &glyph_run,
                                &mut renderer,
                                &mut caches,
                                padding,
                                (char_x_offset, char_y_offset),
                            );
                        }
                    };
                    x_offset += cluster_width;
                }
            }
        }

        y_offset += line_extra_spacing;
    }

    let mut img = Pixmap::new(padded_width, padded_height);
    renderer.render_to_pixmap(&mut img);
    img
}

fn render_glyph_run(
    glyph_run: &GlyphRun<'_, ColorBrush>,
    renderer: &mut RenderContext,
    caches: &mut GlyphCaches,
    padding: u16,
) {
    render_glyph_run_with_offset(glyph_run, renderer, caches, padding, (0.0, 0.0));
}

fn render_glyph_run_with_offset(
    glyph_run: &GlyphRun<'_, ColorBrush>,
    renderer: &mut RenderContext,
    caches: &mut GlyphCaches,
    padding: u16,
    offset: (f32, f32),
) {
    let padding = padding as f32;
    let (x_offset, y_offset) = offset;
    renderer.set_paint(glyph_run.style().brush.color);
    let run = glyph_run.run();
    GlyphRunBuilder::new(run.font().clone(), *renderer.transform(), renderer)
        .font_size(run.font_size())
        .hint(false)
        .normalized_coords(run.normalized_coords())
        .fill_glyphs(
            glyph_run
                .positioned_glyphs()
                .map(|glyph| parley_draw::Glyph {
                    id: glyph.id,
                    x: glyph.x + padding + x_offset,
                    y: glyph.y + padding + y_offset,
                }),
            caches,
        );

    let style = glyph_run.style();
    if let Some(decoration) = &style.underline {
        let offset = decoration.offset.unwrap_or(run.metrics().underline_offset);
        let size = decoration.size.unwrap_or(run.metrics().underline_size);

        render_decoration_with_offset(
            renderer,
            glyph_run,
            decoration.brush,
            offset as f64,
            size as f64,
            padding as f64,
            y_offset as f64,
        );
    }
    if let Some(decoration) = &style.strikethrough {
        let offset = decoration
            .offset
            .unwrap_or(run.metrics().strikethrough_offset);
        let size = decoration.size.unwrap_or(run.metrics().strikethrough_size);

        render_decoration_with_offset(
            renderer,
            glyph_run,
            decoration.brush,
            offset as f64,
            size as f64,
            padding as f64,
            y_offset as f64,
        );
    }
}

fn render_decoration_with_offset(
    renderer: &mut RenderContext,
    glyph_run: &GlyphRun<'_, ColorBrush>,
    brush: ColorBrush,
    offset: f64,
    width: f64,
    padding: f64,
    y_offset: f64,
) {
    let y = glyph_run.baseline() as f64 - offset + padding + y_offset;
    let x = glyph_run.offset() as f64 + padding;
    draw_rect(
        renderer,
        x,
        y,
        glyph_run.advance() as f64,
        width,
        brush.color,
    );
}
