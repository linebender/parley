// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Generic font families.

use super::FamilyId;
use bytemuck::{checked::CheckedBitPattern, Contiguous, NoUninit, Zeroable};
use core::fmt;
use smallvec::SmallVec;

type FamilyVec = SmallVec<[FamilyId; 2]>;

/// Describes a generic font family.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum GenericFamily {
    /// Glyphs have finishing strokes, flared or tapering ends, or have actual
    /// serifed endings.
    Serif = 0,
    /// Glyphs have stroke endings that are plain.
    SansSerif = 1,
    /// All glyphs have the same fixed width.
    Monospace = 2,
    /// Glyphs in cursive fonts generally have either joining strokes or other
    /// cursive characteristics beyond those of italic typefaces. The glyphs
    /// are partially or completely connected, and the result looks more like
    /// handwritten pen or brush writing than printed letter work.
    Cursive = 3,
    /// Fantasy fonts are primarily decorative fonts that contain playful
    /// representations of characters
    Fantasy = 4,
    /// Glyphs are taken from the default user interface font on a given
    /// platform.
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
    /// This is for the particular stylistic concerns of representing
    /// mathematics: superscript and subscript, brackets that cross several
    /// lines, nesting expressions, and double struck glyphs with distinct
    /// meanings.
    Math = 11,
    /// A particular style of Chinese characters that are between serif-style
    /// Song and cursive-style Kai forms. This style is often used for
    /// government documents.
    FangSong = 12,
    // NOTICE: If a new value is added, be sure to modify `MAX_VALUE` in the bytemuck impl.
}

impl GenericFamily {
    /// Parses a generic family from a CSS generic family name.
    ///
    /// # Example
    /// ```
    /// # use fontique::GenericFamily;
    /// assert_eq!(GenericFamily::parse("sans-serif"), Some(GenericFamily::SansSerif));
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
        write!(f, "{}", name)
    }
}

// Safety: The enum is `repr(u8)` and has only fieldless variants.
unsafe impl NoUninit for GenericFamily {}

// Safety: The enum is `repr(u8)` and `0` is a valid value.
unsafe impl Zeroable for GenericFamily {}

// Safety: The enum is `repr(u8)`.
unsafe impl CheckedBitPattern for GenericFamily {
    type Bits = u8;

    fn is_valid_bit_pattern(bits: &u8) -> bool {
        // Don't need to compare against MIN_VALUE as this is u8 and 0 is the MIN_VALUE.
        *bits <= Self::MAX_VALUE
    }
}

// Safety: The enum is `repr(u8)`. All values are `u8` and fall within
// the min and max values.
unsafe impl Contiguous for GenericFamily {
    type Int = u8;
    const MIN_VALUE: u8 = Self::Serif as u8;
    const MAX_VALUE: u8 = Self::FangSong as u8;
}

const COUNT: usize = GenericFamily::MAX_VALUE as usize + 1;

/// Maps generic families to family identifiers.
#[derive(Clone, Default, Debug)]
pub struct GenericFamilyMap {
    map: [FamilyVec; COUNT],
}

impl GenericFamilyMap {
    /// Returns the associated family identifiers for the given generic family.
    pub fn get(&self, generic: GenericFamily) -> &[FamilyId] {
        &self.map[generic as usize]
    }

    /// Sets the associated family identifiers for the given generic family.
    pub fn set(&mut self, generic: GenericFamily, families: impl Iterator<Item = FamilyId>) {
        let map = &mut self.map[generic as usize];
        map.clear();
        map.extend(families);
    }

    /// Appends the family identifiers to the list for the given generic family.
    pub fn append(&mut self, generic: GenericFamily, families: impl Iterator<Item = FamilyId>) {
        self.map[generic as usize].extend(families);
    }
}

#[cfg(test)]
mod tests {
    use crate::GenericFamily;
    use bytemuck::{checked::try_from_bytes, Contiguous, Zeroable};
    use core::ptr;

    #[test]
    fn checked_bit_pattern() {
        let valid = bytemuck::bytes_of(&2_u8);
        let invalid = bytemuck::bytes_of(&200_u8);

        assert_eq!(
            Ok(&GenericFamily::Monospace),
            try_from_bytes::<GenericFamily>(valid)
        );

        assert!(try_from_bytes::<GenericFamily>(invalid).is_err());
    }

    #[test]
    fn contiguous() {
        let hd1 = GenericFamily::SansSerif;
        let hd2 = GenericFamily::from_integer(hd1.into_integer());
        assert_eq!(Some(hd1), hd2);

        assert_eq!(None, GenericFamily::from_integer(255));
    }

    #[test]
    fn zeroable() {
        let hd = GenericFamily::zeroed();
        assert_eq!(hd, GenericFamily::Serif);
    }

    /// Tests that the [`Contiguous`] impl for [`GenericFamily`] is not trivially incorrect.
    const _: () = {
        let mut value = 0;
        while value <= GenericFamily::MAX_VALUE {
            // Safety: In a const context, therefore if this makes an invalid GenericFamily, that will be detected.
            // When updating the MSRV to 1.82 or later, this can use `&raw const value` instead of the addr_of!
            let it: GenericFamily = unsafe { ptr::read((core::ptr::addr_of!(value)).cast()) };
            // Evaluate the enum value to ensure it actually has a valid tag
            if it as u8 != value {
                unreachable!();
            }
            value += 1;
        }
    };
}

#[cfg(doctest)]
/// Doctests aren't collected under `cfg(test)`; we can use `cfg(doctest)` instead
mod doctests {
    /// Validates that any new variants in `GenericFamily` has led to a change in the `Contiguous` impl.
    /// Note that to test this robustly, we'd need 256 tests, which is impractical.
    /// We make the assumption that all new variants will maintain contiguousness.
    ///
    /// ```compile_fail,E0080
    /// use bytemuck::Contiguous;
    /// use fontique::GenericFamily;
    /// const {
    ///     let value = GenericFamily::MAX_VALUE + 1;
    ///     // Safety: In a const context, therefore if this makes an invalid GenericFamily, that will be detected.
    ///     // (Indeed, we rely upon that)
    ///     // When updating the MSRV to 1.82 or later, this can use `&raw const value` instead of the addr_of!
    ///     let it: GenericFamily = unsafe { core::ptr::read((core::ptr::addr_of!(value)).cast()) };
    ///     // Evaluate the enum value to ensure it actually has an invalid tag
    ///     if it as u8 != value {
    ///         unreachable!();
    ///     }
    /// }
    /// ```
    const _GENERIC_FAMILY: () = {};
}
