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
/// This is not a general-purpose container for font variation settings. It holds at most three:
/// one each for width, weight and style.
///
/// ```
/// use parlance::{Synthesis, Tag};
///
/// let synthesis = Synthesis::default().with_weight(700.0).with_skew(14);
/// assert!(
///     synthesis
///         .variation_settings()
///         .eq([(Tag::new(b"wght"), 700.0)])
/// );
/// assert_eq!(synthesis.skew(), Some(14.0));
/// assert!(!synthesis.embolden());
/// ```
//
// The values of variation settings that are not set are kept at zero, so that equality can be
// derived.
#[derive(Copy, Clone, Default, PartialEq)]
pub struct Synthesis {
    /// The value for the `wdth` axis, if `HAS_WIDTH` is set.
    width: f32,
    /// The value for the `wght` axis, if `HAS_WEIGHT` is set.
    weight: f32,
    /// The value for the `slnt` axis, if `HAS_SLANT` is set.
    slant: f32,
    flags: u8,
    skew: i8,
}

impl Synthesis {
    const HAS_WIDTH: u8 = 1 << 0;
    const HAS_WEIGHT: u8 = 1 << 1;
    /// The `ital` axis is set to `1`. Mutually exclusive with `HAS_SLANT`.
    const HAS_ITALIC: u8 = 1 << 2;
    const HAS_SLANT: u8 = 1 << 3;
    const EMBOLDEN: u8 = 1 << 4;

    /// Sets the value to apply to the font's width (`wdth`) variation axis.
    #[inline]
    #[must_use]
    pub const fn with_width(mut self, width: f32) -> Self {
        self.width = width;
        self.flags |= Self::HAS_WIDTH;
        self
    }

    /// Sets the value to apply to the font's weight (`wght`) variation axis.
    #[inline]
    #[must_use]
    pub const fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight;
        self.flags |= Self::HAS_WEIGHT;
        self
    }

    /// Sets the font's italic (`ital`) variation axis to `1`.
    ///
    /// Italic and slant are alternative ways to synthesize a style, so this replaces a setting
    /// made by [`with_slant`](Self::with_slant).
    #[inline]
    #[must_use]
    pub const fn with_italic(mut self) -> Self {
        self.slant = 0.0;
        self.flags = (self.flags & !Self::HAS_SLANT) | Self::HAS_ITALIC;
        self
    }

    /// Sets the value to apply to the font's slant (`slnt`) variation axis.
    ///
    /// Italic and slant are alternative ways to synthesize a style, so this replaces a setting
    /// made by [`with_italic`](Self::with_italic).
    #[inline]
    #[must_use]
    pub const fn with_slant(mut self, slant: f32) -> Self {
        self.slant = slant;
        self.flags = (self.flags & !Self::HAS_ITALIC) | Self::HAS_SLANT;
        self
    }

    /// Sets whether the scaler should apply a faux bold.
    #[inline]
    #[must_use]
    pub const fn with_embolden(mut self, embolden: bool) -> Self {
        if embolden {
            self.flags |= Self::EMBOLDEN;
        } else {
            self.flags &= !Self::EMBOLDEN;
        }
        self
    }

    /// Sets the skew angle in degrees for a faux italic/oblique.
    ///
    /// An angle of `0` means no skew.
    #[inline]
    #[must_use]
    pub const fn with_skew(mut self, degrees: i8) -> Self {
        self.skew = degrees;
        self
    }

    /// Returns `true` if any synthesis suggestions are available.
    #[inline]
    pub fn any(&self) -> bool {
        self.flags != 0 || self.skew != 0
    }

    /// Returns the variation settings that should be applied to match the
    /// requested attributes.
    ///
    /// These are yielded in the order width (`wdth`), weight (`wght`), style (`ital` or `slnt`).
    #[inline]
    pub fn variation_settings(&self) -> impl Iterator<Item = (Tag, f32)> + Clone + use<> {
        let has = |flag: u8| self.flags & flag != 0;
        let style = if has(Self::HAS_ITALIC) {
            Some((Tag::new(b"ital"), 1.0))
        } else {
            has(Self::HAS_SLANT).then_some((Tag::new(b"slnt"), self.slant))
        };
        [
            has(Self::HAS_WIDTH).then_some((Tag::new(b"wdth"), self.width)),
            has(Self::HAS_WEIGHT).then_some((Tag::new(b"wght"), self.weight)),
            style,
        ]
        .into_iter()
        .flatten()
    }

    /// Returns `true` if the scaler should apply a faux bold.
    #[inline]
    pub fn embolden(&self) -> bool {
        self.flags & Self::EMBOLDEN != 0
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
    reason = "the fields are shown as the settings they represent"
)]
impl fmt::Debug for Synthesis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Vars(Synthesis);
        impl fmt::Debug for Vars {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_list().entries(self.0.variation_settings()).finish()
            }
        }

        f.debug_struct("Synthesis")
            .field("vars", &Vars(*self))
            .field("embolden", &self.embolden())
            .field("skew", &self.skew)
            .finish()
    }
}
