// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Metrics information for a line.
#[derive(Copy, Clone, Default, Debug)]
pub struct LineMetrics {
    /// Typographic ascent.
    pub ascent: f32,
    /// Typographic descent.
    pub descent: f32,
    /// Typographic leading.
    pub leading: f32,
    /// The absolute line height (in layout units).
    /// It matches the CSS definition of line height where it is derived as a multiple of the font size.
    pub line_height: f32,
    /// Offset to the baseline.
    pub baseline: f32,
    /// Offset for alignment.
    pub offset: f32,
    /// Full advance of the line.
    pub advance: f32,
    /// Advance of trailing whitespace.
    pub trailing_whitespace: f32,
    /// Minimum coordinate in the direction orthogonal to line
    /// direction.
    ///
    /// For horizontal text, this would be the top of the line.
    pub min_coord: f32,
    /// Maximum coordinate in the direction orthogonal to line
    /// direction.
    ///
    /// For horizontal text, this would be the bottom of the line.
    pub max_coord: f32,
}

impl LineMetrics {
    /// Returns the size of the line
    pub fn size(&self) -> f32 {
        self.line_height
    }
}
