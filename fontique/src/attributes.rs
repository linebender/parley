// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Properties for specifying font weight, width and style.

#[cfg(feature = "libm")]
#[allow(unused_imports)]
use core_maths::CoreFloat;

use core::fmt;

/// Primary attributes for font matching: [`FontWidth`], [`FontStyle`] and [`FontWeight`].
///
/// These are used to [configure] a [`Query`].
///
/// [configure]: crate::Query::set_attributes
/// [`Query`]: crate::Query
#[derive(Copy, Clone, PartialEq, Eq, Hash, Default, Debug)]
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

/// Visual width of a font-- a relative change from the normal aspect
/// ratio, typically in the 50% - 200% range.
///
/// The default value is [`FontWidth::NORMAL`].
///
/// In variable fonts, this can be controlled with the `wdth` [axis].
///
/// In Open Type, the `u16` [`usWidthClass`] field has 9 values, from 1-9,
/// which doesn't allow for the wide range of values possible with variable
/// fonts.
///
/// See <https://fonts.google.com/knowledge/glossary/width>
///
/// In CSS, this corresponds to the [`font-width`] property.
///
/// This has also been known as "stretch" and has a legacy CSS name alias,
/// [`font-stretch`].
///
/// [axis]: crate::AxisInfo
/// [`usWidthClass`]: https://learn.microsoft.com/en-us/typography/opentype/spec/os2#uswidthclass
/// [`font-width`]: https://www.w3.org/TR/css-fonts-4/#font-width-prop
/// [`font-stretch`]: https://www.w3.org/TR/css-fonts-4/#font-stretch-prop
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct FontWidth(u16);

impl FontWidth {
    /// Width that is 50% of normal.
    pub const ULTRA_CONDENSED: Self = Self(128);

    /// Width that is 62.5% of normal.
    pub const EXTRA_CONDENSED: Self = Self(160);

    /// Width that is 75% of normal.
    pub const CONDENSED: Self = Self(192);

    /// Width that is 87.5% of normal.
    pub const SEMI_CONDENSED: Self = Self(224);

    /// Width that is 100% of normal. This is the default value.
    pub const NORMAL: Self = Self(256);

    /// Width that is 112.5% of normal.
    pub const SEMI_EXPANDED: Self = Self(288);

    /// Width that is 125% of normal.
    pub const EXPANDED: Self = Self(320);

    /// Width that is 150% of normal.
    pub const EXTRA_EXPANDED: Self = Self(384);

    /// Width that is 200% of normal.
    pub const ULTRA_EXPANDED: Self = Self(512);
}

impl FontWidth {
    /// Creates a new width attribute with the given ratio.
    ///
    /// Panics if the ratio is not between `0` and `255.996`.
    ///
    /// This can also be created [from a percentage](Self::from_percentage).
    ///
    /// # Example
    ///
    /// ```
    /// # use fontique::FontWidth;
    /// assert_eq!(FontWidth::from_ratio(1.5), FontWidth::EXTRA_EXPANDED);
    /// ```
    pub fn from_ratio(ratio: f32) -> Self {
        let value = (ratio * 256.0).round();
        assert!(0.0 <= value && value <= (u16::MAX as f32));
        Self(value as u16)
    }

    /// Creates a width attribute from a percentage.
    ///
    /// Panics if the percentage is not between `0%` and `25599.6%`.
    ///
    /// This can also be created [from a ratio](Self::from_ratio).
    ///
    /// # Example
    ///
    /// ```
    /// # use fontique::FontWidth;
    /// assert_eq!(FontWidth::from_percentage(87.5), FontWidth::SEMI_CONDENSED);
    /// ```
    pub fn from_percentage(percentage: f32) -> Self {
        Self::from_ratio(percentage / 100.0)
    }

    /// Returns the width attribute as a ratio.
    ///
    /// This is a linear scaling factor with `1.0` being "normal" width.
    ///
    /// # Example
    ///
    /// ```
    /// # use fontique::FontWidth;
    /// assert_eq!(FontWidth::NORMAL.ratio(), 1.0);
    /// ```
    pub fn ratio(self) -> f32 {
        (self.0 as f32) / 256.0
    }

