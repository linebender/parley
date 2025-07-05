// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Unicode Range property

use core::fmt;
use core::ops::Range;

/// A [Unicode Range]
///
/// Ranges, numbered from 0 to 127, are used to describe the subset of
/// Unicode over which a font is "functional". See [Unicode Range].
///
/// [Unicode Range]: https://learn.microsoft.com/en-us/typography/opentype/spec/os2#ur
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum UnicodeRange {
    BasicLatin = 0,
    Latin1Supplement = 1,
    LatinExtendedA = 2,
    LatinExtendedB = 3,
    IpaExtensions = 4,
    SpacingModifierLetters = 5,
    CombiningDiacriticalMarks = 6,
    GreekAndCoptic = 7,
    Coptic = 8,
    Cyrillic = 9,
    Armenian = 10,
    Hebrew = 11,
    Vai = 12,
    Arabic = 13,
    // TODO: the rest
    Arrows = 37,
    Dingbats = 47,
}

impl UnicodeRange {
    /// Convert from a `u8`
    ///
    /// # Example
    ///
    /// ```
    /// # use fontique::UnicodeRange;
    /// assert_eq!(UnicodeRange::Latin1Supplement as u8, 1);
    /// assert_eq!(UnicodeRange::from_u8(1), Some(UnicodeRange::Latin1Supplement));
    /// ```
    pub fn from_u8(n: u8) -> Option<Self> {
        use UnicodeRange::*;
        match n {
            0 => Some(BasicLatin),
            1 => Some(Latin1Supplement),
            2 => Some(LatinExtendedA),
            3 => Some(LatinExtendedB),
            4 => Some(IpaExtensions),
            5 => Some(SpacingModifierLetters),
            6 => Some(CombiningDiacriticalMarks),
            7 => Some(GreekAndCoptic),
            8 => Some(Coptic),
            9 => Some(Cyrillic),
            10 => Some(Armenian),
            11 => Some(Hebrew),
            12 => Some(Vai),
            13 => Some(Arabic),
            // TODO: the rest
            37 => Some(Arrows),
            47 => Some(Dingbats),
            _ => None,
        }
    }

    /// The unicode ranges represented
    #[allow(clippy::single_range_in_vec_init)]
    pub fn ranges(self) -> &'static [Range<u32>] {
        use UnicodeRange::*;
        match self {
            BasicLatin => &[0x0000..0x0080],
            Latin1Supplement => &[0x0080..0x0100],
            LatinExtendedA => &[0x0100..0x0180],
            LatinExtendedB => &[0x0180..0x0250],
            IpaExtensions => &[0x0250..0x02B0, 0x1D00..0x1DC0],
            SpacingModifierLetters => &[0x2B0..0x0300, 0xA700..0xA720],
            CombiningDiacriticalMarks => &[0x0300..0x0370, 0x1DC0..0x1E00],
            GreekAndCoptic => &[0x0370..0x0400],
            Coptic => &[0x2C80..0x2D00],
            Cyrillic => &[0x0400..0x0530, 0x2D20..0x2E00, 0xA640..0xA6A0],
            Armenian => &[0x0530..0x0590],
            Hebrew => &[0x0590..0600],
            Vai => &[0xA500..0xA640],
            Arabic => &[0x0600..0x0700, 0x0750..0x0780],
            Arrows => &[
                0x2190..0x2200,
                0x27F0..0x2800,
                0x2900..0x2980,
                0x2B00..0x2E00,
            ],
            Dingbats => &[0x2700..0x27C0],
        }
    }

    /// Test inclusion of a [`char`] or `u32` value
    ///
    /// # Example
    ///
    /// ```
    /// # use fontique::UnicodeRange;
    /// assert!(UnicodeRange::BasicLatin.contains('a' as u32));
    /// ```
    #[inline]
    pub fn contains(self, c: u32) -> bool {
        self.ranges().iter().any(|r| r.contains(&c))
    }

    /// Find the range for a [`char`] or `u32` value
    ///
    /// # Example
    ///
    /// ```
    /// # use fontique::UnicodeRange;
    /// assert_eq!(UnicodeRange::find('a' as u32), Some(UnicodeRange::BasicLatin));
    /// assert_eq!(UnicodeRange::find('ç' as u32), Some(UnicodeRange::Latin1Supplement));
    /// assert_eq!(UnicodeRange::find(0x1DA0), Some(UnicodeRange::IpaExtensions));
    /// assert_eq!(UnicodeRange::find(0x20_0000), None);
    /// ```
    pub fn find(c: u32) -> Option<Self> {
        let i = match MAP.binary_search_by_key(&c, |x| (*x) & 0x00FF_FFFF) {
            Ok(i) => i,
            Err(next) => next - 1,
        };
        Self::from_u8((MAP[i] >> 24) as u8)
    }
}

