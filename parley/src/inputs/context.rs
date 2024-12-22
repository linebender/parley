// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Context for layout.

use alloc::{vec, vec::Vec};

use crate::algos::bidi;
use crate::algos::resolve::tree::TreeStyleBuilder;
use crate::algos::resolve::{RangedStyle, RangedStyleBuilder, ResolveContext, ResolvedStyle};
use crate::inputs::FontContext;
use crate::inputs::RangedBuilder;
use crate::inputs::{Brush, TextStyle};

use swash::shape::ShapeContext;
use swash::text::cluster::CharInfo;

use crate::inline_box::InlineBox;
use crate::inputs::TreeBuilder;

/// Shared scratch space used when constructing text layouts.
///
/// This type is designed to be a global resource with only one per-application (or per-thread).
pub struct LayoutContext<B: Brush = [u8; 4]> {
    pub(crate) bidi: bidi::BidiResolver,
    pub(crate) rcx: ResolveContext,
    pub(crate) styles: Vec<RangedStyle<B>>,
    pub(crate) inline_boxes: Vec<InlineBox>,

    // Reusable style builders (to amortise allocations)
    pub(crate) ranged_style_builder: RangedStyleBuilder<B>,
    pub(crate) tree_style_builder: TreeStyleBuilder<B>,

    pub(crate) info: Vec<(CharInfo, u16)>,
    pub(crate) scx: ShapeContext,
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

    pub fn ranged_builder<'a>(
        &'a mut self,
        fcx: &'a mut FontContext,
        text: &'a str,
        scale: f32,
    ) -> RangedBuilder<'a, B> {
        self.begin();
        self.analyze_text(text);
        self.ranged_style_builder.begin(text.len());

        fcx.source_cache.prune(128, false);

        RangedBuilder {
            scale,
            lcx: self,
            fcx,
        }
    }

    pub fn tree_builder<'a>(
        &'a mut self,
        fcx: &'a mut FontContext,
        scale: f32,
        raw_style: &TextStyle<'_, B>,
    ) -> TreeBuilder<'a, B> {
        self.begin();

        let resolved_root_style = self.resolve_style_set(fcx, scale, raw_style);
        self.tree_style_builder.begin(resolved_root_style);

        fcx.source_cache.prune(128, false);

        TreeBuilder {
            scale,
            lcx: self,
            fcx,
        }
    }

    pub(crate) fn analyze_text(&mut self, text: &str) {
        let text = if text.is_empty() { " " } else { text };
        let mut a = swash::text::analyze(text.chars());
        for x in a.by_ref() {
            self.info.push((CharInfo::new(x.0, x.1), 0));
        }
        if a.needs_bidi_resolution() {
            self.bidi.resolve(
                text.chars()
                    .zip(self.info.iter().map(|info| info.0.bidi_class())),
                Some(0),
            );
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
