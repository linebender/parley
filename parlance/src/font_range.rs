// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Ranges of font attributes, as declared by variable fonts and CSS `@font-face` rules.

use core::fmt;

use crate::{FontStyle, FontWeight, FontWidth};

/// Inclusive range of [`FontWeight`]s supported by a font.
///
/// A static font supports a single weight. A variable font supports the
/// range of its `wght` axis, and a CSS `@font-face` rule can declare a range
/// with its `font-weight` descriptor:
/// <https://www.w3.org/TR/css-fonts-4/#font-prop-desc>.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct FontWeightRange {
    min: FontWeight,
    max: FontWeight,
}

impl FontWeightRange {
    /// Creates a range spanning `a` and `b`, in either order.
    pub fn new(a: FontWeight, b: FontWeight) -> Self {
        if b < a {
            Self { min: b, max: a }
        } else {
            Self { min: a, max: b }
        }
    }

    /// Returns the inclusive lower bound.
    pub fn min(self) -> FontWeight {
        self.min
    }

    /// Returns the inclusive upper bound.
    pub fn max(self) -> FontWeight {
        self.max
    }

    /// Returns `true` if `value` is within the range.
    pub fn contains(self, value: FontWeight) -> bool {
        self.min <= value && value <= self.max
    }

    /// Returns the value in the range closest to `value`.
    pub fn clamp(self, value: FontWeight) -> FontWeight {
        if value < self.min {
            self.min
        } else if value > self.max {
            self.max
        } else {
            value
        }
    }
}

impl Default for FontWeightRange {
    fn default() -> Self {
        FontWeight::default().into()
    }
}

impl From<FontWeight> for FontWeightRange {
    fn from(value: FontWeight) -> Self {
        Self {
            min: value,
            max: value,
        }
    }
}

impl fmt::Display for FontWeightRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.min == self.max {
            write!(f, "{}", self.min)
        } else {
            write!(f, "{} {}", self.min, self.max)
        }
    }
}

/// Inclusive range of [`FontWidth`]s supported by a font.
///
/// A static font supports a single width. A variable font supports the
/// range of its `wdth` axis, and a CSS `@font-face` rule can declare a range
/// with its `font-width` (`font-stretch`) descriptor:
/// <https://www.w3.org/TR/css-fonts-4/#font-prop-desc>.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct FontWidthRange {
    min: FontWidth,
    max: FontWidth,
}

impl FontWidthRange {
    /// Creates a range spanning `a` and `b`, in either order.
    pub fn new(a: FontWidth, b: FontWidth) -> Self {
        if b < a {
            Self { min: b, max: a }
        } else {
            Self { min: a, max: b }
        }
    }

    /// Returns the inclusive lower bound.
    pub fn min(self) -> FontWidth {
        self.min
    }

    /// Returns the inclusive upper bound.
    pub fn max(self) -> FontWidth {
        self.max
    }

    /// Returns `true` if `value` is within the range.
    pub fn contains(self, value: FontWidth) -> bool {
        self.min <= value && value <= self.max
    }

    /// Returns the value in the range closest to `value`.
    pub fn clamp(self, value: FontWidth) -> FontWidth {
        if value < self.min {
            self.min
        } else if value > self.max {
            self.max
        } else {
            value
        }
    }
}

impl Default for FontWidthRange {
    fn default() -> Self {
        FontWidth::default().into()
    }
}

impl From<FontWidth> for FontWidthRange {
    fn from(value: FontWidth) -> Self {
        Self {
            min: value,
            max: value,
        }
    }
}

impl fmt::Display for FontWidthRange {
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
    Oblique {
        /// The smallest supported angle.
        min_angle: f32,
        /// The largest supported angle.
        max_angle: f32,
    },
}

impl FontStyleRange {
    /// An upright font, equivalent to CSS `oblique 0deg`.
    pub const NORMAL: Self = Self::Oblique {
        min_angle: 0.0,
        max_angle: 0.0,
    };

    /// The oblique angle used when none is specified, in degrees.
    pub const DEFAULT_OBLIQUE_ANGLE: f32 = 14.0;

    /// Creates an oblique range spanning angles `a` and `b`, in either order.
    pub fn oblique(a: f32, b: f32) -> Self {
        Self::Oblique {
            min_angle: a.min(b),
            max_angle: a.max(b),
        }
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
            Self::Oblique {
                min_angle,
                max_angle,
            } => match angle.clamp(min_angle, max_angle) {
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
                let angle = angle.unwrap_or(Self::DEFAULT_OBLIQUE_ANGLE);
                Self::oblique(angle, angle)
            }
        }
    }
}

impl fmt::Display for FontStyleRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            _ if self.is_normal() => f.write_str("normal"),
            Self::Italic => f.write_str("italic"),
            Self::Oblique {
                min_angle,
                max_angle,
            } if min_angle == max_angle => write!(f, "oblique {min_angle}deg"),
            Self::Oblique {
                min_angle,
                max_angle,
            } => write!(f, "oblique {min_angle}deg {max_angle}deg"),
        }
    }
}
