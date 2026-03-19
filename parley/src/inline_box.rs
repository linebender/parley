// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{AlignmentBaseline, BaselineShift, BaselineSource};

/// A box to be laid out inline with text
#[derive(PartialEq, Debug, Clone)]
pub struct InlineBox {
    /// User-specified identifier for the box, which can be used by the user to determine which box in
    /// parley's output corresponds to which box in its input.
    pub id: u64,
    /// The byte offset into the underlying text string at which the box should be placed.
    /// This must not be within a Unicode code point.
    pub index: usize,
    /// The width of the box in pixels
    pub width: f32,
    /// The height of the box in pixels
    pub height: f32,
    /// Which baseline to align to (CSS `alignment-baseline`).
    pub alignment_baseline: AlignmentBaseline,
    /// How much to shift from the alignment baseline (CSS `baseline-shift`).
    pub baseline_shift: BaselineShift,
    /// Which baseline set to use (CSS `baseline-source`).
    pub baseline_source: BaselineSource,
    // TODO: Add `first_baseline: Option<f32>` field. When `Some(baseline)`, the
    // inline box should align using this as its baseline (distance from box top
    // to its internal text baseline). When `None`, falls back to aligning by the
    // bottom of the box (current behavior). In Blitz, this would be sourced from
    // Taffy's layout output.
    // See: https://github.com/linebender/parley/issues/291
}
