// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Minimal navigation scaffold: move left/right across blocks using a flow.

use parley::editing::Cursor;
use parley::layout::Affinity;
use parley::style::Brush;

use crate::{SelectionSet, TextBlock, flow::TextFlow};

fn find_block_index_by_id<B: Brush, S: TextBlock<B>>(blocks: &[S], id: S::Id) -> Option<usize> {
    blocks.iter().position(|s| s.id() == id)
}

/// Move the active caret one cluster to the left, crossing blocks by flow order.
pub fn move_left<B: Brush, S: TextBlock<B>>(
    flow: &TextFlow<S::Id>,
    blocks: &[S],
    set: &mut SelectionSet<S::Id>,
) {
    let Some(mut caret) = set.active else { return };
    let Some(ix) = find_block_index_by_id::<B, S>(blocks, caret.surface) else {
        return;
    };
    let layout = blocks[ix].layout();
    let prev = caret.cursor;
    let next = prev.previous_visual(layout);
    if next != prev {
        caret.cursor = next;
        set.active = Some(caret);
        return;
    }
    let Some(prev_id) = flow.prev_id(caret.surface) else {
        return;
    };
    let Some(prev_ix) = find_block_index_by_id::<B, S>(blocks, prev_id) else {
        return;
    };
    let block = &blocks[prev_ix];
    let end = Cursor::from_byte_index(block.layout(), usize::MAX, Affinity::Upstream);
    caret.surface = block.id();
    caret.cursor = end;
    set.active = Some(caret);
}

/// Move the active caret one cluster to the right, crossing blocks by flow order.
pub fn move_right<B: Brush, S: TextBlock<B>>(
    flow: &TextFlow<S::Id>,
    blocks: &[S],
    set: &mut SelectionSet<S::Id>,
) {
    let Some(mut caret) = set.active else { return };
    let Some(ix) = find_block_index_by_id::<B, S>(blocks, caret.surface) else {
        return;
    };
    let layout = blocks[ix].layout();
    let prev = caret.cursor;
    let next = prev.next_visual(layout);
    if next != prev {
        caret.cursor = next;
        set.active = Some(caret);
        return;
    }
    let Some(next_id) = flow.next_id(caret.surface) else {
        return;
    };
    let Some(next_ix) = find_block_index_by_id::<B, S>(blocks, next_id) else {
        return;
    };
    let block = &blocks[next_ix];
    let start = Cursor::from_byte_index(block.layout(), 0, Affinity::Downstream);
    caret.surface = block.id();
    caret.cursor = start;
    set.active = Some(caret);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Caret, LayoutBlock, SelectionSet};
    use parley::editing::Cursor;
    use parley::{Alignment, AlignmentOptions, FontContext, Layout, LayoutContext, StyleProperty};

    fn build_layout(
        font_cx: &mut FontContext,
        layout_cx: &mut LayoutContext<()>,
        text: &str,
    ) -> Layout<()> {
        let mut builder = layout_cx.ranged_builder(font_cx, text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(14.0));
        let mut layout: Layout<()> = builder.build(text);
        let width = Some(80.0);
        layout.break_all_lines(width);
        layout.align(width, Alignment::Start, AlignmentOptions::default());
        layout
    }

    fn two_blocks_flow() -> (Vec<LayoutBlock<'static, (), u32>>, TextFlow<u32>) {
        let mut font_cx = FontContext::new();
        let mut layout_cx = LayoutContext::new();
        let l1 = build_layout(&mut font_cx, &mut layout_cx, "A");
        let l2 = build_layout(&mut font_cx, &mut layout_cx, "B");
        let l1: &'static Layout<()> = Box::leak(Box::new(l1));
        let l2: &'static Layout<()> = Box::leak(Box::new(l2));
        let blocks = vec![
            LayoutBlock {
                id: 1,
                layout: l1,
                text: "A",
            },
            LayoutBlock {
                id: 2,
                layout: l2,
                text: "B",
            },
        ];
        let flow = TextFlow::new(vec![
            crate::flow::FlowItem::new(
                1,
                parley::BoundingBox::new(0.0, 0.0, 1_000_000.0, l1.height() as f64),
                crate::BoundaryPolicy::Space,
            ),
            crate::flow::FlowItem::new(
                2,
                parley::BoundingBox::new(
                    0.0,
                    (l1.height() + 2.0) as f64,
                    1_000_000.0,
                    (l1.height() + 2.0 + l2.height()) as f64,
                ),
                crate::BoundaryPolicy::Space,
            ),
        ]);
        (blocks, flow)
    }

    #[test]
    fn move_right_crosses_to_next_block_start() {
        let (blocks, flow) = two_blocks_flow();
        let caret = Caret {
            surface: 1_u32,
            cursor: Cursor::from_byte_index(blocks[0].layout, usize::MAX, Affinity::Upstream),
            h_pos: None,
        };
        let mut set = SelectionSet::collapsed(caret);
        move_right::<(), _>(&flow, &blocks, &mut set);
        let active = set.active.expect("active");
        assert_eq!(active.surface, 2);
        let start2 = Cursor::from_byte_index(blocks[1].layout, 0, Affinity::Downstream);
        assert_eq!(active.cursor, start2);
    }

    #[test]
    fn move_left_crosses_to_prev_block_end() {
        let (blocks, flow) = two_blocks_flow();
        let caret = Caret {
            surface: 2_u32,
            cursor: Cursor::from_byte_index(blocks[1].layout, 0, Affinity::Downstream),
            h_pos: None,
        };
        let mut set = SelectionSet::collapsed(caret);
        move_left::<(), _>(&flow, &blocks, &mut set);
        let active = set.active.expect("active");
        assert_eq!(active.surface, 1);
        let end1 = Cursor::from_byte_index(blocks[0].layout, usize::MAX, Affinity::Upstream);
        assert_eq!(active.cursor, end1);
    }
}
