// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Properties for specifying font weight, stretch and style.

#[cfg(feature = "libm")]
#[allow(unused_imports)]
use core_maths::CoreFloat;

use core::fmt;

/// Primary attributes for font matching: [`FontStretch`], [`FontStyle`] and [`FontWeight`].
///
/// These are used to [configure] a [`Query`].
///
/// [configure]: crate::Query::set_attributes
/// [`Query`]: crate::Query
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct Attributes {
    pub stretch: FontStretch,
    pub style: FontStyle,
    pub weight: FontWeight,
}

impl Attributes {
    /// Creates new attributes from the given stretch, style and weight.
    pub fn new(stretch: FontStretch, style: FontStyle, weight: FontWeight) -> Self {
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
/// ratio, typically in the range `0.5` to `2.0`.
///
/// The default value is [`FontStretch::NORMAL`] or `1.0`.
///
/// In variable fonts, this can be controlled with the `wdth` [axis].
///
/// See <https://fonts.google.com/knowledge/glossary/width>
///
/// In CSS, this corresponds to the [`font-width`] property.
///
/// [axis]: crate::AxisInfo
/// [`font-width`]: https://www.w3.org/TR/css-fonts-4/#font-width-prop
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
pub struct FontStretch(f32);

impl FontStretch {
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
}

impl FontStretch {
    /// Creates a new stretch attribute with the given ratio.
    ///
    /// This can also be created [from a percentage](Self::from_percentage).
    ///
    /// # Example
    ///
    /// ```
    /// # use fontique::FontStretch;
    /// assert_eq!(FontStretch::from_ratio(1.5), FontStretch::EXTRA_EXPANDED);
    /// ```
    pub fn from_ratio(ratio: f32) -> Self {
        Self(ratio)
    }

    /// Creates a stretch attribute from a percentage.
    ///
    /// This can also be created [from a ratio](Self::from_ratio).
    ///
    /// # Example
    ///
    /// ```
    /// # use fontique::FontStretch;
    /// assert_eq!(FontStretch::from_percentage(87.5), FontStretch::SEMI_CONDENSED);
    /// ```
    pub fn from_percentage(percentage: f32) -> Self {
        Self(percentage / 100.0)
    }

    /// Returns the stretch attribute as a ratio.
    ///
    /// This is a linear scaling factor with `1.0` being "normal" width.
    ///
    /// # Example
    ///
    /// ```
    /// # use fontique::FontStretch;
    /// assert_eq!(FontStretch::NORMAL.ratio(), 1.0);
    /// ```
    pub fn ratio(self) -> f32 {
        self.0
    }

    /// Returns the stretch attribute as a percentage value.
    ///
    /// This is generally the value associated with the `wdth` axis.
    pub fn percentage(self) -> f32 {
        self.0 * 100.0
    }

    /// Returns `true` if the stretch is [normal].
    ///
    /// [normal]: FontStretch::NORMAL
    pub fn is_normal(self) -> bool {
        self == Self::NORMAL
    }

    /// Returns `true` if the stretch is condensed (less than [normal]).
    ///
    /// [normal]: FontStretch::NORMAL
    pub fn is_condensed(self) -> bool {
        self < Self::NORMAL
    }

    /// Returns `true` if the stretch is expanded (greater than [normal]).
    ///
    /// [normal]: FontStretch::NORMAL
    pub fn is_expanded(self) -> bool {
        self > Self::NORMAL
    }

    /// Parses the stretch from a CSS style keyword or a percentage value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use fontique::FontStretch;
    /// assert_eq!(FontStretch::parse("semi-condensed"), Some(FontStretch::SEMI_CONDENSED));
    /// assert_eq!(FontStretch::parse("80%"), Some(FontStretch::from_percentage(80.0)));
    /// assert_eq!(FontStretch::parse("wideload"), None);
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

impl FontStretch {
    /// Creates a new stretch attribute with the given value from Fontconfig.
    ///
    /// The values are determined based on the [fonts.conf documentation].
    ///
    /// [fonts.conf documentation]: https://www.freedesktop.org/software/fontconfig/fontconfig-user.html
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

impl fmt::Display for FontStretch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

impl Default for FontStretch {
    fn default() -> Self {
        Self::NORMAL
    }
}

/// Visual weight class of a font, typically on a scale from 1.0 to 1000.0.
///
/// The default value is [`FontWeight::NORMAL`] or `400.0`.
///
/// In variable fonts, this can be controlled with the `wght` [axis].
///
/// See <https://fonts.google.com/knowledge/glossary/weight>
///
/// In CSS, this corresponds to the [`font-weight`] property.
///
/// [axis]: crate::AxisInfo
/// [`font-weight`]: https://www.w3.org/TR/css-fonts-4/#font-weight-prop
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
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
}

impl FontWeight {
    /// Creates a new weight attribute with the given value.
    pub fn new(weight: f32) -> Self {
        Self(weight)
    }

    /// Returns the underlying weight value.
    pub fn value(self) -> f32 {
        self.0
    }

    /// Parses a CSS style font weight attribute.
    ///
    /// # Examples
    ///
    /// ```
    /// # use fontique::FontWeight;
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

impl FontWeight {
    /// Creates a new weight attribute with the given value from Fontconfig.
    ///
    /// The values are determined based on the [fonts.conf documentation].
    ///
    /// [fonts.conf documentation]: https://www.freedesktop.org/software/fontconfig/fontconfig-user.html
    pub fn from_fontconfig(weight: i32) -> Self {
        // A selection of OpenType weights (first) and their corresponding fontconfig value (second)
        // Invariant: The fontconfig values are sorted
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
        for (i, (ot, fc)) in MAP.iter().skip(1).enumerate() {
            if weight == *fc {
                return Self::new(*ot as f32);
            }
            // Linear interpolation if not an exact match
            if weight < *fc {
                let weight = weight as f32;
                let fc_a = MAP[i - 1].1 as f32;
                let fc_b = *fc as f32;
                let ot_a = MAP[i - 1].1 as f32;
                let ot_b = *ot as f32;
                let t = (fc_a - fc_b) / (weight - fc_a);
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
/// The default value is [`FontStyle::Normal`].
///
/// In variable fonts, this can be controlled with the `ital`
/// and `slnt` [axes] for italic and oblique styles, respectively.
///
/// See <https://fonts.google.com/knowledge/glossary/style>
///
/// In CSS, this corresponds to the [`font-style`] property.
///
/// [axes]: crate::AxisInfo
/// [`font-style`]: https://www.w3.org/TR/css-fonts-4/#font-style-prop
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub enum FontStyle {
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

impl FontStyle {
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

impl FontStyle {
    /// Creates a new style attribute with the given value from Fontconfig.
    ///
    /// The values are determined based on the [fonts.conf documentation].
    ///
    /// [fonts.conf documentation]: https://www.freedesktop.org/software/fontconfig/fontconfig-user.html
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
                return write!(f, "oblique({}deg)", degrees);
            }
        };
        write!(f, "{}", value)
    }
}

#[cfg(test)]
mod tests {
    use super::{FontStretch, FontStyle, FontWeight};

    #[test]
    fn fontstretch_from_fontconfig() {
        fn check_fc(fc: i32, s: &str) {
            let fs = FontStretch::from_fontconfig(fc);
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
    fn fontweight_from_fontconfig() {
        fn check_fc(fc: i32, s: &str) {
            let fw = FontWeight::from_fontconfig(fc);
            assert_eq!(s, fw.to_string());
        }

        check_fc(0, "thin");
        check_fc(40, "extra-light");
        check_fc(50, "light");
        check_fc(80, "normal");
        check_fc(100, "medium");
        check_fc(180, "semi-bold");
        check_fc(200, "bold");
        check_fc(205, "extra-bold");
        check_fc(210, "black");

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