    /// Returns the width attribute as a percentage value.
    ///
    /// This is generally the value associated with the `wdth` axis.
    pub fn percentage(self) -> f32 {
        self.ratio() * 100.0
    }

    /// Returns `true` if the width is [normal].
    ///
    /// [normal]: FontWidth::NORMAL
    pub fn is_normal(self) -> bool {
        self == Self::NORMAL
    }

    /// Returns `true` if the width is condensed (less than [normal]).
    ///
    /// [normal]: FontWidth::NORMAL
    pub fn is_condensed(self) -> bool {
        self < Self::NORMAL
    }

    /// Returns `true` if the width is expanded (greater than [normal]).
    ///
    /// [normal]: FontWidth::NORMAL
    pub fn is_expanded(self) -> bool {
        self > Self::NORMAL
    }

    /// Parses the width from a CSS style keyword or a percentage value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use fontique::FontWidth;
    /// assert_eq!(FontWidth::parse("semi-condensed"), Some(FontWidth::SEMI_CONDENSED));
    /// assert_eq!(FontWidth::parse("80%"), Some(FontWidth::from_percentage(80.0)));
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

impl FontWidth {
    /// Creates a new width attribute with the given value from Fontconfig.
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

impl fmt::Display for FontWidth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let keyword = match *self {
            v if v == Self::ULTRA_CONDENSED => "ultra-condensed",
            v if v == Self::EXTRA_CONDENSED => "extra-condensed",
            v if v == Self::CONDENSED => "condensed",
            v if v == Self::SEMI_CONDENSED => "semi-condensed",
            v if v == Self::NORMAL => "normal",
            v if v == Self::SEMI_EXPANDED => "semi-expanded",
            v if v == Self::EXPANDED => "expanded",
            v if v == Self::EXTRA_EXPANDED => "extra-expanded",
            v if v == Self::ULTRA_EXPANDED => "ultra-expanded",
            _ => {
                return write!(f, "{}%", self.percentage());
            }
        };
        write!(f, "{keyword}")
    }
}

impl Default for FontWidth {
    fn default() -> Self {
        Self::NORMAL
    }
}

/// Visual weight class of a font, typically on a scale from 1 to 1000.
///
/// The default value is [`FontWeight::NORMAL`] or `400`.
///
/// In variable fonts, this can be controlled with the `wght` [axis]. This
/// is a `u16` so that it can represent the same range of values as the
/// `wght` axis.
///
/// See <https://fonts.google.com/knowledge/glossary/weight>
///
/// In CSS, this corresponds to the [`font-weight`] property.
///
/// [axis]: crate::AxisInfo
/// [`font-weight`]: https://www.w3.org/TR/css-fonts-4/#font-weight-prop
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct FontWeight(u16);

impl FontWeight {
    /// Weight value of 100.
    pub const THIN: Self = Self(100);

    /// Weight value of 200.
    pub const EXTRA_LIGHT: Self = Self(200);

    /// Weight value of 300.
    pub const LIGHT: Self = Self(300);

    /// Weight value of 350.
    pub const SEMI_LIGHT: Self = Self(350);

    /// Weight value of 400. This is the default value.
    pub const NORMAL: Self = Self(400);

    /// Weight value of 500.
    pub const MEDIUM: Self = Self(500);

    /// Weight value of 600.
    pub const SEMI_BOLD: Self = Self(600);

    /// Weight value of 700.
    pub const BOLD: Self = Self(700);

    /// Weight value of 800.
    pub const EXTRA_BOLD: Self = Self(800);

    /// Weight value of 900.
    pub const BLACK: Self = Self(900);

    /// Weight value of 950.
    pub const EXTRA_BLACK: Self = Self(950);
}

impl FontWeight {
    /// Creates a new weight attribute with the given value.
    pub fn new(weight: u16) -> Self {
        Self(weight)
    }

