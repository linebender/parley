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

#[cfg(test)]
mod tests {
    use super::*;

    const WDTH: Tag = Tag::new(b"wdth");
    const WGHT: Tag = Tag::new(b"wght");
    const SLNT: Tag = Tag::new(b"slnt");
    const ITAL: Tag = Tag::new(b"ital");

    #[track_caller]
    fn assert_variation_settings(synthesis: Synthesis, expected: &[(Tag, f32)]) {
        let mut settings = synthesis.variation_settings();
        for setting in expected {
            assert_eq!(settings.next(), Some(*setting));
        }
        assert_eq!(settings.next(), None);
    }

    #[test]
    fn default_has_no_suggestions() {
        let synthesis = Synthesis::default();
        assert!(!synthesis.any());
        assert_variation_settings(synthesis, &[]);
        assert!(!synthesis.embolden());
        assert_eq!(synthesis.skew(), None);
    }

    #[test]
    fn variations_are_yielded_in_slot_order() {
        let expected = [(WDTH, 75.0), (WGHT, 700.0), (SLNT, -14.0)];
        let synthesis = Synthesis::default()
            .with_width(75.0)
            .with_weight(700.0)
            .with_slant(-14.0);
        assert!(synthesis.any());
        assert_variation_settings(synthesis, &expected);
        assert!(!synthesis.embolden());
        assert_eq!(synthesis.skew(), None);

        // The order the setters are called in does not matter.
        let reordered = Synthesis::default()
            .with_slant(-14.0)
            .with_weight(700.0)
            .with_width(75.0);
        assert_variation_settings(reordered, &expected);
        assert_eq!(reordered, synthesis);

        let weight_and_style = Synthesis::default().with_italic().with_weight(700.0);
        assert_variation_settings(weight_and_style, &[(WGHT, 700.0), (ITAL, 1.0)]);
    }

    #[test]
    fn single_variations() {
        let width = Synthesis::default().with_width(75.0);
        assert!(width.any());
        assert_variation_settings(width, &[(WDTH, 75.0)]);

        let slant = Synthesis::default().with_slant(-14.0);
        assert!(slant.any());
        assert_variation_settings(slant, &[(SLNT, -14.0)]);

        let italic = Synthesis::default().with_italic();
        assert!(italic.any());
        assert_variation_settings(italic, &[(ITAL, 1.0)]);
    }

    #[test]
    fn setting_a_variation_again_replaces_it() {
        let synthesis = Synthesis::default()
            .with_width(75.0)
            .with_weight(400.0)
            .with_weight(700.0)
            .with_width(125.0);
        assert_variation_settings(synthesis, &[(WDTH, 125.0), (WGHT, 700.0)]);
        assert_eq!(
            synthesis,
            Synthesis::default().with_width(125.0).with_weight(700.0)
        );
    }

    #[test]
    fn italic_and_slant_replace_each_other() {
        let all = Synthesis::default().with_width(75.0).with_weight(700.0);

        let slant = all.with_italic().with_slant(-14.0);
        assert_variation_settings(slant, &[(WDTH, 75.0), (WGHT, 700.0), (SLNT, -14.0)]);
        assert_eq!(slant, all.with_slant(-14.0));

        let italic = all.with_slant(-14.0).with_italic();
        assert_variation_settings(italic, &[(WDTH, 75.0), (WGHT, 700.0), (ITAL, 1.0)]);
        assert_eq!(italic, all.with_italic());
    }

    #[test]
    fn embolden() {
        let synthesis = Synthesis::default().with_embolden(true);
        assert!(synthesis.any());
        assert!(synthesis.embolden());
        assert_variation_settings(synthesis, &[]);
        assert_eq!(synthesis.skew(), None);

        assert_eq!(synthesis.with_embolden(false), Synthesis::default());
    }

    #[test]
    fn skew() {
        let synthesis = Synthesis::default().with_skew(14);
        assert!(synthesis.any());
        assert_eq!(synthesis.skew(), Some(14.0));
        assert!(!synthesis.embolden());

        let synthesis = Synthesis::default().with_skew(-14);
        assert!(synthesis.any());
        assert_eq!(synthesis.skew(), Some(-14.0));

        assert_eq!(synthesis.with_skew(0), Synthesis::default());
    }

    #[test]
    fn equality() {
        let one = Synthesis::default().with_weight(700.0);
        assert_eq!(one, Synthesis::default().with_weight(700.0));
        assert_ne!(one, Synthesis::default().with_weight(400.0));
        assert_ne!(one, Synthesis::default().with_width(700.0));
        assert_ne!(one, Synthesis::default().with_slant(700.0));
        assert_ne!(one, Synthesis::default());
        assert_ne!(
            Synthesis::default().with_italic(),
            Synthesis::default().with_slant(0.0)
        );
    }
}
