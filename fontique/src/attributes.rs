// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Properties for specifying font weight, stretch and style.

#[cfg(not(feature = "std"))]
use core_maths::*;

use core::fmt;

/// Primary attributes for font matching: stretch, style and weight.
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct Attributes {
    pub stretch: Stretch,
    pub style: Style,
    pub weight: Weight,
}

impl Attributes {
    /// Creates new attributes from the given stretch, style and weight.
    pub fn new(stretch: Stretch, style: Style, weight: Weight) -> Self {
        Self {
            stretch,
            style,
            weight,
        }
    }
}

impl fmt::Display for Attributes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "stretch: {}, style: {}, weight: {}",
            self.stretch, self.style, self.weight
        )
    }
}

/// Visual width of a font-- a relative change from the normal aspect
/// ratio, typically in the range 0.5 to 2.0.
///
/// In variable fonts, this can be controlled with the `wdth` axis.
///
/// See <https://fonts.google.com/knowledge/glossary/width>
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
pub struct Stretch(f32);

impl Stretch {
    /// Width that is 50% of normal.
    pub const ULTRA_CONDENSED: Self = Self(0.5);

    /// Width that is 62.5% of normal.
    pub const EXTRA_CONDENSED: Self = Self(0.625);

    /// Width that is 75% of normal.
    pub const CONDENSED: Self = Self(0.75);

    /// Width that is 87.5% of normal.
    pub const SEMI_CONDENSED: Self = Self(0.875);

    /// Width that is 100% of normal.
    pub const NORMAL: Self = Self(1.0);

    /// Width that is 112.5% of normal.
    pub const SEMI_EXPANDED: Self = Self(1.125);

    /// Width that is 125% of normal.
    pub const EXPANDED: Self = Self(1.25);

    /// Width that is 150% of normal.
    pub const EXTRA_EXPANDED: Self = Self(1.5);

    /// Width that is 200% of normal.
    pub const ULTRA_EXPANDED: Self = Self(2.0);
}

impl Stretch {
    /// Creates a new stretch attribute with the given ratio.
    pub fn from_ratio(ratio: f32) -> Self {
        Self(ratio)
    }

    /// Creates a stretch attribute from a percentage.
    pub fn from_percentage(percentage: f32) -> Self {
        Self(percentage / 100.0)
    }

    /// Returns the stretch attribute as a ratio.
    ///
    /// This is a linear scaling factor with 1.0 being "normal" width.
    pub fn ratio(self) -> f32 {
        self.0
    }

    /// Returns the stretch attribute as a percentage value.
    ///
    /// This is generally the value associated with the `wdth` axis.
    pub fn percentage(self) -> f32 {
        self.0 * 100.0
    }

    /// Returns true if the stretch is normal.
    pub fn is_normal(self) -> bool {
        self == Self::NORMAL
    }

    /// Returns true if the stretch is condensed (less than normal).
    pub fn is_condensed(self) -> bool {
        self < Self::NORMAL
    }

    /// Returns true if the stretch is expanded (greater than normal).
    pub fn is_expanded(self) -> bool {
        self > Self::NORMAL
    }

    /// Parses the stretch from a CSS style keyword or a percentage value.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        Some(match s {
            "ultra-condensed" => Self::ULTRA_CONDENSED,
            "extra-condensed" => Self::EXTRA_CONDENSED,
            "condensed" => Self::CONDENSED,
            "semi-condensed" => Self::SEMI_CONDENSED,
            "normal" => Self::NORMAL,
            "semi-expanded" => Self::SEMI_EXPANDED,
            "extra-expanded" => Self::EXTRA_EXPANDED,
            "ultra-expanded" => Self::ULTRA_EXPANDED,
            _ => {
                if s.ends_with('%') {
                    let p = s.get(..s.len() - 1)?.parse::<f32>().ok()?;
                    return Some(Self::from_percentage(p));
                }
                return None;
            }
        })
    }
}

impl fmt::Display for Stretch {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let value = self.0 * 1000.0;
        if value.fract() == 0.0 {
            let keyword = match value as i32 {
                500 => "ultra-condensed",
                625 => "extra-condensed",
                750 => "condensed",
                875 => "semi-condensed",
                1000 => "normal",
                1125 => "semi-expanded",
                1250 => "expanded",
                1500 => "extra-expanded",
                2000 => "ultra-expanded",
                _ => {
                    return write!(f, "{}%", self.percentage());
                }
            };
            write!(f, "{}", keyword)
        } else {
            write!(f, "{}%", self.percentage())
        }
    }
}

impl Default for Stretch {
    fn default() -> Self {
        Self::NORMAL
    }
}

