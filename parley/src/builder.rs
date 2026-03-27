// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Context for layout.

use super::FontContext;
use super::context::LayoutContext;
use super::style::{Brush, StyleProperty, TextStyle, WhiteSpaceCollapse};

use super::layout::Layout;

use alloc::string::String;
use core::ops::RangeBounds;

use crate::InlineBoxKind;
use crate::resolve::tree::ItemKind;
use parley_core::InlineBox;

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
            .resolve_style_property(self.fcx, self.scale, &property.into());
        self.lcx.ranged_style_builder.push_default(resolved);
    }

    pub fn push<'a>(
        &mut self,
        property: impl Into<StyleProperty<'a, B>>,
        range: impl RangeBounds<usize>,
    ) {
        let resolved = self
            .lcx
            .resolve_style_property(self.fcx, self.scale, &property.into());
        self.lcx.ranged_style_builder.push(resolved, range);
    }

    pub fn push_inline_box(&mut self, inline_box: InlineBox) {
        self.lcx.core_ctx.push_inline_box(inline_box);
    }

    pub fn build_into(self, layout: &mut Layout<B>, text: impl AsRef<str>) {
        // Apply RangedStyleBuilder styles directly to style-table/style-run state.
        self.lcx.ranged_style_builder.finish(&mut self.lcx.core_ctx);

        // Call generic layout builder method
        self.lcx.build_into(
            &mut layout.data,
            self.scale,
            self.quantize,
            text.as_ref(),
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
    pub fn push_style_span(&mut self, style: TextStyle<'_, '_, B>) {
        let resolved = self.lcx.resolve_style_set(self.fcx, self.scale, &style);
        self.lcx.tree_style_builder.push_style_span(resolved);
    }

    pub fn push_style_modification_span<'s, 'iter>(
        &mut self,
        properties: impl IntoIterator<Item = &'iter StyleProperty<'s, B>>,
    ) where
        's: 'iter,
        B: 'iter,
    {
        self.lcx
            .tree_style_builder
            .push_style_modification_span(properties.into_iter().map(|p| {
                self.lcx
                    .core_ctx
                    .resolve_style_property(self.fcx, self.scale, p)
            }));
    }

    pub fn pop_style_span(&mut self) {
        self.lcx.tree_style_builder.pop_style_span();
    }

    pub fn push_text(&mut self, text: &str) {
        self.lcx.tree_style_builder.push_text(text);
    }

    pub fn push_inline_box(&mut self, mut inline_box: InlineBox) {
        if inline_box.kind == InlineBoxKind::InFlow {
            self.lcx.tree_style_builder.push_uncommitted_text(false);
            self.lcx.tree_style_builder.set_is_span_first(false);
            self.lcx
                .tree_style_builder
                .set_last_item_kind(ItemKind::InlineBox);
        }

        // TODO: arrange type better here to factor out the index
        inline_box.index = self.lcx.tree_style_builder.current_text_len();
        self.lcx.core_ctx.push_inline_box(inline_box);
    }

    pub fn set_white_space_mode(&mut self, white_space_collapse: WhiteSpaceCollapse) {
        self.lcx
            .tree_style_builder
            .set_white_space_mode(white_space_collapse);
    }

    #[inline]
    pub fn build_into(self, layout: &mut Layout<B>) -> String {
        // Apply TreeStyleBuilder styles to LayoutContext.
        let text = self.lcx.tree_style_builder.finish(&mut self.lcx.core_ctx);

        // Call generic layout builder method
        self.lcx
            .build_into(&mut layout.data, self.scale, self.quantize, &text, self.fcx);

        text
    }

    #[inline]
    pub fn build(self) -> (Layout<B>, String) {
        let mut layout = Layout::default();
        let text = self.build_into(&mut layout);
        (layout, text)
    }
}
