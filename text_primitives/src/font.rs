// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;

/// Visual weight class of a font, typically on a scale from 1.0 to 1000.0.
///
/// In variable fonts, this can be controlled with the `wght` axis. This uses an `f32` so that it
/// can represent the full range of values possible with variable fonts.
///
/// In CSS, this corresponds to the `font-weight` property.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct FontWeight(f32);

impl FontWeight {
    /// Weight value of 100.
    pub const THIN: Self = Self(100.0);

    /// Weight value of 200.
    pub const EXTRA_LIGHT: Self = Self(200.0);

    /// Weight value of 300.
    pub const LIGHT: Self = Self(300.0);

    /// Weight value of 350.
    pub const SEMI_LIGHT: Self = Self(350.0);

    /// Weight value of 400. This is the default value.
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

    /// Creates a new weight value.
    pub fn new(weight: f32) -> Self {
        Self(weight)
    }

    /// Returns the underlying weight value.
    pub fn value(self) -> f32 {
        self.0
    }

    /// Parses a CSS `font-weight` value.
    ///
    /// Supported syntax (after trimming ASCII whitespace):
    /// - `normal` → `FontWeight::NORMAL`
    /// - `bold` → `FontWeight::BOLD`
    /// - a number → `FontWeight::new(value)`
    ///
    /// This parser is case-sensitive and does not clamp the numeric range.
    ///
    /// ```
    /// use text_primitives::FontWeight;
    ///
    /// assert_eq!(FontWeight::parse("normal"), Some(FontWeight::NORMAL));
    /// assert_eq!(FontWeight::parse("bold"), Some(FontWeight::BOLD));
    /// assert_eq!(FontWeight::parse("850"), Some(FontWeight::new(850.0)));
    /// assert_eq!(FontWeight::parse("invalid"), None);
    /// ```
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        Some(match s {
            "normal" => Self::NORMAL,
            "bold" => Self::BOLD,
            _ => Self(s.parse::<f32>().ok()?),
        })
    }
}

impl Default for FontWeight {
    fn default() -> Self {
        Self::NORMAL
    }
}

impl fmt::Display for FontWeight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "The integer keyword mapping is only used when the cast is lossless (checked)."
        )]
        let int_value = self.0 as i32;

        if self.0 == int_value as f32 {
            let keyword = match int_value {
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
            f.write_str(keyword)
        } else {
            write!(f, "{}", self.0)
        }
    }
}

/// Visual width of a font — a relative change from the normal aspect ratio.
///
/// In variable fonts, this can be controlled with the `wdth` axis. This uses an `f32` so that it
/// can represent the full range of values possible with variable fonts.
///
/// In CSS, this corresponds to the `font-width` (`font-stretch`) property.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct FontWidth(f32);

impl FontWidth {
    /// Width that is 50% of normal.
    pub const ULTRA_CONDENSED: Self = Self(0.5);

    /// Width that is 62.5% of normal.
    pub const EXTRA_CONDENSED: Self = Self(0.625);

    /// Width that is 75% of normal.
    pub const CONDENSED: Self = Self(0.75);

    /// Width that is 87.5% of normal.
    pub const SEMI_CONDENSED: Self = Self(0.875);

    /// Width that is 100% of normal. This is the default value.
    pub const NORMAL: Self = Self(1.0);

    /// Width that is 112.5% of normal.
    pub const SEMI_EXPANDED: Self = Self(1.125);

    /// Width that is 125% of normal.
    pub const EXPANDED: Self = Self(1.25);

    /// Width that is 150% of normal.
    pub const EXTRA_EXPANDED: Self = Self(1.5);

    /// Width that is 200% of normal.
    pub const ULTRA_EXPANDED: Self = Self(2.0);

    /// Creates a new width value with the given ratio.
    pub fn from_ratio(ratio: f32) -> Self {
        Self(ratio)
    }

    /// Creates a width value from a percentage.
    pub fn from_percentage(percentage: f32) -> Self {
        Self(percentage / 100.0)
    }

    /// Returns the width value as a ratio, with `1.0` being normal width.
    pub fn ratio(self) -> f32 {
        self.0
    }

    /// Returns the width value as a percentage.
    pub fn percentage(self) -> f32 {
        self.0 * 100.0
    }

