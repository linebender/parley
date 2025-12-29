// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// An absolute computed font weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontWeight(pub f32);

impl FontWeight {
    /// The CSS `normal` weight (typically 400).
    pub const NORMAL: Self = Self(400.0);
    /// The CSS `bold` weight (typically 700).
    pub const BOLD: Self = Self(700.0);

    /// Creates a weight value.
    pub fn new(value: f32) -> Self {
        Self(value)
    }
}

/// A computed font width / stretch value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontWidth(pub f32);

impl FontWidth {
    /// The CSS `normal` width (ratio 1.0).
    pub const NORMAL: Self = Self(1.0);

    /// Creates a width value.
    pub fn new(value: f32) -> Self {
        Self(value)
    }
}
