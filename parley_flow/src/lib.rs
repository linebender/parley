// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `parley_flow`: text blocks, flow, and multi-selection on top of Parley.
//!
//! This crate adds a small, reusable layer over Parley’s per-paragraph [`parley::layout::Layout`]:
//! - [`TextBlock`]: a uniform facade for blocks of text (labels,
//!   paragraphs, document fragments) that expose layout and text access.
//! - [`LayoutBlock`]: a thin adapter that turns a [`parley::layout::Layout`] and its source `&str`
//!   into a [`TextBlock`].
//! - [`flow::TextFlow`]: an explicit list of containers (order + rect + separator) for deterministic
//!   hit-testing, navigation, and text concatenation.
//! - Multi-selection types: [`Caret`], [`SelectionSegment`], and [`SelectionSet`].
//!
//! Design background and platform comparisons are collected in [`design`]; crate docs focus on API.
//!
//! Quick usage outline:
//! - Build your paragraphs with Parley as usual to get a [`parley::layout::Layout`].
//! - Wrap each paragraph as a [`LayoutBlock`], providing a stable `id` and `y_offset`.
//! - Build a [`flow::TextFlow`] with one [`flow::FlowItem`] per block (rect + join policy).
//! - Hit-test with [`hit_test`] to obtain a [`Caret`], then create a
//!   [`SelectionSet::collapsed`] around it or add [`SelectionSegment`]s directly.
//! - Render selection boxes with [`selection_geometry`] and extract text with
//!   [`copy_text`].
//!
//! ## Flow and Ordering
//!
//! This crate uses an explicit flow (inspired by TextKit’s container array):
//! - Hit-testing consults the flow’s container rects directly; no heuristics.
//! - Navigation order across blocks follows the flow’s item order.
//! - Text concatenation between adjacent blocks uses each item’s `join` policy.
//!   Use [`flow::TextFlow::from_vertical_stack`] for a uniform separator, or assign `join`
//!   per item when you need mixed separators (inline vs block) in the same flow.
//!
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::String;

use parley::BoundingBox;
use parley::editing::{Cursor, Selection};
use parley::style::Brush;

mod block;
pub use block::{LayoutBlock, TextBlock};

mod multi_selection;
pub use multi_selection::{BoundaryPolicy, Caret, SelectionSegment, SelectionSet};

/// Design notes and platform comparisons.
pub mod design;

/// Explicit flow of text blocks (containers).
pub mod flow;
/// Navigation helpers for moving the active caret across blocks.
pub mod navigation;

// Flow-based hit testing and geometry utilities are below.

/// Hit-test with a [`flow::TextFlow`], returning a [`Caret`] at the corresponding block.
#[allow(
    clippy::cast_possible_truncation,
    reason = "Layout coordinates are f32; flow rects are f64; truncation is acceptable when mapping to local layout space."
)]
pub fn hit_test<'a, B, S>(
    flow: &flow::TextFlow<S::Id>,
    blocks: &'a [S],
    x: f32,
    y: f32,
) -> Option<Caret<S::Id>>
where
    B: Brush,
    S: TextBlock<B> + 'a,
{
    let id = flow.hit_test(x, y)?;
    let item = flow.items().iter().find(|it| it.id == id)?;
    let block = blocks.iter().find(|b| b.id() == id)?;
    // Map to local coordinates using flow rect
    let local_x = x - item.rect.x0 as f32;
    let local_y = y - item.rect.y0 as f32;
    let cursor = Cursor::from_point(block.layout(), local_x, local_y);
    Some(Caret {
        surface: id,
        cursor,
        h_pos: None,
    })
}

