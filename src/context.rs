// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Context for layout.

use super::bidi;
use super::layout::Layout;
use super::resolve::range::*;
use super::resolve::*;
use super::style::*;
use super::FontContext;

use swash::shape::ShapeContext;
use swash::text::cluster::CharInfo;

use std::ops::RangeBounds;

/// Context for building a text layout.
pub struct LayoutContext<B: Brush = [u8; 4]> {
    bidi: bidi::BidiResolver,
    rcx: ResolveContext,
    styles: Vec<RangedStyle<B>>,
    rsb: RangedStyleBuilder<B>,
    info: Vec<(CharInfo, u16)>,
    scx: ShapeContext,
}

impl<B: Brush> LayoutContext<B> {
    pub fn new() -> Self {
        Self {
            bidi: bidi::BidiResolver::new(),
            rcx: ResolveContext::default(),
            styles: vec![],
            rsb: RangedStyleBuilder::default(),
            info: vec![],
            scx: ShapeContext::default(),
        }
    }

    pub fn ranged_builder<'a>(
        &'a mut self,
        fcx: &'a mut FontContext,
        text: &'a str,
        scale: f32,
    ) -> RangedBuilder<B, &'a str> {
        self.begin(text);
        fcx.source_cache.prune(128, false);
        RangedBuilder {
            text,
            scale,
            lcx: self,
            fcx,
        }
    }

    fn begin(&mut self, text: &str) {
        self.rcx.clear();
        self.styles.clear();
        self.rsb.begin(text.len());
        self.info.clear();
        self.bidi.clear();
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

/// Builder for constructing a text layout with ranged attributes.
pub struct RangedBuilder<'a, B: Brush, T: TextSource> {
    text: T,
    scale: f32,
    lcx: &'a mut LayoutContext<B>,
    fcx: &'a mut FontContext,
}

impl<'a, B: Brush, T: TextSource> RangedBuilder<'a, B, T> {
    pub fn push_default(&mut self, property: &StyleProperty<B>) {
        let resolved = self.lcx.rcx.resolve(self.fcx, &property, self.scale);
        self.lcx.rsb.push_default(resolved);
    }

    pub fn push(&mut self, property: &StyleProperty<B>, range: impl RangeBounds<usize>) {
        let resolved = self.lcx.rcx.resolve(&mut self.fcx, &property, self.scale);
        self.lcx.rsb.push(resolved, range);
    }

    pub fn build_into(&mut self, layout: &mut Layout<B>) {
        layout.data.clear();
        layout.data.scale = self.scale;
        let lcx = &mut self.lcx;
        let mut text = self.text.as_str();
        let is_empty = text.is_empty();
        if is_empty {
            // Force a layout to have at least one line.
            text = " ";
        }
        layout.data.has_bidi = !lcx.bidi.levels().is_empty();
        layout.data.base_level = lcx.bidi.base_level();
        layout.data.text_len = text.len();
        let fcx = &mut self.fcx;
        lcx.rsb.finish(&mut lcx.styles);
        let mut char_index = 0;
        for (i, style) in lcx.styles.iter().enumerate() {
            for _ in text[style.range.clone()].chars() {
                lcx.info[char_index].1 = i as u16;
                char_index += 1;
            }
        }
        use super::layout::{Decoration, Style};
        fn conv_deco<B: Brush>(
            deco: &ResolvedDecoration<B>,
            default_brush: &B,
        ) -> Option<Decoration<B>> {
            if deco.enabled {
                Some(Decoration {
                    brush: deco.brush.clone().unwrap_or_else(|| default_brush.clone()),
                    offset: deco.offset,
                    size: deco.size,
                })
            } else {
                None
            }
        }
        layout.data.styles.extend(lcx.styles.iter().map(|s| {
            let s = &s.style;
            Style {
                brush: s.brush.clone(),
                underline: conv_deco(&s.underline, &s.brush),
                strikethrough: conv_deco(&s.strikethrough, &s.brush),
                line_height: s.line_height,
            }
        }));
        {
            let query = fcx.collection.query(&mut fcx.source_cache);
            super::shape::shape_text(
                &lcx.rcx,
                query,
                &lcx.styles,
                &lcx.info,
                lcx.bidi.levels(),
                &mut lcx.scx,
                text,
                layout,
            );
        }
        layout.data.finish();
        if is_empty {
            layout.data.text_len = 0;
            let run = &mut layout.data.runs[0];
            run.cluster_range.end = 0;
            run.text_range.end = 0;
            layout.data.clusters.clear();
        }
    }

    pub fn build(&mut self) -> Layout<B> {
        let mut layout = Layout::default();
        self.build_into(&mut layout);
        layout
    }
}

#[doc(hidden)]
pub trait TextSource {
    fn as_str(&self) -> &str;
}

impl<'a> TextSource for &'a str {
    fn as_str(&self) -> &str {
        *self
    }
}
