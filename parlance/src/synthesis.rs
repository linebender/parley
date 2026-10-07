// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;

use crate::Tag;

/// Suggestions for synthesizing a set of font attributes for a given
/// font.
///
/// When the font selected for some requested attributes (width, weight and style) does not match
/// those attributes exactly, this describes how to make up the difference: variation settings to
/// apply to a variable font, and whether to apply a faux bold or a faux italic/oblique skew.
///
/// This is not a general-purpose container for font variation settings. It holds at most three,
/// which is enough for one setting each for width, weight and style.
#[derive(Copy, Clone, PartialEq)]
pub struct Synthesis {
    vars: [(Tag, f32); 3],
    len: u8,
    embolden: bool,
    skew: i8,
}

impl Default for Synthesis {
    fn default() -> Self {
        Self {
            vars: [(Tag::from_bytes([0; 4]), 0.0); 3],
            len: 0,
            embolden: false,
            skew: 0,
        }
    }
}

impl Synthesis {
    /// The maximum number of variation settings a `Synthesis` can hold.
    const MAX_VARIATIONS: usize = 3;

    /// Creates synthesis suggestions from their parts.
    ///
    /// - `variations` are the variation settings that should be applied to the font. At most
    ///   three are supported; this is enough for one setting each for width, weight and style.
    /// - `embolden` is whether a faux bold should be applied.
    /// - `skew_degrees` is the skew angle in degrees for a faux italic/oblique, with `0` meaning
    ///   no skew.
    ///
    /// Returns `None` if there are more than three `variations`.
    #[inline]
    pub fn try_new(variations: &[(Tag, f32)], embolden: bool, skew_degrees: i8) -> Option<Self> {
        if variations.len() > Self::MAX_VARIATIONS {
            return None;
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the length is at most `MAX_VARIATIONS`"
        )]
        let mut synthesis = Self {
            len: variations.len() as u8,
            embolden,
            skew: skew_degrees,
            ..Self::default()
        };
        synthesis.vars[..variations.len()].copy_from_slice(variations);
        Some(synthesis)
    }

    /// Returns `true` if any synthesis suggestions are available.
    #[inline]
    pub fn any(&self) -> bool {
        self.len != 0 || self.embolden || self.skew != 0
    }

    /// Returns the variation settings that should be applied to match the
    /// requested attributes.
    #[inline]
    pub fn variation_settings(&self) -> &[(Tag, f32)] {
        &self.vars[..self.len as usize]
    }

    /// Returns `true` if the scaler should apply a faux bold.
    #[inline]
    pub fn embolden(&self) -> bool {
        self.embolden
    }

    /// Returns a skew angle for faux italic/oblique, if requested.
    #[inline]
    pub fn skew(&self) -> Option<f32> {
        if self.skew != 0 {
            Some(self.skew as f32)
        } else {
            None
        }
    }
}

#[expect(
    clippy::missing_fields_in_debug,
    reason = "only the variation settings in use are shown"
)]
impl fmt::Debug for Synthesis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Synthesis")
            .field("vars", &self.variation_settings())
            .field("embolden", &self.embolden)
            .field("skew", &self.skew)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WDTH: Tag = Tag::new(b"wdth");
    const WGHT: Tag = Tag::new(b"wght");
    const SLNT: Tag = Tag::new(b"slnt");
    const OPSZ: Tag = Tag::new(b"opsz");

    #[test]
    fn default_has_no_suggestions() {
        let synthesis = Synthesis::default();
        assert!(!synthesis.any());
        assert!(synthesis.variation_settings().is_empty());
        assert!(!synthesis.embolden());
        assert_eq!(synthesis.skew(), None);
    }

    #[test]
    fn try_new_empty_is_default() {
        let synthesis = Synthesis::try_new(&[], false, 0).unwrap();
        assert_eq!(synthesis, Synthesis::default());
        assert!(!synthesis.any());
    }

    #[test]
    fn try_new_three_variations() {
        let variations = [(WDTH, 75.0), (WGHT, 700.0), (SLNT, -14.0)];
        let synthesis = Synthesis::try_new(&variations, false, 0).unwrap();
        assert!(synthesis.any());
        assert_eq!(synthesis.variation_settings(), &variations);
        assert!(!synthesis.embolden());
        assert_eq!(synthesis.skew(), None);
    }

    #[test]
    fn try_new_rejects_more_than_three_variations() {
        let variations = [(WDTH, 75.0), (WGHT, 700.0), (SLNT, -14.0), (OPSZ, 12.0)];
        assert_eq!(Synthesis::try_new(&variations, false, 0), None);
    }

    #[test]
    fn try_new_embolden() {
        let synthesis = Synthesis::try_new(&[], true, 0).unwrap();
        assert!(synthesis.any());
        assert!(synthesis.embolden());
        assert!(synthesis.variation_settings().is_empty());
        assert_eq!(synthesis.skew(), None);
    }

    #[test]
    fn try_new_skew() {
        let synthesis = Synthesis::try_new(&[], false, 14).unwrap();
        assert!(synthesis.any());
        assert_eq!(synthesis.skew(), Some(14.0));
        assert!(!synthesis.embolden());

        let synthesis = Synthesis::try_new(&[], false, -14).unwrap();
        assert!(synthesis.any());
        assert_eq!(synthesis.skew(), Some(-14.0));
    }

    #[test]
    fn equality() {
        let one = Synthesis::try_new(&[(WGHT, 700.0)], false, 0).unwrap();
        assert_eq!(one, Synthesis::try_new(&[(WGHT, 700.0)], false, 0).unwrap());
        assert_ne!(one, Synthesis::try_new(&[(WGHT, 400.0)], false, 0).unwrap());
        assert_ne!(one, Synthesis::default());
    }
}
