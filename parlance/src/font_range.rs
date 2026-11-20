// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Ranges of font attributes, as declared by variable fonts and CSS `@font-face` rules.

use core::fmt;

use crate::{FontStyle, FontWeight, FontWidth};

/// Inclusive range of font attribute values supported by a font, such as
/// weights, widths or oblique angles.
///
/// A static font supports a single value. A variable font supports the range
/// of the matching variation axis, and a CSS `@font-face` rule can declare a
/// range with its `font-weight`, `font-width` (`font-stretch`) or `font-style`
/// descriptor: <https://www.w3.org/TR/css-fonts-4/#font-prop-desc>.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct FontRange<T> {
    min: T,
    max: T,
}

/// Inclusive range of [`FontWeight`]s supported by a font.
pub type FontWeightRange = FontRange<FontWeight>;

/// Inclusive range of [`FontWidth`]s supported by a font.
pub type FontWidthRange = FontRange<FontWidth>;

impl<T: Copy + PartialOrd> FontRange<T> {
    /// Creates a range spanning `a` and `b`, in either order.
    pub fn new(a: T, b: T) -> Self {
        if b < a {
            Self { min: b, max: a }
        } else {
            Self { min: a, max: b }
        }
    }

    /// Returns the inclusive lower bound.
    pub fn min(self) -> T {
        self.min
    }

    /// Returns the inclusive upper bound.
    pub fn max(self) -> T {
        self.max
    }

    /// Returns `true` if `value` is within the range.
    pub fn contains(self, value: T) -> bool {
        self.min <= value && value <= self.max
    }

    /// Returns the value in the range closest to `value`.
    pub fn clamp(self, value: T) -> T {
        if value < self.min {
            self.min
        } else if value > self.max {
            self.max
        } else {
            value
        }
    }
}

impl<T: Copy + Default> Default for FontRange<T> {
    fn default() -> Self {
        T::default().into()
    }
}

impl<T: Copy> From<T> for FontRange<T> {
    fn from(value: T) -> Self {
        Self {
            min: value,
            max: value,
        }
    }
}

impl<T: fmt::Display + PartialEq> fmt::Display for FontRange<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.min == self.max {
            write!(f, "{}", self.min)
        } else {
            write!(f, "{} {}", self.min, self.max)
        }
    }
}

/// Range of [`FontStyle`]s supported by a font.
///
/// A CSS `@font-face` rule can declare a range of oblique angles with its
/// `font-style` descriptor: <https://www.w3.org/TR/css-fonts-4/#font-prop-desc>.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum FontStyleRange {
    /// An italic font.
    Italic,
    /// An upright or oblique font covering an inclusive range of angles, in
    /// degrees. Positive angles slant clockwise, as in CSS. An upright font is
    /// `0deg 0deg` ([`FontStyleRange::NORMAL`]).
    Oblique(FontRange<f32>),
}

impl FontStyleRange {
    /// An upright font, equivalent to CSS `oblique 0deg`.
    pub const NORMAL: Self = Self::Oblique(FontRange { min: 0.0, max: 0.0 });

    /// The oblique angle used when none is specified, in degrees.
    pub const DEFAULT_OBLIQUE_ANGLE: f32 = 14.0;

    /// Creates an oblique range spanning angles `a` and `b`, in either order.
    pub fn oblique(a: f32, b: f32) -> Self {
        Self::Oblique(FontRange::new(a, b))
    }

    /// Returns `true` if this is [`FontStyleRange::NORMAL`].
    pub fn is_normal(self) -> bool {
        self == Self::NORMAL
    }

    /// Returns the style in this range closest to an oblique angle of `angle`.
    ///
    /// Italic ranges return italic, and an angle of zero returns normal.
    pub fn clamp_angle(self, angle: f32) -> FontStyle {
        match self {
            Self::Italic => FontStyle::Italic,
            Self::Oblique(angles) => match angles.clamp(angle) {
                0.0 => FontStyle::Normal,
                angle => FontStyle::Oblique(Some(angle)),
            },
        }
    }
}

impl Default for FontStyleRange {
    fn default() -> Self {
        Self::NORMAL
    }
}

impl From<FontStyle> for FontStyleRange {
    fn from(style: FontStyle) -> Self {
        match style {
            FontStyle::Normal => Self::NORMAL,
            FontStyle::Italic => Self::Italic,
            FontStyle::Oblique(angle) => {
                Self::Oblique(angle.unwrap_or(Self::DEFAULT_OBLIQUE_ANGLE).into())
            }
        }
    }
}

impl fmt::Display for FontStyleRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            _ if self.is_normal() => f.write_str("normal"),
            Self::Italic => f.write_str("italic"),
            Self::Oblique(angles) if angles.min == angles.max => {
                write!(f, "oblique {}deg", angles.min)
            }
            Self::Oblique(angles) => write!(f, "oblique {}deg {}deg", angles.min, angles.max),
        }
    }
}
