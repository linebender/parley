// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Context for layout.

use super::FontContext;
use super::builder::RangedBuilder;
use super::resolve::RangedStyleBuilder;
use super::resolve::tree::TreeStyleBuilder;
use super::style::{Brush, TextStyle};
use crate::{StyleProperty, TreeBuilder};
use parley_core::{ParleyCoreContext, ResolvedProperty, ResolvedStyle, ShapeSink, StyleRunBuilder};

/// Shared scratch space used when constructing text layouts.
///
/// This type is designed to be a global resource with only one per-application (or per-thread).
pub struct LayoutContext<B: Brush = [u8; 4]> {
    pub(crate) core_ctx: ParleyCoreContext<B>,

    // Reusable style builders (to amortise allocations)
    pub(crate) ranged_style_builder: RangedStyleBuilder<B>,
    pub(crate) tree_style_builder: TreeStyleBuilder<B>,
}

impl<B: Brush> LayoutContext<B> {
    pub fn new() -> Self {
        Self {
            core_ctx: ParleyCoreContext::new(),
            ranged_style_builder: RangedStyleBuilder::default(),
            tree_style_builder: TreeStyleBuilder::default(),
        }
    }

    pub fn resolve_style_set(
        &mut self,
        font_ctx: &mut FontContext,
        scale: f32,
        raw_style: &TextStyle<'_, '_, B>,
    ) -> ResolvedStyle<B> {
        self.core_ctx.resolve_style_set(font_ctx, scale, raw_style)
    }

    pub fn resolve_style_property(
        &mut self,
        fcx: &mut FontContext,
        scale: f32,
        property: &StyleProperty<'_, B>,
    ) -> ResolvedProperty<B> {
        self.core_ctx.resolve_style_property(fcx, scale, property)
    }

    /// Create a ranged style layout builder.
    ///
    /// Set `quantize` as `true` to have the layout coordinates aligned to pixel boundaries.
    /// That is the easiest way to avoid blurry text and to receive ready-to-paint layout metrics.
    ///
    /// For advanced rendering use cases you can set `quantize` as `false` and receive
    /// fractional coordinates. This ensures the most accurate results if you want to perform
    /// some post-processing on the coordinates before painting. To avoid blurry text you will
    /// still need to quantize the coordinates just before painting.
    ///
    /// Your should round at least the following:
    /// * Glyph run baseline
    /// * Inline box baseline
    ///   - `box.y = (box.y + box.height).round() - box.height`
    /// * Selection geometry's `y0` & `y1`
    /// * Cursor geometry's `y0` & `y1`
    ///
    /// Keep in mind that for the simple `f32::round` to be effective,
    /// you need to first ensure the coordinates are in physical pixel space.
    pub fn ranged_builder<'a>(
        &'a mut self,
        fcx: &'a mut FontContext,
        text: &'a str,
        scale: f32,
        quantize: bool,
    ) -> RangedBuilder<'a, B> {
        self.begin();

        let resolved_root_style = self.resolve_style_set(fcx, scale, &TextStyle::default());
        self.ranged_style_builder
            .begin(resolved_root_style, text.len());

        fcx.source_cache.prune(128, false);

        RangedBuilder {
            scale,
            quantize,
            lcx: self,
            fcx,
        }
    }

    /// Create a builder for constructing a layout from indexed style runs.
    ///
    /// Unlike [`Self::ranged_builder`], this builder expects callers to provide:
    /// - a style table of fully specified [`TextStyle`] values (via [`StyleRunBuilder::push_style`])
    /// - a complete sequence of **contiguous**, **non-overlapping** spans that cover
    ///   `0..text.len()` and reference style indices (via [`StyleRunBuilder::push_style_run`])
    ///
    /// Parley then skips its internal range-splitting logic.
    pub fn style_run_builder<'a>(
        &'a mut self,
        fcx: &'a mut FontContext,
        text: &'a str,
        scale: f32,
        quantize: bool,
    ) -> StyleRunBuilder<'a, B> {
        self.core_ctx.style_run_builder(fcx, text, scale, quantize)
    }

    /// Create a tree style layout builder.
    ///
    /// Set `quantize` as `true` to have the layout coordinates aligned to pixel boundaries.
    /// That is the easiest way to avoid blurry text and to receive ready-to-paint layout metrics.
    ///
    /// For advanced rendering use cases you can set `quantize` as `false` and receive
    /// fractional coordinates. This ensures the most accurate results if you want to perform
    /// some post-processing on the coordinates before painting. To avoid blurry text you will
    /// still need to quantize the coordinates just before painting.
    ///
    /// Your should round at least the following:
    /// * Glyph run baseline
    /// * Inline box baseline
    ///   - `box.y = (box.y + box.height).round() - box.height`
    /// * Selection geometry's `y0` & `y1`
    /// * Cursor geometry's `y0` & `y1`
    ///
    /// Keep in mind that for the simple `f32::round` to be effective,
    /// you need to first ensure the coordinates are in physical pixel space.
    pub fn tree_builder<'a>(
        &'a mut self,
        fcx: &'a mut FontContext,
        scale: f32,
        quantize: bool,
        root_style: &TextStyle<'_, '_, B>,
    ) -> TreeBuilder<'a, B> {
        self.begin();

        let resolved_root_style = self.resolve_style_set(fcx, scale, root_style);
        self.tree_style_builder.begin(resolved_root_style);

        fcx.source_cache.prune(128, false);

        TreeBuilder {
            scale,
            quantize,
            lcx: self,
            fcx,
        }
    }

    pub fn build_into(
        &mut self,
        sink: &mut impl ShapeSink<B>,
        scale: f32,
        quantize: bool,
        text: &str,
        fcx: &mut FontContext,
    ) {
        self.core_ctx.build_into(sink, scale, quantize, text, fcx);
    }

    fn begin(&mut self) {
        self.core_ctx.begin();
    }
}

impl<B: Brush> Default for LayoutContext<B> {
    fn default() -> Self {
        Self::new()
    }
}

impl<B: Brush> Clone for LayoutContext<B> {
    fn clone(&self) -> Self {
        // None of the internal state is visible so just return a new instance.
        Self::new()
    }
}
