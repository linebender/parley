// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Properties for specifying font matching attributes.

use core::fmt;

use parlance::{FontStyle, FontWeight, FontWidth};

/// Primary attributes for font matching: [`FontWidth`], [`FontStyle`] and [`FontWeight`].
///
/// These are used to [configure] a [`Query`].
///
/// [configure]: crate::Query::set_attributes
/// [`Query`]: crate::Query
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct Attributes {
    pub width: FontWidth,
    pub style: FontStyle,
    pub weight: FontWeight,
}

impl Attributes {
    /// Creates new attributes from the given width, style and weight.
    pub fn new(width: FontWidth, style: FontStyle, weight: FontWeight) -> Self {
        Self {
            width,
            style,
            weight,
        }
    }
}

impl fmt::Display for Attributes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "width: {}, style: {}, weight: {}",
            self.width, self.style, self.weight
        )
    }
}

macro_rules! attribute_range {
    ($(#[$meta:meta])* $name:ident, $ty:ident) => {
        $(#[$meta])*
        #[derive(Copy, Clone, PartialEq, Debug)]
        pub struct $name {
            min: $ty,
            max: $ty,
        }

        impl $name {
            /// Creates a range spanning `a` and `b`, in either order.
            pub fn new(a: $ty, b: $ty) -> Self {
                if b < a {
                    Self { min: b, max: a }
                } else {
                    Self { min: a, max: b }
                }
            }

            /// Returns the inclusive lower bound.
            pub fn min(self) -> $ty {
                self.min
            }

            /// Returns the inclusive upper bound.
            pub fn max(self) -> $ty {
                self.max
            }

            /// Returns `true` if `value` is within the range.
            pub fn contains(self, value: $ty) -> bool {
                self.min <= value && value <= self.max
            }

            /// Returns the value in the range closest to `value`.
            pub fn clamp(self, value: $ty) -> $ty {
                if value < self.min {
                    self.min
                } else if value > self.max {
                    self.max
                } else {
                    value
                }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                $ty::default().into()
            }
        }

        impl From<$ty> for $name {
            fn from(value: $ty) -> Self {
                Self {
                    min: value,
                    max: value,
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                if self.min == self.max {
                    write!(f, "{}", self.min)
                } else {
                    write!(f, "{} {}", self.min, self.max)
                }
            }
        }
    };
}

attribute_range!(
    /// Inclusive range of [`FontWeight`]s supported by a font.
    ///
    /// A static font supports a single weight. A variable font supports the
    /// range of its `wght` axis, and a CSS `@font-face` rule can declare a range
    /// with its `font-weight` descriptor:
    /// <https://www.w3.org/TR/css-fonts-4/#font-prop-desc>.
    FontWeightRange,
    FontWeight
);

attribute_range!(
    /// Inclusive range of [`FontWidth`]s supported by a font.
    ///
    /// A static font supports a single width. A variable font supports the
    /// range of its `wdth` axis, and a CSS `@font-face` rule can declare a range
    /// with its `font-width` (`font-stretch`) descriptor:
    /// <https://www.w3.org/TR/css-fonts-4/#font-prop-desc>.
    FontWidthRange,
    FontWidth
);

/// Range of [`FontStyle`]s supported by a font.
///
/// A CSS `@font-face` rule can declare a range of oblique angles with its
/// `font-style` descriptor: <https://www.w3.org/TR/css-fonts-4/#font-prop-desc>.
#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub enum FontStyleRange {
    /// An upright font.
    #[default]
    Normal,
    /// An italic font.
    Italic,
    /// An oblique font covering an inclusive range of angles, in degrees.
    /// Positive angles slant clockwise, as in CSS.
    Oblique {
        /// The smallest supported angle.
        min_angle: f32,
        /// The largest supported angle.
        max_angle: f32,
    },
}

impl FontStyleRange {
    /// The oblique angle used when none is specified, in degrees.
    pub const DEFAULT_OBLIQUE_ANGLE: f32 = 14.0;

    /// Creates an oblique range spanning angles `a` and `b`, in either order.
    pub fn oblique(a: f32, b: f32) -> Self {
        Self::Oblique {
            min_angle: a.min(b),
            max_angle: a.max(b),
        }
    }

    /// Returns the style in this range closest to an oblique angle of `angle`.
    ///
    /// Normal and italic ranges return themselves.
    pub fn clamp_angle(self, angle: f32) -> FontStyle {
        match self {
            Self::Normal => FontStyle::Normal,
            Self::Italic => FontStyle::Italic,
            Self::Oblique {
                min_angle,
                max_angle,
            } => FontStyle::Oblique(Some(angle.clamp(min_angle, max_angle))),
        }
    }
}

impl From<FontStyle> for FontStyleRange {
    fn from(style: FontStyle) -> Self {
        match style {
            FontStyle::Normal => Self::Normal,
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
            Self::Normal => f.write_str("normal"),
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