    /// Returns `true` if the width is normal.
    pub fn is_normal(self) -> bool {
        self == Self::NORMAL
    }

    /// Returns `true` if the width is condensed (less than normal).
    pub fn is_condensed(self) -> bool {
        self < Self::NORMAL
    }

    /// Returns `true` if the width is expanded (greater than normal).
    pub fn is_expanded(self) -> bool {
        self > Self::NORMAL
    }

    /// Parses a CSS `font-width` / `font-stretch` value.
    ///
    /// Supported syntax (after trimming ASCII whitespace):
    /// - keywords: `ultra-condensed`, `extra-condensed`, `condensed`, `semi-condensed`, `normal`,
    ///   `semi-expanded`, `expanded`, `extra-expanded`, `ultra-expanded`
    /// - a percentage: e.g. `87.5%` → `FontWidth::from_percentage(87.5)`
    ///
    /// This parser is case-sensitive.
    ///
    /// ```
    /// use text_primitives::FontWidth;
    ///
    /// assert_eq!(FontWidth::parse("semi-condensed"), Some(FontWidth::SEMI_CONDENSED));
    /// assert_eq!(
    ///     FontWidth::parse("80%"),
    ///     Some(FontWidth::from_percentage(80.0))
    /// );
    /// assert_eq!(FontWidth::parse("wideload"), None);
    /// ```
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        Some(match s {
            "ultra-condensed" => Self::ULTRA_CONDENSED,
            "extra-condensed" => Self::EXTRA_CONDENSED,
            "condensed" => Self::CONDENSED,
            "semi-condensed" => Self::SEMI_CONDENSED,
            "normal" => Self::NORMAL,
            "semi-expanded" => Self::SEMI_EXPANDED,
            "expanded" => Self::EXPANDED,
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

impl Default for FontWidth {
    fn default() -> Self {
        Self::NORMAL
    }
}

impl fmt::Display for FontWidth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = self.0 * 1000.0;

        #[allow(
            clippy::cast_possible_truncation,
            reason = "The integer keyword mapping is only used when the cast is lossless (checked)."
        )]
        let int_value = value as i32;

        if value == int_value as f32 {
            let keyword = match int_value {
                500 => "ultra-condensed",
                625 => "extra-condensed",
                750 => "condensed",
                875 => "semi-condensed",
                1000 => "normal",
                1125 => "semi-expanded",
                1250 => "expanded",
                1500 => "extra-expanded",
                2000 => "ultra-expanded",
                _ => return write!(f, "{}%", self.percentage()),
            };
            f.write_str(keyword)
        } else {
            write!(f, "{}%", self.percentage())
        }
    }
}

/// Visual style or “slope” of a font.
///
/// In variable fonts, this can be controlled with the `ital` and `slnt` axes for italic and
/// oblique styles, respectively.
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

