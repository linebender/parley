// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Renders a `parley_core` [`ShapedText`] to a [`Pixmap`] for snapshot testing.
//!
//! This rasterizes glyphs with `vello_cpu`'s glyph-run API.
//!
//! # Coordinate model
//!
//! Everything is laid out in a line-local frame `(main, cross)` and mapped to the screen by a
//! single affine ([`Frame`]). `main` runs along the inline axis (the direction the pen advances)
//! from the start of the line; `cross` is the signed distance from the baseline, negative toward
//! ascent and positive toward descent.
//!
//! Within a run, glyphs are placed by composing the pen position with the glyph's own offset.
//! [`RunOrientation::Upright`] glyphs were already shaped along the vertical axis, so they stay
//! upright; [`RunOrientation::Sideways`] glyphs were shaped horizontally and are rotated 90°
//! clockwise to lie along the line. [`RunOrientation::Horizontal`] glyphs are never rotated.
//!
//! `parley_core` doesn't perform layout, so the whole paragraph is laid out on a single visual
//! line.

use core::f64::consts::FRAC_PI_2;

use parley_core::{RunOrientation, ShapedText, reorder_visual};
use peniko::Color;
use peniko::kurbo::{Affine, Point, Rect};
use vello_cpu::{Glyph, Pixmap, RenderContext};

/// Blank space around the text, in pixels.
const MARGIN: f32 = 16.0;

const BACKGROUND: Color = Color::WHITE;
const GLYPH_COLOR: Color = Color::BLACK;

/// Maps line-local coordinates `(main, cross)` to screen coordinates.
///
/// See the module docs for the coordinate model. A horizontal frame is a pure translation; a
/// vertical frame rotates 90° clockwise about its origin so the inline axis runs down the page.
#[derive(Copy, Clone)]
struct Frame {
    /// Screen position of the line-local origin: `main = 0` on the baseline.
    origin: Point,
    /// Whether the line runs down the page, used for vertical writing modes.
    vertical: bool,
}

impl Frame {
    /// The screen affine.
    ///
    /// When [`Self::vertical`] is `true`, the inline-axis magnitude `main` maps to screen `+y`
    /// (down) and `cross` maps to `-x`, so the ascent side (`cross < 0`) faces right.
    fn affine(self) -> Affine {
        let translate = Affine::translate(self.origin.to_vec2());
        if self.vertical {
            translate * Affine::rotate(FRAC_PI_2)
        } else {
            translate
        }
    }

    /// Screen position of a line-local point.
    fn point(self, main: f32, cross: f32) -> Point {
        self.affine() * Point::new(main as f64, cross as f64)
    }
}

/// Renders `shaped` to a [`Pixmap`].
pub(crate) fn render_shaped(shaped: &ShapedText) -> Pixmap {
    // A vertical paragraph has at least one non-horizontal run.
    let vertical = shaped
        .runs()
        .any(|r| r.orientation() != RunOrientation::Horizontal);

    let metrics = shaped.run(0).map(|r| *r.metrics()).unwrap_or_default();
    let ascent = metrics.ascent;
    let descent = metrics.descent;

    let total_advance: f32 = shaped.runs().map(|r| r.advance()).sum();

    // `cross = 0` is the baseline glyphs sit on: the alphabetic baseline for horizontal text, the
    // central baseline (column center) for vertical text. The line spans the full cross extent
    // (ascent + descent).
    let cross_extent = ascent + descent;

    let (width, height, origin) = if vertical {
        // Main runs down the page; the central baseline sits half the line in from the left, so
        // ascent fits to the right and the descent side fits to the left.
        let width = (2.0 * MARGIN + cross_extent).ceil() as u16;
        let height = (2.0 * MARGIN + total_advance).ceil() as u16;
        let origin = Point::new((MARGIN + 0.5 * cross_extent) as f64, MARGIN as f64);
        (width, height, origin)
    } else {
        let width = (2.0 * MARGIN + total_advance).ceil() as u16;
        let height = (2.0 * MARGIN + cross_extent).ceil() as u16;
        let origin = Point::new(MARGIN as f64, (MARGIN + ascent) as f64);
        (width, height, origin)
    };
    let frame = Frame { origin, vertical };

    let mut ctx = RenderContext::new(width, height);
    ctx.set_paint(BACKGROUND);
    ctx.fill_rect(&Rect::new(0.0, 0.0, width as f64, height as f64));

    // Place runs in visual order along the inline axis.
    let mut runs: Vec<_> = shaped.runs().collect();
    reorder_visual(&mut runs, |r| r.bidi_level());

    let mut pen = 0.0_f32;
    for run in runs {
        // Within a run clusters are stored in logical order; an RTL run advances backward along
        // the line, so walk it in reverse to place clusters in visual order.
        let mut clusters: Vec<_> = run.clusters().collect();
        if run.is_rtl() {
            clusters.reverse();
        }

        // Upright runs are shaped along the vertical axis and stay upright;
        // Sideways runs are shaped horizontally and rotated 90° clockwise to lie
        // along the line. Horizontal runs are never rotated.
        let glyph_rotation = match run.orientation() {
            RunOrientation::Sideways => Affine::rotate(FRAC_PI_2),
            RunOrientation::Horizontal | RunOrientation::Upright => Affine::IDENTITY,
        };

        // Upright glyphs are centered on the central baseline by the shaper, and
        // horizontal glyphs sit on `cross = 0` directly. A sideways run was shaped
        // on its alphabetic baseline, which is off-center, so shift it by half the
        // run's ascent/descent span to center its box on the central baseline —
        // otherwise it would hang to the ascent side of upright neighbors.
        let baseline_cross = if run.orientation() == RunOrientation::Sideways {
            let m = run.metrics();
            0.5 * (m.ascent - m.descent)
        } else {
            0.0
        };

        let mut glyphs = Vec::new();
        for cluster in &clusters {
            let cluster_main = pen;
            // The pen sits on the run's baseline; the glyph's own offset is rotated
            // into place and added in screen space.
            let pen_screen = frame.point(cluster_main, baseline_cross);
            for g in cluster.glyphs() {
                let offset = glyph_rotation * Point::new(g.x as f64, g.y as f64);
                let pos = pen_screen + offset.to_vec2();
                glyphs.push(Glyph {
                    id: g.id,
                    x: pos.x as f32,
                    y: pos.y as f32,
                });
            }
            pen += cluster.advance();
        }

        // Inline-box runs have no font or glyphs; their reserved space is simply
        // left blank.
        if let Some(font) = run.font() {
            ctx.set_paint(GLYPH_COLOR);
            ctx.glyph_run(font)
                .font_size(run.font_size())
                .hint(false)
                .glyph_transform(glyph_rotation)
                .normalized_coords(run.normalized_coords())
                .fill_glyphs(glyphs.into_iter());
        }
    }

    ctx.flush();
    let mut pixmap = Pixmap::new(width, height);
    ctx.render_to_pixmap(&mut pixmap);
    pixmap
}
