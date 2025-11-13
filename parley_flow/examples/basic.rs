// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Minimal example showing surfaces, multi-selection geometry, and copy.

use parley::{Alignment, AlignmentOptions, FontContext, Layout, LayoutContext, StyleProperty};
use parley_flow::{
    BoundaryPolicy, LayoutBlock, SelectionSegment, SelectionSet, copy_text, flow::TextFlow,
    hit_test, selection_geometry,
};

fn build_layout(
    font_cx: &mut FontContext,
    layout_cx: &mut LayoutContext<()>,
    text: &str,
) -> Layout<()> {
    let mut builder = layout_cx.ranged_builder(font_cx, text, 1.0, true);
    builder.push_default(StyleProperty::FontSize(16.0));
    let mut layout: Layout<()> = builder.build(text);
    let width = Some(200.0);
    layout.break_all_lines(width);
    layout.align(width, Alignment::Start, AlignmentOptions::default());
    layout
}

fn main() {
    // Build two simple paragraph layouts with Parley
    let mut font_cx = FontContext::new();
    let mut layout_cx = LayoutContext::new();

    let text1 = "Hello world";
    let text2 = "Second line";
    let layout1 = build_layout(&mut font_cx, &mut layout_cx, text1);
    let layout2 = build_layout(&mut font_cx, &mut layout_cx, text2);

    // Wrap them as surfaces with y-offsets stacked vertically
    let surfaces = vec![
        LayoutBlock {
            id: 0_u32,
            layout: &layout1,
            text: text1,
        },
        LayoutBlock {
            id: 1_u32,
            layout: &layout2,
            text: text2,
        },
    ];
    let flow = TextFlow::<u32>::from_vertical_stack::<(), _>(&surfaces, BoundaryPolicy::Newline);

    // Hit-test near the start of the first paragraph to get a caret
    let caret = hit_test::<(), _>(&flow, &surfaces, 2.0, 2.0).expect("caret");
    println!("Caret: {:?}", caret);

    // Build a multi-selection spanning both surfaces
    let mut set = SelectionSet::collapsed(caret);
    set.add_segment(SelectionSegment::new(0, 0..5)); // "Hello"
    set.add_segment(SelectionSegment::new(1, 0..6)); // "Second"

    // Compute geometry (global coordinates)
    let mut rects = Vec::new();
    selection_geometry::<(), _, _>(&flow, &surfaces, &set, |bb, _| rects.push((bb, 0)));
    println!("Selection rects (count={}):", rects.len());
    for (bb, surface_ix) in &rects {
        println!(
            "  surface={} rect=({},{})->({},{}))",
            surface_ix, bb.x0, bb.y0, bb.x1, bb.y1
        );
    }

    // Extract text across surfaces
    let copied = copy_text::<(), _>(&flow, &surfaces, &set);
    println!("Copied text:\n{}", copied);
}
