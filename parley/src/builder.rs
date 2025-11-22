// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Context for layout.

use super::FontContext;
use super::context::LayoutContext;
use super::style::{Brush, StyleProperty, TextStyle, WhiteSpaceCollapse};

use super::layout::Layout;

use alloc::string::String;
use core::ops::RangeBounds;

use crate::inline_box::InlineBox;
use crate::resolve::tree::ItemKind;

/// Builder for constructing a text layout with ranged attributes.
#[must_use]
pub struct RangedBuilder<'a, B: Brush> {
    pub(crate) scale: f32,
    pub(crate) quantize: bool,
    pub(crate) lcx: &'a mut LayoutContext<B>,
    pub(crate) fcx: &'a mut FontContext,
}

impl<B: Brush> RangedBuilder<'_, B> {
    pub fn push_default<'a>(&mut self, property: impl Into<StyleProperty<'a, B>>) {
        let resolved = self
            .lcx
            .rcx
            .resolve_property(self.fcx, &property.into(), self.scale);
        self.lcx.ranged_style_builder.push_default(resolved);
    }

    pub fn push<'a>(
        &mut self,
        property: impl Into<StyleProperty<'a, B>>,
        range: impl RangeBounds<usize>,
    ) {
        let resolved = self
            .lcx
            .rcx
            .resolve_property(self.fcx, &property.into(), self.scale);
        self.lcx.ranged_style_builder.push(resolved, range);
    }

    pub fn push_inline_box(&mut self, inline_box: InlineBox) {
        self.lcx.inline_boxes.push(inline_box);
    }

    pub fn build_into(self, layout: &mut Layout<B>, text: impl AsRef<str>) {
        // Apply RangedStyleBuilder styles to LayoutContext
        self.lcx.ranged_style_builder.finish(&mut self.lcx.styles);

        // Call generic layout builder method
        build_into_layout(
            layout,
            self.scale,
            self.quantize,
            text.as_ref(),
            self.lcx,
            self.fcx,
        );
    }

    pub fn build(self, text: impl AsRef<str>) -> Layout<B> {
        let mut layout = Layout::default();
        self.build_into(&mut layout, text);
        layout
    }
}

/// Builder for constructing a text layout with a tree of attributes.
#[must_use]
pub struct TreeBuilder<'a, B: Brush> {
    pub(crate) scale: f32,
    pub(crate) quantize: bool,
    pub(crate) lcx: &'a mut LayoutContext<B>,
    pub(crate) fcx: &'a mut FontContext,
}

impl<B: Brush> TreeBuilder<'_, B> {
    pub fn push_style_span(&mut self, style: TextStyle<'_, B>) {
        let resolved = self
            .lcx
            .rcx
            .resolve_entire_style_set(self.fcx, &style, self.scale);
        self.lcx.tree_style_builder.push_style_span(resolved);
    }

    pub fn push_style_modification_span<'s, 'iter>(
        &mut self,
        properties: impl IntoIterator<Item = &'iter StyleProperty<'s, B>>,
    ) where
        's: 'iter,
        B: 'iter,
    {
        self.lcx.tree_style_builder.push_style_modification_span(
            properties
                .into_iter()
                .map(|p| self.lcx.rcx.resolve_property(self.fcx, p, self.scale)),
        );
    }

    pub fn pop_style_span(&mut self) {
        self.lcx.tree_style_builder.pop_style_span();
    }

    pub fn push_text(&mut self, text: &str) {
        self.lcx.tree_style_builder.push_text(text);
    }

    pub fn push_inline_box(&mut self, mut inline_box: InlineBox) {
        self.lcx.tree_style_builder.push_uncommitted_text(false);
        self.lcx.tree_style_builder.set_is_span_first(false);
        self.lcx
            .tree_style_builder
            .set_last_item_kind(ItemKind::InlineBox);
        // TODO: arrange type better here to factor out the index
        inline_box.index = self.lcx.tree_style_builder.current_text_len();
        self.lcx.inline_boxes.push(inline_box);
    }

    pub fn set_white_space_mode(&mut self, white_space_collapse: WhiteSpaceCollapse) {
        self.lcx
            .tree_style_builder
            .set_white_space_mode(white_space_collapse);
    }

    #[inline]
    pub fn build_into(self, layout: &mut Layout<B>) -> String {
        // Apply TreeStyleBuilder styles to LayoutContext
        let text = self.lcx.tree_style_builder.finish(&mut self.lcx.styles);

        // Call generic layout builder method
        build_into_layout(layout, self.scale, self.quantize, &text, self.lcx, self.fcx);

        text
    }

    #[inline]
    pub fn build(self) -> (Layout<B>, String) {
        let mut layout = Layout::default();
        let text = self.build_into(&mut layout);
        (layout, text)
    }
}

fn build_into_layout<B: Brush>(
    layout: &mut Layout<B>,
    scale: f32,
    quantize: bool,
    text: &str,
    lcx: &mut LayoutContext<B>,
    fcx: &mut FontContext,
) {
    crate::analysis::analyze_text(lcx, text);

    layout.data.clear();
    layout.data.scale = scale;
    layout.data.quantize = quantize;
    layout.data.base_level = lcx.info.first().map(|i| i.0.bidi_embed_level).unwrap_or(0);
    layout.data.text_len = text.len();

    let mut char_index = 0;
    for (i, style) in lcx.styles.iter().enumerate() {
        for _ in text[style.range.clone()].chars() {
            lcx.info[char_index].1 = i as u16;
            char_index += 1;
        }
    }

    // Copy the visual styles into the layout
    layout
        .data
        .styles
        .extend(lcx.styles.iter().map(|s| s.style.as_layout_style()));

    // Sort the inline boxes as subsequent code assumes that they are in text index order.
    // Note: It's important that this is a stable sort to allow users to control the order of contiguous inline boxes
    lcx.inline_boxes.sort_by_key(|b| b.index);

    {
        let query = fcx.collection.query(&mut fcx.source_cache);
        super::shape::shape_text(
            &lcx.rcx,
            query,
            &lcx.styles,
            &lcx.inline_boxes,
            &lcx.info,
            &mut lcx.scx,
            text,
            layout,
            &lcx.analysis_data_sources,
        );
    }

    // Move inline boxes into the layout
    layout.data.inline_boxes.clear();
    core::mem::swap(&mut layout.data.inline_boxes, &mut lcx.inline_boxes);

    layout.data.finish();
}
