// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::generated;

/// Emoji character properties relevant for text analysis.
#[derive(Clone, Copy, Debug)]
pub struct EmojiProperties(u32);

impl EmojiProperties {
    const EMOJI_SHIFT: u32 = 0;
    const EXTENDED_PICTOGRAPHIC_SHIFT: u32 = 1;
    const EMOJI_COMPONENT_SHIFT: u32 = 2;
    const EMOJI_PRESENTATION_SHIFT: u32 = 3;
    const EMOJI_MODIFIER_SHIFT: u32 = 4;
    const EMOJI_MODIFIER_BASE_SHIFT: u32 = 5;
    const REGIONAL_INDICATOR_SHIFT: u32 = 6;

    const EMOJI_MASK: u32 = 1 << Self::EMOJI_SHIFT;
    const EMOJI_PRESENTATION_MASK: u32 = 1 << Self::EMOJI_PRESENTATION_SHIFT;
    const EMOJI_MODIFIER_MASK: u32 = 1 << Self::EMOJI_MODIFIER_SHIFT;
    const EMOJI_MODIFIER_BASE_MASK: u32 = 1 << Self::EMOJI_MODIFIER_BASE_SHIFT;
    const REGIONAL_INDICATOR_MASK: u32 = 1 << Self::REGIONAL_INDICATOR_SHIFT;

    #[cfg(feature = "baked")]
    #[inline]
    /// Returns the properties for a given character.
    pub const fn get(ch: char) -> Self {
        Self(generated::emoji_composite_get(ch as u32))
    }

    /// Creates a new [`EmojiProperties`] from the given properties
    #[inline]
    pub const fn new(
        is_emoji: bool,
        is_extended_pictographic: bool,
        is_emoji_component: bool,
        is_emoji_presentation: bool,
        is_emoji_modifier: bool,
        is_emoji_modifier_base: bool,
        is_regional_indicator: bool,
    ) -> Self {
        Self(
            (is_emoji as u32) << Self::EMOJI_SHIFT
                | (is_extended_pictographic as u32) << Self::EXTENDED_PICTOGRAPHIC_SHIFT
                | (is_emoji_component as u32) << Self::EMOJI_COMPONENT_SHIFT
                | (is_emoji_presentation as u32) << Self::EMOJI_PRESENTATION_SHIFT
                | (is_emoji_modifier as u32) << Self::EMOJI_MODIFIER_SHIFT
                | (is_emoji_modifier_base as u32) << Self::EMOJI_MODIFIER_BASE_SHIFT
                | (is_regional_indicator as u32) << Self::REGIONAL_INDICATOR_SHIFT,
        )
    }

    /// Returns whether the character is an emoji.
    #[inline]
    pub const fn is_emoji(self) -> bool {
        self.0 & Self::EMOJI_MASK != 0
    }

    /// Returns whether the character is an extended pictographic.
    #[inline]
    pub const fn is_extended_pictographic(self) -> bool {
        self.0 & Self::EXTENDED_PICTOGRAPHIC_SHIFT != 0
    }

    /// Returns whether the character is an emoji component.
    #[inline]
    pub const fn is_emoji_component(self) -> bool {
        self.0 & Self::EMOJI_COMPONENT_SHIFT != 0
    }

    /// Returns whether the character is an emoji presentation.
    #[inline]
    pub const fn is_emoji_presentation(self) -> bool {
        self.0 & Self::EMOJI_PRESENTATION_MASK != 0
    }

    /// Returns whether the character is a modifier.
    #[inline]
    pub const fn is_emoji_modifier(self) -> bool {
        self.0 & Self::EMOJI_MODIFIER_MASK != 0
    }

    /// Returns whether the character is a modifier base.
    #[inline]
    pub const fn is_emoji_modifier_base(self) -> bool {
        self.0 & Self::EMOJI_MODIFIER_BASE_MASK != 0
    }

    /// Returns whether the character is a region indicator.
    #[inline]
    pub const fn is_regional_indicator(self) -> bool {
        self.0 & Self::REGIONAL_INDICATOR_MASK != 0
    }
}

impl From<EmojiProperties> for u32 {
    fn from(value: EmojiProperties) -> Self {
        value.0
    }
}
