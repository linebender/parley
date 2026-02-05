// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Context for layout.

use alloc::{vec, vec::Vec};

use super::FontContext;
use super::builder::RangedBuilder;
use super::resolve::tree::TreeStyleBuilder;
use super::resolve::{RangedStyle, RangedStyleBuilder, ResolveContext, ResolvedStyle};
use super::style::{Brush, TextStyle};

use crate::analysis::{AnalysisDataSources, CharInfo};
use crate::bidi::BidiResolver;
use crate::builder::TreeBuilder;
use crate::inline_box::InlineBox;
use crate::shape::ShapeContext;

/// Shared scratch space used when constructing text layouts.
///
/// This type is designed to be a global resource with only one per-application (or per-thread).
pub struct LayoutContext<B: Brush = [u8; 4]> {
    pub(crate) rcx: ResolveContext,
    pub(crate) styles: Vec<RangedStyle<B>>,
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
            styles: vec![],
            inline_boxes: vec![],
            bidi: BidiResolver::new(),
            ranged_style_builder: RangedStyleBuilder::default(),
            tree_style_builder: TreeStyleBuilder::default(),
            info: vec![],
            analysis_data_sources: AnalysisDataSources::new(),
            scx: ShapeContext::default(),
        }
    }

    /// Loads runtime segmenter models for improved word/line breaking.
    ///
    /// By default, Parley uses rule-based segmentation which works well for most languages but produces suboptimal
    /// results for languages like Thai, Lao, Khmer, and Burmese that don't use spaces between words.
    ///
    /// This method allows loading LSTM models at runtime to enable proper word segmentation for these languages. CJK
    /// text uses dictionary data.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use parley::{LayoutContext, FontContext, SegmenterModelData};
    ///
    /// let mut layout_ctx = LayoutContext::new();
    ///
    /// // Load Thai and Khmer LSTM models from files
    /// let thai_blob = std::fs::read("Thai_codepoints_exclusive_model4_heavy.postcard")?;
    /// let thai_model = SegmenterModelData::from_blob(thai_blob.into_boxed_slice())?;
    ///
    /// let khmer_blob = std::fs::read("Khmer_codepoints_exclusive_model4_heavy.postcard")?;
    /// let khmer_model = SegmenterModelData::from_blob(khmer_blob.into_boxed_slice())?;
    ///
    /// layout_ctx.load_segmenter_models_auto([thai_model, khmer_model]);
    /// ```
    #[cfg(feature = "runtime-segmenter-data")]
    #[cfg_attr(docsrs, doc(cfg(feature = "runtime-segmenter-data")))]
    pub fn load_segmenter_models_auto(
        &mut self,
        models: impl IntoIterator<Item = crate::SegmenterModelData>,
    ) {
        self.analysis_data_sources.load_segmenter_models(
            models.into_iter().map(|model| model.provider).collect(),
            crate::analysis::SegmenterMode::Auto,
        );
    }

    /// Loads runtime dictionary segmenter models for word/line breaking.
    ///
    /// Unlike [`Self::load_segmenter_models_auto`] which uses LSTM for Southeast Asian scripts, this uses dictionary
    /// data for all complex scripts. Dictionaries may be faster at runtime but require more data.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use parley::{LayoutContext, FontContext};
    /// use parley_data::SegmenterModelData;
    ///
    /// let mut layout_ctx = LayoutContext::new();
    ///
    /// // Load Thai dictionary-based segmenter
    /// let thai_blob = std::fs::read("thaidict.postcard")?;
    /// let thai_model = SegmenterModelData::from_blob(thai_blob.into_boxed_slice())?;
    ///
    /// layout_ctx.load_segmenter_models_dictionary([thai_model]);
    /// ```
    #[cfg(feature = "runtime-segmenter-data")]
    #[cfg_attr(docsrs, doc(cfg(feature = "runtime-segmenter-data")))]
    pub fn load_segmenter_models_dictionary(
        &mut self,
        models: impl IntoIterator<Item = crate::SegmenterModelData>,
    ) {
        self.analysis_data_sources.load_segmenter_models(
            models.into_iter().map(|model| model.provider).collect(),
            crate::analysis::SegmenterMode::Dictionary,
        );
    }

    /// Add a runtime segmenter model for improved word/line breaking. See [`Self::load_segmenter_models_auto`] for
    /// more.
    ///
    /// This method does *not* handle duplicates--if you load the same model twice, it will simply attempt to use it
    /// twice during text analysis and make things slower.
    ///
    /// # Panics
    ///
    /// If you previously called [`Self::load_segmenter_models_dictionary`] or
    /// [`Self::append_segmenter_model_dictionary`] to load a dictionary segmenter. Previously-loaded segmenters, if
    /// any, must have been "auto".
    #[cfg(feature = "runtime-segmenter-data")]
    #[cfg_attr(docsrs, doc(cfg(feature = "runtime-segmenter-data")))]
    pub fn append_segmenter_model_auto(&mut self, model: crate::SegmenterModelData) {
        self.analysis_data_sources
            .append_segmenter_model(model.provider, crate::analysis::SegmenterMode::Auto);
    }

    /// Add a runtime segmenter model for improved word/line breaking. See [`Self::load_segmenter_models_dictionary`]
    /// for more.
    ///
    /// This method does *not* handle duplicates--if you load the same model twice, it will simply attempt to use it
    /// twice during text analysis and make things slower.
    ///
    /// # Panics
    ///
    /// If you previously called [`Self::load_segmenter_models_auto`] or [`Self::append_segmenter_model_auto`] to load
    /// an "auto mode" segmenter. Previously-loaded segmenters, if any, must have been "dictionary".
    #[cfg(feature = "runtime-segmenter-data")]
    #[cfg_attr(docsrs, doc(cfg(feature = "runtime-segmenter-data")))]
    pub fn append_segmenter_model_dictionary(&mut self, model: crate::SegmenterModelData) {
        self.analysis_data_sources
            .append_segmenter_model(model.provider, crate::analysis::SegmenterMode::Dictionary);
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

    fn begin(&mut self) {
        self.rcx.clear();
        self.styles.clear();
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
