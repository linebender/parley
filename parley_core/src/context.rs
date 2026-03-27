// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Context for layout.

use alloc::{vec, vec::Vec};

use super::FontContext;
use super::builder::StyleRunBuilder;
use super::resolve::{ResolveContext, ResolvedStyle, StyleRun};
use super::style::{Brush, TextStyle};

use crate::StyleProperty;
use crate::analysis::{AnalysisDataSources, CharInfo};
use crate::bidi::BidiResolver;
use crate::inline_box::InlineBox;
use crate::resolve::ResolvedProperty;
use crate::shape::{ShapeContext, ShapeSink};

/// Shared scratch space used when constructing text layouts.
///
/// This type is designed to be a global resource with only one per-application (or per-thread).
pub struct ParleyCoreContext<B: Brush = [u8; 4]> {
    pub(crate) rcx: ResolveContext,
    pub style_table: Vec<ResolvedStyle<B>>,
    pub style_runs: Vec<StyleRun>,
    pub inline_boxes: Vec<InlineBox>,

    pub(crate) bidi: BidiResolver,

    // u16: style index for character
    pub(crate) info: Vec<(CharInfo, u16)>,
    pub(crate) scx: ShapeContext,

    // Unicode analysis data sources (provided by icu)
    pub(crate) analysis_data_sources: AnalysisDataSources,
}

impl<B: Brush> ParleyCoreContext<B> {
    pub fn new() -> Self {
        Self {
            rcx: ResolveContext::default(),
            style_table: vec![],
            style_runs: vec![],
            inline_boxes: vec![],
            bidi: BidiResolver::new(),
            info: vec![],
            analysis_data_sources: AnalysisDataSources::new(),
            scx: ShapeContext::default(),
        }
    }

    pub fn resolve_style_set(
        &mut self,
        font_ctx: &mut FontContext,
        scale: f32,
        raw_style: &TextStyle<'_, '_, B>,
    ) -> ResolvedStyle<B> {
        self.rcx
            .resolve_entire_style_set(font_ctx, raw_style, scale)
    }

    pub fn resolve_style_property(
        &mut self,
        fcx: &mut FontContext,
        scale: f32,
        property: &StyleProperty<'_, B>,
    ) -> ResolvedProperty<B> {
        self.rcx.resolve_property(fcx, property, scale)
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

    pub fn push_inline_box(&mut self, inline_box: InlineBox) {
        self.inline_boxes.push(inline_box);
    }

    pub fn set_style_table(&mut self, styles: impl ExactSizeIterator<Item = ResolvedStyle<B>>) {
        self.style_table.clear();
        self.style_table.reserve(styles.len());
        self.style_table.extend(styles);
    }

    pub fn set_style_runs(&mut self, style_runs: impl ExactSizeIterator<Item = StyleRun>) {
        self.style_runs.clear();
        self.style_runs.reserve(style_runs.len());
        self.style_runs.extend(style_runs);
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

    pub fn begin(&mut self) {
        self.rcx.clear();
        self.style_table.clear();
        self.style_runs.clear();
        self.inline_boxes.clear();
        self.info.clear();
        self.bidi.clear();
    }
}

impl<B: Brush> Default for ParleyCoreContext<B> {
    fn default() -> Self {
        Self::new()
    }
}

impl<B: Brush> Clone for ParleyCoreContext<B> {
    fn clone(&self) -> Self {
        // None of the internal state is visible so just return a new instance.
        Self::new()
    }
}
