// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Context for layout.

use alloc::{vec, vec::Vec};

use self::tree::TreeStyleBuilder;

use super::FontContext;
use super::bidi;
use super::builder::RangedBuilder;
use super::resolve::{RangedStyle, RangedStyleBuilder, ResolveContext, ResolvedStyle, tree};
use super::style::{Brush, TextStyle};

use swash::shape::ShapeContext;
use swash::text::cluster::CharInfo;

use crate::Layout;
use crate::StyleProperty;
use crate::builder::TreeBuilder;
use crate::inline_box::InlineBox;
use crate::resolve::ResolvedProperty;

/// Shared scratch space used when constructing text layouts.
///
/// This type is designed to be a global resource with only one per-application (or per-thread).
pub struct LayoutContext<B: Brush = [u8; 4]> {
    pub(crate) styles: Vec<RangedStyle<B>>,
    pub(crate) inline_boxes: Vec<InlineBox>,

    // Reusable style builders (to amortise allocations)
    pub(crate) ranged_style_builder: RangedStyleBuilder<B>,
    pub(crate) tree_style_builder: TreeStyleBuilder<B>,

    // Internal contexts
    bidi: bidi::BidiResolver,
    rcx: ResolveContext,
    scx: ShapeContext,
    info: Vec<(CharInfo, u16)>,
}

impl<B: Brush> LayoutContext<B> {
    pub fn new() -> Self {
        Self {
            bidi: bidi::BidiResolver::new(),
            rcx: ResolveContext::default(),
            styles: vec![],
            inline_boxes: vec![],
            ranged_style_builder: RangedStyleBuilder::default(),
            tree_style_builder: TreeStyleBuilder::default(),
            info: vec![],
            scx: ShapeContext::default(),
        }
    }

    fn resolve_style_set(
        &mut self,
        font_ctx: &mut FontContext,
        scale: f32,
        raw_style: &TextStyle<'_, B>,
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
        root_style: &TextStyle<'_, B>,
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

    pub(crate) fn analyze_text(&mut self, text: &str) {
        let text = if text.is_empty() { " " } else { text };
        let mut a = swash::text::analyze(text.chars());

        let mut word_break = Default::default();
        let mut style_idx = 0;

        let mut char_indices = text.char_indices();
        loop {
            let Some((char_idx, _)) = char_indices.next() else {
                break;
            };

            // Find the style for this character. If the text is empty, we may not have any styles. Otherwise,
            // self.styles should span the entire range of the text.
            while let Some(style) = self.styles.get(style_idx) {
                if style.range.end > char_idx {
                    word_break = style.style.word_break;
                    break;
                }
                style_idx += 1;
            }
            a.set_break_strength(word_break);

            let Some((properties, boundary)) = a.next() else {
                break;
            };

            self.info.push((CharInfo::new(properties, boundary), 0));
        }
        if a.needs_bidi_resolution() {
            self.bidi.resolve(
                text.chars()
                    .zip(self.info.iter().map(|info| info.0.bidi_class())),
                None,
            );
        }
    }

    pub(crate) fn resolve_property(
        &mut self,
        fcx: &mut FontContext,
        property: &StyleProperty<'_, B>,
        scale: f32,
    ) -> ResolvedProperty<B> {
        self.rcx.resolve_property(fcx, property, scale)
    }

    pub(crate) fn resolve_entire_style_set(
        &mut self,
        fcx: &mut FontContext,
        raw_style: &TextStyle<'_, B>,
        scale: f32,
    ) -> ResolvedStyle<B> {
        self.rcx.resolve_entire_style_set(fcx, raw_style, scale)
    }

    pub(crate) fn shape_into_layout(
        &mut self,
        fcx: &mut FontContext,
        text: &str,
        layout: &mut Layout<B>,
    ) {
        let query = fcx.collection.query(&mut fcx.source_cache);
        super::shape::shape_text(
            &self.rcx,
            query,
            &self.styles,
            &self.inline_boxes,
            &self.info,
            self.bidi.levels(),
            &mut self.scx,
            text,
            layout,
        );
    }

    pub(crate) fn build_into_layout(
        &mut self,
        layout: &mut Layout<B>,
        scale: f32,
        quantize: bool,
        text: &str,
        fcx: &mut FontContext,
    ) {
        self.analyze_text(text);

        layout.data.clear();
        layout.data.scale = scale;
        layout.data.quantize = quantize;
        layout.data.has_bidi = !self.bidi.levels().is_empty();
        layout.data.base_level = self.bidi.base_level();
        layout.data.text_len = text.len();

        let mut char_index = 0;
        for (i, style) in self.styles.iter().enumerate() {
            for _ in text[style.range.clone()].chars() {
                self.info[char_index].1 = i as u16;
                char_index += 1;
            }
        }

        // Copy the visual styles into the layout
        layout
            .data
            .styles
            .extend(self.styles.iter().map(|s| s.style.as_layout_style()));

        // Sort the inline boxes as subsequent code assumes that they are in text index order.
        // Note: It's important that this is a stable sort to allow users to control the order of contiguous inline boxes
        self.inline_boxes.sort_by_key(|b| b.index);

        self.shape_into_layout(fcx, text, layout);

        // Move inline boxes into the layout
        layout.data.inline_boxes.clear();
        core::mem::swap(&mut layout.data.inline_boxes, &mut self.inline_boxes);

        layout.data.finish();
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