/// Compute selection rectangles across blocks with a [`flow::TextFlow`].
///
/// Each rectangle is offset by the corresponding flow item's rect (x and y) to global space.
pub fn selection_geometry<B, S, F>(
    flow: &flow::TextFlow<S::Id>,
    blocks: &[S],
    set: &SelectionSet<S::Id>,
    mut f: F,
) where
    B: Brush,
    S: TextBlock<B>,
    F: FnMut(BoundingBox, S::Id),
{
    for seg in &set.segments {
        let Some(item) = flow.items().iter().find(|it| it.id == seg.surface) else {
            continue;
        };
        let Some(block) = blocks.iter().find(|b| b.id() == seg.surface) else {
            continue;
        };
        let sel = Selection::new(
            Cursor::from_byte_index::<B>(block.layout(), seg.range.start, seg.anchor_affinity),
            Cursor::from_byte_index::<B>(block.layout(), seg.range.end, seg.focus_affinity),
        );
        sel.geometry_with::<B>(block.layout(), |bb, _line| {
            let g = BoundingBox::new(
                bb.x0 + item.rect.x0,
                bb.y0 + item.rect.y0,
                bb.x1 + item.rect.x0,
                bb.y1 + item.rect.y0,
            );
            f(g, seg.surface);
        });
    }
}

/// Extract selected text across blocks using a [`flow::TextFlow`].
pub fn copy_text<B, S>(
    flow: &flow::TextFlow<S::Id>,
    blocks: &[S],
    set: &SelectionSet<S::Id>,
) -> String
where
    B: Brush,
    S: TextBlock<B>,
{
    let mut out = String::new();
    let mut prev_id: Option<S::Id> = None;
    for seg in &set.segments {
        if let Some(pid) = prev_id.filter(|p| *p != seg.surface) {
            match flow.join_after(pid) {
                BoundaryPolicy::None => {}
                BoundaryPolicy::Space => out.push(' '),
                BoundaryPolicy::Newline => out.push('\n'),
            }
        }
        if let Some(block) = blocks.iter().find(|b| b.id() == seg.surface) {
            if !block.read_text(seg.range.clone(), &mut out) {
                if let Some(slice) = block.text_slice(seg.range.clone()) {
                    out.push_str(slice);
                }
            }
        }
        prev_id = Some(seg.surface);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use parley::{Alignment, AlignmentOptions, FontContext, Layout, LayoutContext, StyleProperty};

    fn build_layout(
        font_cx: &mut FontContext,
        layout_cx: &mut LayoutContext<()>,
        text: &str,
    ) -> Layout<()> {
        let mut builder = layout_cx.ranged_builder(font_cx, text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(14.0));
        let mut layout: Layout<()> = builder.build(text);
        let width = Some(200.0);
        layout.break_all_lines(width);
        layout.align(width, Alignment::Start, AlignmentOptions::default());
        layout
    }

    #[test]
    fn hit_test_and_copy_text() {
        let mut font_cx = FontContext::new();
        let mut layout_cx = LayoutContext::new();
        let text1 = "Hello";
        let text2 = "World";
        let l1 = build_layout(&mut font_cx, &mut layout_cx, text1);
        let l2 = build_layout(&mut font_cx, &mut layout_cx, text2);
        let l1: &'static Layout<()> = Box::leak(Box::new(l1));
        let l2: &'static Layout<()> = Box::leak(Box::new(l2));
        let blocks = vec![
            LayoutBlock {
                id: 1_u32,
                layout: l1,
                text: text1,
            },
            LayoutBlock {
                id: 2_u32,
                layout: l2,
                text: text2,
            },
        ];
        // Build explicit rects to ensure reliable hit-testing
        let flow = flow::TextFlow::new(vec![
            flow::FlowItem::new(
                1,
                BoundingBox::new(0.0, 0.0, 1_000_000.0, (l1.height() + 2.0) as f64),
                BoundaryPolicy::Space,
            ),
            flow::FlowItem::new(
                2,
                BoundingBox::new(
                    0.0,
                    (l1.height() + 4.0) as f64,
                    1_000_000.0,
                    (l1.height() + 4.0 + l2.height()) as f64,
                ),
                BoundaryPolicy::Space,
            ),
        ]);
        // Hit-test in the first block
        let caret = hit_test::<(), _>(&flow, &blocks, 1.0, 0.1).expect("hit");
        assert_eq!(caret.surface, 1);
        // Copy text across blocks
        let mut set = SelectionSet::default();
        set.add_segment(SelectionSegment::new(1, 0..text1.len()));
        set.add_segment(SelectionSegment::new(2, 0..text2.len()));
        let copied = copy_text::<(), _>(&flow, &blocks, &set);
        assert_eq!(copied, "Hello World");
    }
}
