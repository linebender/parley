// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A simple example that lays out some text using Parley, rasterises the glyph using Swash
//! and and then renders it into a PNG using the `image` crate.

use image::codecs::png::PngEncoder;
use image::{self, Pixel, Rgba, RgbaImage};
use parley::layout::{Alignment, GlyphRun, Layout};
use parley::style::{FontStack, FontWeight, StyleProperty};
use parley::{FontContext, LayoutContext};
use peniko::Color;
use std::fs::File;
use swash::scale::image::{Content, Image as SwashImage};
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::{zeno, NormalizedCoord};
use swash::{FontRef, GlyphId};

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
    let foreground_color = Color::rgb8(0, 0, 0);
    let background_color = Color::rgb8(255, 255, 255);

    // Padding around the output image
    let padding = 20;

    // Create a FontContext, LayoutContext and ScaleContext
    //
    // These are all intended to be constructed rarely (perhaps even once per app (or once per thread))
    // and provide caches and scratch space to avoid allocations
    let mut font_cx = FontContext::default();
    let mut layout_cx = LayoutContext::new();
    let mut scale_cx = ScaleContext::new();

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
    let mut layout: Layout<Color> = builder.build();

    // Perform layout (including bidi resolution and shaping) with start alignment
    layout.break_all_lines(max_advance, Alignment::Start);
    let width = layout.width().ceil() as u32;
    let height = layout.height().ceil() as u32;

    let mut img = RgbaImage::new(width + (padding * 2), height + (padding * 2));
    for pixel in img.pixels_mut() {
        *pixel = Rgba([
            background_color.r,
            background_color.g,
            background_color.b,
            255,
        ]);
    }

    // Iterate over laid out lines
    for line in layout.lines() {
        // Iterate over GlyphRun's within each line
        for glyph_run in line.glyph_runs() {
            render_glyph_run(&mut scale_cx, &glyph_run, &mut img, padding);
        }
    }

    // Write image to PNG file in examples/_output dir
    let output_path = {
        let path = std::path::PathBuf::from(file!());
        let mut path = std::fs::canonicalize(path).unwrap();
        path.pop();
        path.pop();
        path.pop();
        path.push("_output");
        let _ = std::fs::create_dir(path.clone());
        path.push("swash_render.png");
        path
    };
    let output_file = File::create(output_path).unwrap();
    let png_encoder = PngEncoder::new(output_file);
    img.write_with_encoder(png_encoder).unwrap();
}

fn render_glyph_run(
    context: &mut ScaleContext,
    glyph_run: &GlyphRun<Color>,
    img: &mut RgbaImage,
    padding: u32,
) {
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
    let normalized_coords = run.normalized_coords();

    // Convert from parley::Font to swash::FontRef
    let font_ref = FontRef::from_index(font.data.as_ref(), font.index as usize).unwrap();

    // Iterates over the glyphs in the GlyphRun
    for glyph in glyph_run.glyphs() {
        let glyph_id: GlyphId = glyph.id;
        let glyph_x = run_x + glyph.x;
        let glyph_y = run_y - glyph.y;
        run_x += glyph.advance;
        let Some(rendered_glyph) = render_glyph(
            context,
            &font_ref,
            font_size,
            normalized_coords,
            glyph_id,
            glyph_x.fract(),
            glyph_y.fract(),
        ) else {
            println!("No glyph");
            continue;
        };

        let glyph_width = usize::try_from(rendered_glyph.placement.width).expect("usize < 32 bits");
        let glyph_height =
            usize::try_from(rendered_glyph.placement.height).expect("usize < 32 bits");
        let glyph_origin_x =
            glyph_x.floor() as i32 + rendered_glyph.placement.left + padding as i32;
        let glyph_origin_y =
            (glyph_y.floor() as i32) - rendered_glyph.placement.top + padding as i32;

        match rendered_glyph.content {
            Content::Mask => {
                let mut i = 0;
                for off_y in 0..glyph_height as i32 {
                    for off_x in 0..glyph_width as i32 {
                        let x = (glyph_origin_x + off_x) as u32;
                        let y = (glyph_origin_y + off_y) as u32;
                        let alpha = rendered_glyph.data[i];
                        let color = Rgba([color.r, color.g, color.b, alpha]);
                        img.get_pixel_mut(x, y).blend(&color);
                        i += 1;
                    }
                }
            }
            Content::SubpixelMask => unimplemented!(),
            Content::Color => {
                for (off_y, row) in rendered_glyph
                    .data
                    .chunks_exact(glyph_width * 4)
                    .enumerate()
                {
                    for (off_x, pixel) in row.chunks_exact(4).enumerate() {
                        let &[r, g, b, a] = pixel else {
                            panic!("Pixel doesn't have 4 components")
                        };
                        let x = (glyph_origin_x + off_x as i32) as u32;
                        let y = (glyph_origin_y + off_y as i32) as u32;
                        let color = Rgba([r, g, b, a]);
                        img.get_pixel_mut(x, y).blend(&color);
                    }
                }
            }
        };
    }
}

/// Render a glyph using Swash
fn render_glyph(
    context: &mut ScaleContext,
    font: &FontRef,
    font_size: f32,
    normalized_coords: &[NormalizedCoord],
    glyph_id: GlyphId,
    x: f32,
    y: f32,
) -> Option<SwashImage> {
    use zeno::{Format, Vector};

    // Build the scaler
    let mut scaler = context
        .builder(*font)
        .size(font_size)
        .hint(true)
        .normalized_coords(normalized_coords)
        .build();

    // Compute the fractional offset
    // You'll likely want to quantize this in a real renderer
    let offset = Vector::new(x.fract(), y.fract());

    // Select our source order
    Render::new(&[
        Source::ColorOutline(0),
        Source::ColorBitmap(StrikeWith::BestFit),
        Source::Outline,
    ])
    // Select the simple alpha (non-subpixel) format
    .format(Format::Alpha)
    // Apply the fractional offset
    .offset(offset)
    // Render the image
    .render(&mut scaler, glyph_id)
}