impl FontStyle {
    /// Parses a CSS `font-style` value.
    ///
    /// Supported syntax (after trimming ASCII whitespace):
    /// - `normal` → `FontStyle::Normal`
    /// - `italic` → `FontStyle::Italic`
    /// - `oblique` → `FontStyle::Oblique(Some(14.0))`
    /// - `oblique <angle>` where `<angle>` is one of:
    ///   - `<number>deg`
    ///   - `<number>grad` (gradians, converted to degrees)
    ///   - `<number>rad` (radians, converted to degrees)
    ///   - `<number>turn` (turns, converted to degrees)
    ///
    /// If an `oblique <angle>` form is present but the angle cannot be parsed, this returns
    /// `Some(FontStyle::Oblique(None))`.
    ///
    /// This parser is case-sensitive.
    ///
    /// ```
    /// use text_primitives::FontStyle;
    ///
    /// assert_eq!(FontStyle::parse("normal"), Some(FontStyle::Normal));
    /// assert_eq!(FontStyle::parse("italic"), Some(FontStyle::Italic));
    /// assert_eq!(FontStyle::parse("oblique"), Some(FontStyle::Oblique(Some(14.0))));
    /// assert_eq!(FontStyle::parse("oblique 30deg"), Some(FontStyle::Oblique(Some(30.0))));
    /// assert_eq!(FontStyle::parse("oblique banana"), Some(FontStyle::Oblique(None)));
    /// assert_eq!(FontStyle::parse("banana"), None);
    /// ```
    pub fn parse(mut s: &str) -> Option<Self> {
        s = s.trim();
        Some(match s {
            "normal" => Self::Normal,
            "italic" => Self::Italic,
            "oblique" => Self::Oblique(Some(14.0)),
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

impl fmt::Display for FontStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Normal => "normal",
            Self::Italic => "italic",
            Self::Oblique(None) => "oblique",
            Self::Oblique(Some(degrees)) if *degrees == 14.0 => "oblique",
            Self::Oblique(Some(degrees)) => {
                return write!(f, "oblique({degrees}deg)");
            }
        };
        write!(f, "{value}")
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use super::FontWidth;
    use crate::{FontStyle, FontWeight};

    #[test]
    fn fontwidth_parse_includes_expanded() {
        assert_eq!(FontWidth::parse("expanded"), Some(FontWidth::EXPANDED));
    }

    #[test]
    fn fontwidth_parse_keywords() {
        assert_eq!(FontWidth::parse("normal"), Some(FontWidth::NORMAL));
        assert_eq!(
            FontWidth::parse("ultra-condensed"),
            Some(FontWidth::ULTRA_CONDENSED)
        );
        assert_eq!(
            FontWidth::parse("extra-expanded"),
            Some(FontWidth::EXTRA_EXPANDED)
        );
        assert_eq!(FontWidth::parse("  condensed "), Some(FontWidth::CONDENSED));
    }

    #[test]
    fn fontwidth_parse_percentage() {
        assert_eq!(
            FontWidth::parse("87.5%"),
            Some(FontWidth::from_percentage(87.5))
        );
        assert_eq!(
            FontWidth::parse(" 80% "),
            Some(FontWidth::from_percentage(80.0))
        );
        assert_eq!(FontWidth::parse("80"), None);
        assert_eq!(FontWidth::parse("%"), None);
        assert_eq!(FontWidth::parse("80%%"), None);
    }

    #[test]
    fn fontweight_parse_keywords_and_numbers() {
        assert_eq!(FontWeight::parse("normal"), Some(FontWeight::NORMAL));
        assert_eq!(FontWeight::parse("bold"), Some(FontWeight::BOLD));
        assert_eq!(FontWeight::parse(" 850 "), Some(FontWeight::new(850.0)));
        assert_eq!(FontWeight::parse("invalid"), None);
    }

    #[test]
    fn fontstyle_parse_keywords() {
        assert_eq!(FontStyle::parse("normal"), Some(FontStyle::Normal));
        assert_eq!(FontStyle::parse("italic"), Some(FontStyle::Italic));
        assert_eq!(
            FontStyle::parse("oblique"),
            Some(FontStyle::Oblique(Some(14.0)))
        );
        assert_eq!(
            FontStyle::parse(" oblique "),
            Some(FontStyle::Oblique(Some(14.0)))
        );
    }

    #[test]
    fn fontstyle_parse_oblique_angles() {
        assert_eq!(
            FontStyle::parse("oblique 30deg"),
            Some(FontStyle::Oblique(Some(30.0)))
        );
        assert_eq!(
            FontStyle::parse("oblique 0.5turn"),
            Some(FontStyle::Oblique(Some(180.0)))
        );
        assert_eq!(
            FontStyle::parse("oblique 200grad"),
            Some(FontStyle::Oblique(Some(180.0)))
        );
        assert_eq!(
            FontStyle::parse("oblique 3.1415927rad"),
            Some(FontStyle::Oblique(Some(180.0)))
        );

        // Present but unparsable angle yields `Oblique(None)`.
        assert_eq!(
            FontStyle::parse("oblique banana"),
            Some(FontStyle::Oblique(None))
        );
        assert_eq!(
            FontStyle::parse("oblique 12"),
            Some(FontStyle::Oblique(None))
        );
        assert_eq!(
            FontStyle::parse("oblique 12foo"),
            Some(FontStyle::Oblique(None))
        );
    }

    #[test]
    fn fontstyle_parse_invalid() {
        assert_eq!(FontStyle::parse("banana"), None);
        assert_eq!(FontStyle::parse("oblique12deg"), None);
        assert_eq!(FontStyle::parse("Oblique"), None);
    }
}
