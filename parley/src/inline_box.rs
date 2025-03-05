// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{BaselineShift, VerticalAlign};

/// A box to be laid out inline with text
#[derive(Debug, Clone)]
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
    /// The baseline along which this item is aligned.
    pub vertical_align: VerticalAlign,
    /// Additional baseline alignment applied afterwards.
    pub baseline_shift: BaselineShift,
}

impl InlineBox {
    pub fn new(id: u64, index: usize, width: f32, height: f32) -> Self {
        Self {
            id,
            index,
            width,
            height,
            vertical_align: Default::default(),
            baseline_shift: Default::default(),
        }
    }
}
