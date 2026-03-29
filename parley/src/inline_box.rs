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
    /// The distance from the top of the box to its internal text baseline, if the
    /// box contains text content. When `Some(baseline)`, the box aligns using this
    /// as its baseline (giving separate "ascent" and "descent" portions). When `None`,
    /// falls back to aligning by the bottom of the box (the entire height is treated
    /// as ascent above the baseline). In Blitz, this is sourced from Taffy's layout output.
    pub first_baseline: Option<f32>,
}
