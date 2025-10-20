// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Various helper functions to assert truths during testing.

use crate::{Brush, data::LayoutData};

/// Assert that the two provided `LayoutData` are equal.
pub(crate) fn assert_eq_layout_data<B: Brush>(a: &LayoutData<B>, b: &LayoutData<B>, case: &str) {
    assert_eq!(a.scale, b.scale, "{case} scale mismatch");
    assert_eq!(a.quantize, b.quantize, "{case} quantize mismatch");
    assert_eq!(a.base_level, b.base_level, "{case} base_level mismatch");
    assert_eq!(a.text_len, b.text_len, "{case} text_len mismatch");
    assert_eq!(a.width, b.width, "{case} width mismatch");
    assert_eq!(a.full_width, b.full_width, "{case} full_width mismatch");
    assert_eq!(a.height, b.height, "{case} height mismatch");
    assert_eq!(a.fonts, b.fonts, "{case} fonts mismatch");
    assert_eq!(a.coords, b.coords, "{case} coords mismatch");

    // Input (/ output of style resolution)
    assert_eq!(a.styles, b.styles, "{case} styles mismatch");
    assert_eq!(
        a.inline_boxes, b.inline_boxes,
        "{case} inline_boxes mismatch"
    );

    // Output of shaping
    assert_eq!(a.runs, b.runs, "{case} runs mismatch");
    assert_eq!(a.items, b.items, "{case} items mismatch");
    assert_eq!(a.clusters, b.clusters, "{case} clusters mismatch");
    assert_eq!(a.glyphs, b.glyphs, "{case} glyphs mismatch");

    // Output of line breaking
    assert_eq!(a.lines, b.lines, "{case} lines mismatch");
    assert_eq!(a.line_items, b.line_items, "{case} line_items mismatch");

    // Output of alignment
    assert_eq!(
        a.is_aligned_justified, b.is_aligned_justified,
        "{case} is_aligned_justified mismatch"
    );
    assert_eq!(
        a.alignment_width, b.alignment_width,
        "{case} alignment_width mismatch"
    );

    // Also compare the whole struct in case any fields have been added that aren't
    // part of this test yet. If this triggers, add the missing assert to the above set.
    assert_eq!(a, b, "{case} LayoutData mismatch");
}

/// Assert that the two provided `LayoutData` are equal in terms of alignment metrics.
pub(crate) fn assert_eq_layout_data_alignments<B: Brush>(
    a: &LayoutData<B>,
    b: &LayoutData<B>,
    case: &str,
) {
    assert_eq!(
        a.lines.len(),
        b.lines.len(),
        "line count mismatch with {case}"
    );

    for (line_a, line_b) in a.lines.iter().zip(b.lines.iter()) {
        assert_eq!(
            line_a.metrics.offset, line_b.metrics.offset,
            "line offset mismatch with {case}"
        );
    }

    assert_eq!(
        a.clusters.len(),
        b.clusters.len(),
        "cluster count mismatch with {case}"
    );

    for (cluster_a, cluster_b) in a.clusters.iter().zip(b.clusters.iter()) {
        assert_eq!(
            cluster_a.advance, cluster_b.advance,
            "cluster advance mismatch with {case}"
        );
    }
}
