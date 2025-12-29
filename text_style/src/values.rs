// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// A specified font size.
///
/// Relative sizes like [`FontSize::Em`] are resolved against the **parent** computed font size.
/// See the crate-level docs for details on the resolution model and how CSS keyword sizes like
/// `larger`/`smaller` can be represented: [`crate`].
///
/// See: <https://www.w3.org/TR/css-fonts-4/#font-size-prop>
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum FontSize {
    /// An absolute size in CSS pixels.
    Px(f32),
    /// A size relative to the parent font size.
    Em(f32),
    /// A size relative to the root font size.
    Rem(f32),
}

/// A specified "spacing" value, such as `letter-spacing` or `word-spacing`.
///
/// Relative values like [`Spacing::Em`] are resolved against the computed font size for the style.
/// See the crate-level docs for details: [`crate`].
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Spacing {
    /// An absolute value in CSS pixels.
    Px(f32),
    /// A value relative to the current font size.
    Em(f32),
    /// A value relative to the root font size.
    Rem(f32),
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

/// A specified line height.
///
/// The relationship between line-height, font size, and font metrics is engine-dependent; this
/// is typically resolved by an engine layer (for example `text_style_resolve`) into a computed line
/// height that can be lowered to engine-specific representations. See the crate-level docs for
/// details: [`crate`].
///
/// See: <https://www.w3.org/TR/css-inline-3/#propdef-line-height>
#[derive(Clone, Copy, Debug, PartialEq, Default)]
#[non_exhaustive]
pub enum LineHeight {
    /// `normal`.
    #[default]
    Normal,
    /// A unitless multiplier of the font size (CSS `line-height: <number>`).
    Factor(f32),
    /// An absolute value in CSS pixels.
    Px(f32),
    /// A value relative to the font size.
    Em(f32),
    /// A value relative to the root font size.
    Rem(f32),
}
