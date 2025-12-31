// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;

/// Generic font families, named after CSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
#[non_exhaustive]
pub enum GenericFamily {
    /// Glyphs have finishing strokes, flared or tapering ends, or have actual serifed endings.
    Serif = 0,
    /// Glyphs have stroke endings that are plain.
    SansSerif = 1,
    /// All glyphs have the same fixed width.
    Monospace = 2,
    /// Glyphs are partially or completely connected, resembling handwriting.
    Cursive = 3,
    /// Decorative fonts with playful representations of characters.
    Fantasy = 4,
    /// Glyphs are taken from the default user interface font on a given platform.
    SystemUi = 5,
    /// The default user interface serif font.
    UiSerif = 6,
    /// The default user interface sans-serif font.
    UiSansSerif = 7,
    /// The default user interface monospace font.
    UiMonospace = 8,
    /// The default user interface font that has rounded features.
    UiRounded = 9,
    /// Fonts that are specifically designed to render emoji.
    Emoji = 10,
    /// Fonts designed to represent mathematics.
    Math = 11,
    /// A Chinese character style between Song and Kai forms.
    FangSong = 12,
}

impl GenericFamily {
    /// Returns the maximum numeric value for known variants.
    ///
    /// This is primarily intended for use in fixed-size maps keyed by `GenericFamily`.
    pub const MAX_VALUE: u8 = Self::FangSong as u8;

    /// Parses a generic family from a CSS generic family name.
    ///
    /// ```
    /// use text_primitives::GenericFamily;
    ///
    /// assert_eq!(
    ///     GenericFamily::parse("sans-serif"),
    ///     Some(GenericFamily::SansSerif)
    /// );
    /// assert_eq!(GenericFamily::parse("Arial"), None);
    /// ```
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        Some(match s {
            "serif" => Self::Serif,
            "sans-serif" => Self::SansSerif,
            "monospace" => Self::Monospace,
            "cursive" => Self::Cursive,
            "fantasy" => Self::Fantasy,
            "system-ui" => Self::SystemUi,
            "ui-serif" => Self::UiSerif,
            "ui-sans-serif" => Self::UiSansSerif,
            "ui-monospace" => Self::UiMonospace,
            "ui-rounded" => Self::UiRounded,
            "emoji" => Self::Emoji,
            "math" => Self::Math,
            "fangsong" => Self::FangSong,
            _ => return None,
        })
    }

    /// Returns a slice containing all generic family variants.
    pub const fn all() -> &'static [Self] {
        &[
            Self::SansSerif,
            Self::Serif,
            Self::Monospace,
            Self::Cursive,
            Self::Fantasy,
            Self::SystemUi,
            Self::UiSerif,
            Self::UiSansSerif,
            Self::UiMonospace,
            Self::UiRounded,
            Self::Emoji,
            Self::Math,
            Self::FangSong,
        ]
    }
}

impl fmt::Display for GenericFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Serif => "serif",
            Self::SansSerif => "sans-serif",
            Self::Monospace => "monospace",
            Self::Cursive => "cursive",
            Self::Fantasy => "fantasy",
            Self::SystemUi => "system-ui",
            Self::UiSerif => "ui-serif",
            Self::UiSansSerif => "ui-sans-serif",
            Self::UiMonospace => "ui-monospace",
            Self::UiRounded => "ui-rounded",
            Self::Emoji => "emoji",
            Self::Math => "math",
            Self::FangSong => "fangsong",
        };
        f.write_str(name)
    }
}
