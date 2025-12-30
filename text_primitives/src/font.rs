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

    /// Parses a CSS font weight value.
    ///
    /// This supports the `normal` and `bold` keywords and numeric weights.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        Some(match s {
            "normal" => Self::NORMAL,
            "bold" => Self::BOLD,
            _ => Self(s.parse::<f32>().ok()?),
        })
    }

    /// Creates a weight value from a Fontconfig weight.
    ///
    /// The values are determined based on the `fonts.conf` documentation.
    pub fn from_fontconfig(weight: i32) -> Self {
        // A selection of OpenType weights (first) and their corresponding fontconfig value (second)
        // Invariant: The fontconfig values are sorted.
        const MAP: &[(i32, i32)] = &[
            (0, 0),
            (100, 0),
            (200, 40),
            (300, 50),
            (350, 55),
            (380, 75),
            (400, 80),
            (500, 100),
            (600, 180),
            (700, 200),
            (800, 205),
            (900, 210),
            (950, 215),
        ];

        for i in 1..MAP.len() {
            let (ot_b, fc_b) = MAP[i];
            if weight == fc_b {
                return Self::new(ot_b as f32);
            }

            if weight < fc_b {
                let weight = weight as f32;
                let fc_b = fc_b as f32;
                let ot_b = ot_b as f32;

                let (ot_a, fc_a) = MAP[i - 1];
                let fc_a = fc_a as f32;
                let ot_a = ot_a as f32;

                // Linear interpolation between (fc_a → ot_a) and (fc_b → ot_b).
                let t = (weight - fc_a) / (fc_b - fc_a);
                return Self::new(ot_a + (ot_b - ot_a) * t);
            }
        }

        Self::EXTRA_BLACK
    }
}

impl Default for FontWeight {
    fn default() -> Self {
        Self::NORMAL
    }
}

impl fmt::Display for FontWeight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let keyword = if self.0 == 100.0 {
            Some("thin")
        } else if self.0 == 200.0 {
            Some("extra-light")
        } else if self.0 == 300.0 {
            Some("light")
        } else if self.0 == 400.0 {
            Some("normal")
        } else if self.0 == 500.0 {
            Some("medium")
        } else if self.0 == 600.0 {
            Some("semi-bold")
        } else if self.0 == 700.0 {
            Some("bold")
        } else if self.0 == 800.0 {
            Some("extra-bold")
        } else if self.0 == 900.0 {
            Some("black")
        } else {
            None
        };

        if let Some(keyword) = keyword {
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

    /// Parses the width from a CSS style keyword or a percentage value.
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

    /// Creates a width value from a Fontconfig width.
    ///
    /// The values are determined based on the `fonts.conf` documentation.
    pub fn from_fontconfig(width: i32) -> Self {
        match width {
            50 => Self::ULTRA_CONDENSED,
            63 => Self::EXTRA_CONDENSED,
            75 => Self::CONDENSED,
            87 => Self::SEMI_CONDENSED,
            100 => Self::NORMAL,
            113 => Self::SEMI_EXPANDED,
            125 => Self::EXPANDED,
            150 => Self::EXTRA_EXPANDED,
            200 => Self::ULTRA_EXPANDED,
            _ => Self::from_ratio(width as f32 / 100.0),
        }
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
        let keyword = if value == 500.0 {
            Some("ultra-condensed")
        } else if value == 625.0 {
            Some("extra-condensed")
        } else if value == 750.0 {
            Some("condensed")
        } else if value == 875.0 {
            Some("semi-condensed")
        } else if value == 1000.0 {
            Some("normal")
        } else if value == 1125.0 {
            Some("semi-expanded")
        } else if value == 1250.0 {
            Some("expanded")
        } else if value == 1500.0 {
            Some("extra-expanded")
        } else if value == 2000.0 {
            Some("ultra-expanded")
        } else {
            None
        };

        if let Some(keyword) = keyword {
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
    /// Parses a font style from a CSS value.
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

    /// Creates a style value from a Fontconfig slant.
    ///
    /// The values are determined based on the `fonts.conf` documentation.
    pub fn from_fontconfig(slant: i32) -> Self {
        match slant {
            100 => Self::Italic,
            110 => Self::Oblique(None),
            _ => Self::Normal,
        }
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

    use alloc::string::ToString;

    use super::{FontStyle, FontWeight, FontWidth};

    #[test]
    fn fontwidth_parse_includes_expanded() {
        assert_eq!(FontWidth::parse("expanded"), Some(FontWidth::EXPANDED));
    }

    #[test]
    fn fontwidth_from_fontconfig() {
        fn check_fc(fc: i32, s: &str) {
            let fs = FontWidth::from_fontconfig(fc);
            assert_eq!(s, fs.to_string());
        }

        check_fc(50, "ultra-condensed");
        check_fc(63, "extra-condensed");
        check_fc(75, "condensed");
        check_fc(87, "semi-condensed");
        check_fc(100, "normal");
        check_fc(113, "semi-expanded");
        check_fc(125, "expanded");
        check_fc(150, "extra-expanded");
        check_fc(200, "ultra-expanded");
    }

    #[test]
    fn fontstyle_from_fontconfig() {
        fn check_fc(fc: i32, s: &str) {
            let fs = FontStyle::from_fontconfig(fc);
            assert_eq!(s, fs.to_string());
        }

        check_fc(0, "normal");
        check_fc(100, "italic");
        check_fc(110, "oblique");
    }

    #[test]
    fn fontweight_from_fontconfig_interpolates_monotonically() {
        let demilight = FontWeight::from_fontconfig(55);
        assert!(demilight > FontWeight::LIGHT);
        assert!(demilight < FontWeight::NORMAL);

        let book = FontWeight::from_fontconfig(75);
        assert!(book > demilight);
        assert!(book < FontWeight::NORMAL);

        let extrablack = FontWeight::from_fontconfig(215);
        assert!(extrablack > FontWeight::BLACK);
    }
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
