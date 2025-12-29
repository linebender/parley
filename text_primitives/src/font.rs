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

/// A specified font style.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
#[non_exhaustive]
pub enum FontStyle {
    /// `normal`.
    #[default]
    Normal,
    /// `italic`.
    Italic,
    /// `oblique` with an optional angle in degrees.
    ///
    /// If `None`, the engine-specific default oblique angle is used.
    Oblique(Option<f32>),
}

/// Generic font families, named after CSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GenericFamily {
    /// The `serif` generic family.
    Serif,
    /// The `sans-serif` generic family.
    SansSerif,
    /// The `monospace` generic family.
    Monospace,
    /// The `cursive` generic family.
    Cursive,
    /// The `fantasy` generic family.
    Fantasy,
    /// The `system-ui` generic family.
    SystemUi,
    /// The `emoji` generic family.
    Emoji,
    /// The `math` generic family.
    Math,
    /// The `fangsong` generic family.
    Fangsong,
}