/// A bit-map of [`UnicodeRange`]s
///
/// This type represents the set of [`UnicodeRange`]s over which a font is
/// "functional".
///
/// The default instance is empty (contains no ranges).
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct UnicodeRanges(u128);

impl UnicodeRanges {
    /// Iterate over the ranges represented
    pub fn as_iter(self) -> impl Iterator<Item = UnicodeRange> {
        (0_u8..=127)
            .filter(move |n| self.0 & (1_u128 << n) != 0)
            .flat_map(UnicodeRange::from_u8)
    }

    /// Test inclusion of a [`UnicodeRange`]
    ///
    /// # Example
    ///
    /// ```
    /// # use fontique::{UnicodeRange, UnicodeRanges};
    /// let ranges: UnicodeRanges = [3, 0, 0, 0].into();
    /// assert!(ranges.contains(UnicodeRange::Latin1Supplement));
    /// assert!(!ranges.contains(UnicodeRange::LatinExtendedA));
    /// ```
    #[inline]
    pub fn contains(self, ur: UnicodeRange) -> bool {
        self.0 & (1_u128 << (ur as u8)) != 0
    }
}

impl fmt::Debug for UnicodeRanges {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UnicodeRanges(")?;
        let mut first = true;
        for ur in self.as_iter() {
            if !first {
                write!(f, ", ")?;
            }
            write!(f, "{ur:?}")?;
            first = false;
        }
        write!(f, ")")
    }
}

impl From<[u32; 4]> for UnicodeRanges {
    #[inline]
    fn from(a: [u32; 4]) -> Self {
        Self((a[3] as u128) << 96 | (a[2] as u128) << 64 | (a[1] as u128) << 32 | (a[0] as u128))
    }
}

// A mapping from the start of a range to the corresponding UnicodeRange, if
// any. Entries are ordered. Unmapped char ranges map to None.
// Top 8 bits represent the UnicodeRange, bottom 24 the `char`.
#[allow(clippy::identity_op)]
const MAP: [u32; 27] = [
    0x00_0000 | (UnicodeRange::BasicLatin as u32) << 24,
    0x00_0080 | (UnicodeRange::Latin1Supplement as u32) << 24,
    0x00_0100 | (UnicodeRange::LatinExtendedA as u32) << 24,
    0x00_0180 | (UnicodeRange::LatinExtendedB as u32) << 24,
    0x00_0250 | (UnicodeRange::IpaExtensions as u32) << 24,
    0x00_02B0 | (UnicodeRange::SpacingModifierLetters as u32) << 24,
    0x00_0300 | (UnicodeRange::CombiningDiacriticalMarks as u32) << 24,
    0x00_0370 | (UnicodeRange::GreekAndCoptic as u32) << 24,
    0x00_0400 | (UnicodeRange::Cyrillic as u32) << 24,
    0x00_0530 | (UnicodeRange::Armenian as u32) << 24,
    0x00_0590 | (UnicodeRange::Hebrew as u32) << 24,
    0x00_0600 | (UnicodeRange::Arabic as u32) << 24,
    0x00_0700 | 255 << 24,
    0x00_0750 | (UnicodeRange::Arabic as u32) << 24,
    0x00_0780 | 255 << 24,
    0x00_1D00 | (UnicodeRange::IpaExtensions as u32) << 24,
    0x00_1DC0 | 255 << 24,
    0x00_2190 | (UnicodeRange::Arrows as u32) << 24,
    0x00_2200 | 255 << 24,
    0x00_2700 | (UnicodeRange::Dingbats as u32) << 24,
    0x00_27C0 | 255 << 24,
    0x00_27F0 | (UnicodeRange::Arrows as u32) << 24,
    0x00_2800 | 255 << 24,
    0x00_2900 | (UnicodeRange::Arrows as u32) << 24,
    0x00_2980 | 255 << 24,
    0x00_2B00 | (UnicodeRange::Arrows as u32) << 24,
    0x00_2E00 | 255 << 24,
    // TODO: the rest
];

#[cfg(test)]
#[test]
fn is_sorted() {
    assert!(MAP.is_sorted_by_key(|x| (*x) & 0x00FF_FFFF));
}