    /// Returns the underlying weight value.
    pub fn value(self) -> u16 {
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
    /// assert_eq!(FontWeight::parse("850"), Some(FontWeight::new(850)));
    /// assert_eq!(FontWeight::parse("invalid"), None);
    /// ```
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        Some(match s {
            "normal" => Self::NORMAL,
            "bold" => Self::BOLD,
            _ => Self(s.parse::<u16>().ok()?),
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
                return Self::new(*ot as u16);
            }
            // Linear interpolation if not an exact match
            if weight < *fc {
                let weight = weight as f32;
                let fc_a = MAP[i - 1].1 as f32;
                let fc_b = *fc as f32;
                let ot_a = MAP[i - 1].1 as f32;
                let ot_b = *ot as f32;
                let t = (fc_a - fc_b) / (weight - fc_a);
                let w = ot_a + (ot_b - ot_a) * t;
                return Self::new(w.round() as u16);
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
        let keyword = match self.0 {
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
        write!(f, "{keyword}")
    }
}

/// Visual style or 'slope' of a font.
///
/// The default value is [`FontStyle::Normal`].
///
/// In variable fonts, this can be controlled with the `ital`
/// and `slnt` [axes] for italic and oblique styles, respectively.
/// This uses an `f32` for the `Oblique` variant so so that it
/// can represent the same range of values as the `slnt` axis.
///
/// See <https://fonts.google.com/knowledge/glossary/style>
///
/// In CSS, this corresponds to the [`font-style`] property.
///
/// [axes]: crate::AxisInfo
/// [`font-style`]: https://www.w3.org/TR/css-fonts-4/#font-style-prop
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug, Hash)]
pub enum FontStyle {
    /// An upright or "roman" style.
    #[default]
    Normal,
    /// Generally a slanted style, originally based on semi-cursive forms.
    /// This often has a different structure from the normal style.
    Italic,
    /// Oblique (or slanted) style with an optional angle in degrees times 256,
    /// counter-clockwise from the vertical.
    ///
    /// To convert `Some(angle)` to degrees, use
    /// `degrees = (angle as f32) / 256.0` or [`FontStyle::oblique_degrees`].
    Oblique(Option<i16>),
}

impl FontStyle {
    /// Parses a font style from a CSS value.
    pub fn parse(mut s: &str) -> Option<Self> {
        s = s.trim();
        Some(match s {
            "normal" => Self::Normal,
            "italic" => Self::Italic,
            "oblique" => Self::Oblique(Some(14 * 256)),
            _ => {
                if s.starts_with("oblique ") {
                    s = s.get(8..)?;
                    if s.ends_with("deg") {
                        s = s.get(..s.len() - 3)?;
                        if let Ok(degrees) = s.trim().parse::<f32>() {
                            return Some(Self::from_degrees(degrees));
                        }
                    } else if s.ends_with("grad") {
                        s = s.get(..s.len() - 4)?;
                        if let Ok(gradians) = s.trim().parse::<f32>() {
                            return Some(Self::from_degrees(gradians / 400.0 * 360.0));
                        }
                    } else if s.ends_with("rad") {
                        s = s.get(..s.len() - 3)?;
                        if let Ok(radians) = s.trim().parse::<f32>() {
                            return Some(Self::from_degrees(radians.to_degrees()));
                        }
                    } else if s.ends_with("turn") {
                        s = s.get(..s.len() - 4)?;
                        if let Ok(turns) = s.trim().parse::<f32>() {
                            return Some(Self::from_degrees(turns * 360.0));
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
    /// Convert an `Oblique` payload to an angle in degrees.
    pub const fn oblique_degrees(angle: Option<i16>) -> f32 {
        if let Some(a) = angle {
            (a as f32) / 256.0
        } else {
            14.0
        }
    }

    /// Creates a new oblique style with angle specified in degrees.
    ///
    /// Panics if `degrees` is not between `-90` and `90`.
    pub fn from_degrees(degrees: f32) -> Self {
        let a = (degrees * 256.0).round();
        assert!(-90.0 <= a && a <= 90.0);
        Self::Oblique(Some(a as i16))
    }

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
        let value = match *self {
            Self::Normal => "normal",
            Self::Italic => "italic",
            Self::Oblique(None) => "oblique",
            Self::Oblique(Some(angle)) if angle == 14 * 256 => "oblique",
            Self::Oblique(Some(angle)) => {
                let degrees = (angle as f32) / 256.0;
                return write!(f, "oblique({degrees}deg)");
            }
        };
        write!(f, "{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::{FontStyle, FontWeight, FontWidth};

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
