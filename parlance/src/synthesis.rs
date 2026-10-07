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
/// assert_eq!(synthesis.variation_settings(), &[(Tag::new(b"wght"), 700.0)]);
/// assert_eq!(synthesis.skew(), Some(14.0));
/// assert!(!synthesis.embolden());
/// ```
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
    const WDTH: Tag = Tag::new(b"wdth");
    const WGHT: Tag = Tag::new(b"wght");
    const ITAL: Tag = Tag::new(b"ital");
    const SLNT: Tag = Tag::new(b"slnt");

    /// Sets the value to apply to the font's width (`wdth`) variation axis.
    #[inline]
    #[must_use]
    pub const fn with_width(self, width: f32) -> Self {
        self.with_variation(Self::WDTH, width)
    }

    /// Sets the value to apply to the font's weight (`wght`) variation axis.
    #[inline]
    #[must_use]
    pub const fn with_weight(self, weight: f32) -> Self {
        self.with_variation(Self::WGHT, weight)
    }

    /// Sets the font's italic (`ital`) variation axis to `1`.
    ///
    /// Italic and slant are alternative ways to synthesize a style, so this replaces a setting
    /// made by [`with_slant`](Self::with_slant).
    #[inline]
    #[must_use]
    pub const fn with_italic(self) -> Self {
        self.with_variation(Self::ITAL, 1.0)
    }

    /// Sets the value to apply to the font's slant (`slnt`) variation axis.
    ///
    /// Italic and slant are alternative ways to synthesize a style, so this replaces a setting
    /// made by [`with_italic`](Self::with_italic).
    #[inline]
    #[must_use]
    pub const fn with_slant(self, slant: f32) -> Self {
        self.with_variation(Self::SLNT, slant)
    }

    /// Sets whether the scaler should apply a faux bold.
    #[inline]
    #[must_use]
    pub const fn with_embolden(mut self, embolden: bool) -> Self {
        self.embolden = embolden;
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

    /// The position of a variation setting within `vars`. Width, weight and style each have one
    /// slot, kept in that order.
    const fn slot(tag: Tag) -> u8 {
        match tag.to_bytes() {
            [b'w', b'd', b't', b'h'] => 0,
            [b'w', b'g', b'h', b't'] => 1,
            _ => 2,
        }
    }

    /// Sets the variation setting of `tag`'s slot, replacing a previous setting of that slot.
    const fn with_variation(mut self, tag: Tag, value: f32) -> Self {
        let slot = Self::slot(tag);
        let len = self.len as usize;
        let mut index = 0;
        while index < len && Self::slot(self.vars[index].0) < slot {
            index += 1;
        }
        if index == len || Self::slot(self.vars[index].0) != slot {
            // The slot is not in use yet, so there is room for it. Make space at its position.
            let mut end = len;
            while end > index {
                self.vars[end] = self.vars[end - 1];
                end -= 1;
            }
            self.len += 1;
        }
        self.vars[index] = (tag, value);
        self
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
    const ITAL: Tag = Tag::new(b"ital");

    #[test]
    fn default_has_no_suggestions() {
        let synthesis = Synthesis::default();
        assert!(!synthesis.any());
        assert!(synthesis.variation_settings().is_empty());
        assert!(!synthesis.embolden());
        assert_eq!(synthesis.skew(), None);
    }

    #[test]
    fn variations_are_kept_in_slot_order() {
        let expected = [(WDTH, 75.0), (WGHT, 700.0), (SLNT, -14.0)];
        let synthesis = Synthesis::default()
            .with_width(75.0)
            .with_weight(700.0)
            .with_slant(-14.0);
        assert!(synthesis.any());
        assert_eq!(synthesis.variation_settings(), &expected);
        assert!(!synthesis.embolden());
        assert_eq!(synthesis.skew(), None);

        // The order the setters are called in does not matter.
        let reordered = Synthesis::default()
            .with_slant(-14.0)
            .with_weight(700.0)
            .with_width(75.0);
        assert_eq!(reordered.variation_settings(), &expected);
        assert_eq!(reordered, synthesis);

        let weight_and_style = Synthesis::default().with_italic().with_weight(700.0);
        assert_eq!(
            weight_and_style.variation_settings(),
            &[(WGHT, 700.0), (ITAL, 1.0)]
        );
    }

    #[test]
    fn setting_a_variation_again_replaces_it() {
        let synthesis = Synthesis::default()
            .with_width(75.0)
            .with_weight(400.0)
            .with_weight(700.0)
            .with_width(125.0);
        assert_eq!(
            synthesis.variation_settings(),
            &[(WDTH, 125.0), (WGHT, 700.0)]
        );
        assert_eq!(
            synthesis,
            Synthesis::default().with_width(125.0).with_weight(700.0)
        );
    }

    #[test]
    fn italic_and_slant_replace_each_other() {
        let all = Synthesis::default().with_width(75.0).with_weight(700.0);

        let slant = all.with_italic().with_slant(-14.0);
        assert_eq!(
            slant.variation_settings(),
            &[(WDTH, 75.0), (WGHT, 700.0), (SLNT, -14.0)]
        );
        assert_eq!(slant, all.with_slant(-14.0));

        let italic = all.with_slant(-14.0).with_italic();
        assert_eq!(
            italic.variation_settings(),
            &[(WDTH, 75.0), (WGHT, 700.0), (ITAL, 1.0)]
        );
        assert_eq!(italic, all.with_italic());
    }

    #[test]
    fn embolden() {
        let synthesis = Synthesis::default().with_embolden(true);
        assert!(synthesis.any());
        assert!(synthesis.embolden());
        assert!(synthesis.variation_settings().is_empty());
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
        assert_ne!(one, Synthesis::default());
    }
}