/// Visual weight class of a font, typically on a scale from 1.0 to 1000.0.
///
/// In variable fonts, this can be controlled with the `wght` axis.
///
/// See <https://fonts.google.com/knowledge/glossary/weight>
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
pub struct Weight(f32);

impl Weight {
    /// Weight value of 100.
    pub const THIN: Self = Self(100.0);

    /// Weight value of 200.
    pub const EXTRA_LIGHT: Self = Self(200.0);

    /// Weight value of 300.
    pub const LIGHT: Self = Self(300.0);

    /// Weight value of 350.
    pub const SEMI_LIGHT: Self = Self(350.0);

    /// Weight value of 400.
    pub const NORMAL: Self = Self(400.0);

    /// Weight value of 500.
    pub const MEDIUM: Self = Self(500.0);

    /// Weight value of 600.
    pub const SEMI_BOLD: Self = Self(600.0);

    /// Weight value of 700.
    pub const BOLD: Self = Self(700.0);

    /// Weight value of 800.
    pub const EXTRA_BOLD: Self = Self(800.0);

    /// Weight value of 900.
    pub const BLACK: Self = Self(900.0);

    /// Weight value of 950.
    pub const EXTRA_BLACK: Self = Self(950.0);
}

impl Weight {
    /// Creates a new weight attribute with the given value.
    pub fn new(weight: f32) -> Self {
        Self(weight)
    }

    /// Returns the underlying weight value.
    pub fn value(self) -> f32 {
        self.0
    }

    /// Parses a CSS style font weight attribute.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        Some(match s {
            "normal" => Self::NORMAL,
            "bold" => Self::BOLD,
            _ => Self(s.parse::<f32>().ok()?),
        })
    }
}

impl Default for Weight {
    fn default() -> Self {
        Self::NORMAL
    }
}

impl fmt::Display for Weight {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let value = self.0;
        if value.fract() == 0.0 {
            let keyword = match value as i32 {
                100 => "thin",
                200 => "extra-light",
                300 => "light",
                400 => "normal",
                500 => "medium",
                600 => "semi-bold",
                700 => "bold",
                800 => "extra-bold",
                900 => "black",
                _ => return write!(f, "{}", self.0),
            };
            write!(f, "{}", keyword)
        } else {
            write!(f, "{}", self.0)
        }
    }
}

/// Visual style or 'slope' of a font.
///
/// In variable fonts, this can be controlled with the `ital`
/// and `slnt` axes for italic and oblique styles, respectively.
///
/// See <https://fonts.google.com/knowledge/glossary/style>
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub enum Style {
    /// An upright or "roman" style.
    #[default]
    Normal,
    /// Generally a slanted style, originally based on semi-cursive forms.
    /// This often has a different structure from the normal style.
    Italic,
    /// Oblique (or slanted) style with an optional angle in degrees,
    /// counter-clockwise from the vertical.
    Oblique(Option<f32>),
}

impl Style {
    /// Parses a font style from a CSS value.
    pub fn parse(mut s: &str) -> Option<Self> {
        s = s.trim();
        Some(match s {
            "normal" => Self::Normal,
            "italic" => Self::Italic,
            "oblique" => Self::Oblique(Some(14.)),
            _ => {
                if s.starts_with("oblique ") {
                    s = s.get(8..)?;
                    if s.ends_with("deg") {
                        s = s.get(..s.len() - 3)?;
                        if let Ok(degrees) = s.trim().parse::<f32>() {
                            return Some(Self::Oblique(Some(degrees)));
                        }
                    } else if s.ends_with("grad") {
                        s = s.get(..s.len() - 4)?;
                        if let Ok(gradians) = s.trim().parse::<f32>() {
                            return Some(Self::Oblique(Some(gradians / 400.0 * 360.0)));
                        }
                    } else if s.ends_with("rad") {
                        s = s.get(..s.len() - 3)?;
                        if let Ok(radians) = s.trim().parse::<f32>() {
                            return Some(Self::Oblique(Some(radians.to_degrees())));
                        }
                    } else if s.ends_with("turn") {
                        s = s.get(..s.len() - 4)?;
                        if let Ok(turns) = s.trim().parse::<f32>() {
                            return Some(Self::Oblique(Some(turns * 360.0)));
                        }
                    }
                    return Some(Self::Oblique(None));
                }
                return None;
            }
        })
    }
}

impl fmt::Display for Style {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let value = match self {
            Self::Normal => "normal",
            Self::Italic => "italic",
            Self::Oblique(None) => "oblique",
            Self::Oblique(Some(degrees)) if *degrees == 14.0 => "oblique",
            Self::Oblique(Some(degrees)) => {
                return write!(f, "oblique({}deg)", degrees);
            }
        };
        write!(f, "{}", value)
    }
}
