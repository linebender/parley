// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Context for layout.

use alloc::{vec, vec::Vec};

use super::FontContext;
use super::builder::{RangedBuilder, StyleRunBuilder};
use super::resolve::tree::TreeStyleBuilder;
use super::resolve::{RangedStyleBuilder, ResolveContext, ResolvedStyle, StyleRun};
use super::style::{Brush, TextStyle};

use crate::StyleProperty;
use crate::analysis::{AnalysisDataSources, CharInfo};
use crate::bidi::BidiResolver;
use crate::builder::TreeBuilder;
use crate::inline_box::InlineBox;
use crate::resolve::ResolvedProperty;
use crate::shape::{ShapeContext, ShapeSink};

/// Shared scratch space used when constructing text layouts.
///
/// This type is designed to be a global resource with only one per-application (or per-thread).
pub struct LayoutContext<B: Brush = [u8; 4]> {
    pub(crate) rcx: ResolveContext,
    pub(crate) style_table: Vec<ResolvedStyle<B>>,
    pub(crate) style_runs: Vec<StyleRun>,
    pub(crate) inline_boxes: Vec<InlineBox>,
    pub(crate) bidi: BidiResolver,

    // Reusable style builders (to amortise allocations)
    pub(crate) ranged_style_builder: RangedStyleBuilder<B>,
    pub(crate) tree_style_builder: TreeStyleBuilder<B>,

    // u16: style index for character
    pub(crate) info: Vec<(CharInfo, u16)>,
    pub(crate) scx: ShapeContext,

    // Unicode analysis data sources (provided by icu)
    pub(crate) analysis_data_sources: AnalysisDataSources,
}

impl<B: Brush> LayoutContext<B> {
    pub fn new() -> Self {
        Self {
            rcx: ResolveContext::default(),
            style_table: vec![],
            style_runs: vec![],
            inline_boxes: vec![],
            bidi: BidiResolver::new(),
            ranged_style_builder: RangedStyleBuilder::default(),
            tree_style_builder: TreeStyleBuilder::default(),
            info: vec![],
            analysis_data_sources: AnalysisDataSources::new(),
            scx: ShapeContext::default(),
        }
    }

    fn resolve_style_set(
        &mut self,
        font_ctx: &mut FontContext,
        scale: f32,
        raw_style: &TextStyle<'_, '_, B>,
    ) -> ResolvedStyle<B> {
        self.rcx
            .resolve_entire_style_set(font_ctx, raw_style, scale)
    }

    #[allow(dead_code)]
    fn resolve_style_property(
        &mut self,
        fcx: &mut FontContext,
        scale: f32,
        property: &StyleProperty<'_, B>,
    ) -> ResolvedProperty<B> {
        self.rcx.resolve_property(fcx, property, scale)
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
        self.begin();

        fcx.source_cache.prune(128, false);

        StyleRunBuilder {
            scale,
            quantize,
            len: text.len(),
            lcx: self,
            fcx,
            cursor: 0,
        }
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
        if text.is_empty() && self.style_runs.is_empty() {
            self.style_table.push(ResolvedStyle::default());
            self.style_runs.push(StyleRun {
                style_index: 0,
                range: 0..0,
            });
        }
        assert!(
            !self.style_runs.is_empty(),
            "at least one style run is required"
        );

        crate::analysis::analyze_text(self, text);

        sink.clear();
        sink.set_scale(scale);
        sink.set_quantize(quantize);
        sink.set_base_level(self.bidi.base_level());
        sink.set_text_len(text.len());

        let mut char_index = 0;
        for style_run in &self.style_runs {
            for _ in text[style_run.range.clone()].chars() {
                self.info[char_index].1 = style_run.style_index;
                char_index += 1;
            }
        }

        // Copy the visual styles into the layout
        sink.push_styles(&self.style_table);

        // Sort the inline boxes as subsequent code assumes that they are in text index order.
        // Note: It's important that this is a stable sort to allow users to control the order of contiguous inline boxes
        self.inline_boxes.sort_by_key(|b| b.index);

        {
            let query = fcx.collection.query(&mut fcx.source_cache);
            super::shape::shape_text(
                &self.rcx,
                query,
                &self.style_table,
                &self.inline_boxes,
                &self.info,
                self.bidi.levels(),
                &mut self.scx,
                text,
                sink,
                &self.analysis_data_sources,
            );
        }

        // Move inline boxes into the layout
        let boxes = core::mem::take(&mut self.inline_boxes);
        self.inline_boxes = sink.set_inline_boxes(boxes);

        sink.finish();
    }

    fn begin(&mut self) {
        self.rcx.clear();
        self.style_table.clear();
        self.style_runs.clear();
        self.inline_boxes.clear();
        self.info.clear();
        self.bidi.clear();
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
