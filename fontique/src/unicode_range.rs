// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Unicode Range property

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
    // TODO: the rest
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
            // TODO: the rest
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

// A mapping from the start of a range to the corresponding UnicodeRange, if
// any. Entries are ordered. Unmapped char ranges map to None.
// Top 8 bits represent the UnicodeRange, bottom 24 the `char`.
#[allow(clippy::identity_op)]
const MAP: [u32; 8] = [
    0x00_0000 | (UnicodeRange::BasicLatin as u32) << 24,
    0x00_0080 | (UnicodeRange::Latin1Supplement as u32) << 24,
    0x00_0100 | (UnicodeRange::LatinExtendedA as u32) << 24,
    0x00_0180 | (UnicodeRange::LatinExtendedB as u32) << 24,
    0x00_0250 | (UnicodeRange::IpaExtensions as u32) << 24,
    0x00_02B0 | 255 << 24,
    0x00_1D00 | (UnicodeRange::IpaExtensions as u32) << 24,
    0x00_1DC0 | 255 << 24,
    // TODO: the rest
];
