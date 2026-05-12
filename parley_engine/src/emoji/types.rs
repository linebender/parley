// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Flags are used to identify [`EmojiSegmentationCategory`].
#[derive(Clone, Copy, Default)]
pub struct EmojiFlags(u32);

impl EmojiFlags {
    const EMOJI_SHIFT: u32 = 0;
    const EMOJI_MODIFIER_SHIFT: u32 = 1;
    const EMOJI_MODIFIER_BASE_SHIFT: u32 = 2;
    const EMOJI_PRESENTATION_SHIFT: u32 = 3;
    const EMOJI_COMPONENT_SHIFT: u32 = 4;
    const REGIONAL_INDICATOR_SHIFT: u32 = 5;

    const EMOJI_MASK: u32 = 1 << Self::EMOJI_SHIFT;
    const EMOJI_MODIFIER_MASK: u32 = 1 << Self::EMOJI_MODIFIER_SHIFT;
    const EMOJI_MODIFIER_BASE_MASK: u32 = 1 << Self::EMOJI_MODIFIER_BASE_SHIFT;
    const EMOJI_PRESENTATION_MASK: u32 = 1 << Self::EMOJI_PRESENTATION_SHIFT;
    #[allow(unused)]
    const EMOJI_COMPONENT_MASK: u32 = 1 << Self::EMOJI_COMPONENT_SHIFT;
    const REGIONAL_INDICATOR_MASK: u32 = 1 << Self::REGIONAL_INDICATOR_SHIFT;

    #[inline]
    pub const fn new() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn with_emoji(mut self, is_emoji: bool) -> Self {
        self.0 |= (is_emoji as u32) << Self::EMOJI_SHIFT;
        self
    }

    #[inline]
    pub const fn with_extra(
        mut self,
        is_emoji_modifier: bool,
        is_emoji_modifier_base: bool,
        is_emoji_presentation: bool,
        is_emoji_component: bool,
        is_regional_indicator: bool,
    ) -> Self {
        self.0 |= (is_emoji_modifier as u32) << Self::EMOJI_MODIFIER_SHIFT;
        self.0 |= (is_emoji_modifier_base as u32) << Self::EMOJI_MODIFIER_BASE_SHIFT;
        self.0 |= (is_emoji_presentation as u32) << Self::EMOJI_PRESENTATION_SHIFT;
        self.0 |= (is_emoji_component as u32) << Self::EMOJI_COMPONENT_SHIFT;
        self.0 |= (is_regional_indicator as u32) << Self::REGIONAL_INDICATOR_SHIFT;
        self
    }

    #[inline]
    pub(crate) const fn is_emoji(self) -> bool {
        self.0 & Self::EMOJI_MASK != 0
    }

    #[inline]
    pub(crate) const fn is_emoji_modifier(self) -> bool {
        self.0 & Self::EMOJI_MODIFIER_MASK != 0
    }

    #[inline]
    pub(crate) const fn is_emoji_modifier_base(self) -> bool {
        self.0 & Self::EMOJI_MODIFIER_BASE_MASK != 0
    }

    #[inline]
    pub(crate) const fn is_emoji_presentation(self) -> bool {
        self.0 & Self::EMOJI_PRESENTATION_MASK != 0
    }

    #[allow(unused)]
    #[inline]
    pub(crate) const fn is_emoji_component(self) -> bool {
        self.0 & Self::EMOJI_COMPONENT_MASK != 0
    }

    #[inline]
    pub(crate) const fn is_regional_indicator(self) -> bool {
        self.0 & Self::REGIONAL_INDICATOR_MASK != 0
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum EmojiState {
    Reject = 0,
    Start,

    Terminal,
    Emoji,
    #[allow(unused)]
    EmojiModifier,
    EmojiModifierBaseText,
    EmojiModifierBaseEmoji,
    OptionalZwj,
    KeycapVs,
    TagBase,
    /// `RegionalIndicator`
    Ri,

    TagSpec,
    TagEmpty,
    KeycapBase,
    Zwj,
}

impl EmojiState {
    #[inline]
    pub(crate) const fn as_usize(self) -> usize {
        self as usize
    }

    #[inline]
    pub(crate) const fn as_u8(self) -> u8 {
        self as u8
    }

    #[inline]
    pub(crate) const fn eq(self, other: Self) -> bool {
        self.as_u8() == other.as_u8()
    }
}

impl<T> core::ops::Index<EmojiState> for [T] {
    type Output = T;

    #[inline]
    fn index(&self, index: EmojiState) -> &T {
        &self[index.as_usize()]
    }
}

impl<T> core::ops::IndexMut<EmojiState> for [T] {
    #[inline]
    fn index_mut(&mut self, index: EmojiState) -> &mut T {
        &mut self[index.as_usize()]
    }
}

/// Represents the category of an emoji segmentation.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EmojiSegmentationCategory {
    Emoji = 0,
    EmojiTextPresentation,
    EmojiEmojiPresentation,
    EmojiModifierBaseText,
    EmojiModifierBaseEmoji,
    EmojiModifier,
    /// `RegionalIndicator`
    Ri,
    KeycapBase,
    KeycapTerm,
    Zwj,
    Vs15,
    Vs16,
    TagBase,
    TagSpec,
    TagTerm,
    None,
}

impl EmojiSegmentationCategory {
    /// Returns the category of the given codepoint and flags.
    ///
    /// <https://unicode.org/reports/tr51/#Definitions>
    #[inline]
    pub fn from_codepoint(cp: u32, flags: EmojiFlags) -> Self {
        match cp {
            // '0'..'9', '#', '*'
            0x30..=0x39 | 0x23 | 0x2A => Self::KeycapBase,
            0x200D => Self::Zwj,
            0x20E3 => Self::KeycapTerm,
            0xFE0E => Self::Vs15,
            0xFE0F => Self::Vs16,
            0x1F3F4 => Self::TagBase,
            0xE0030..=0xE0039 | 0xE0061..=0xE007A => Self::TagSpec,
            0xE007F => Self::TagTerm,
            _ => {
                if flags.is_emoji_modifier_base() {
                    if flags.is_emoji_presentation() {
                        return Self::EmojiModifierBaseEmoji;
                    }
                    return Self::EmojiModifierBaseText;
                }

                if flags.is_emoji_modifier() {
                    return Self::EmojiModifier;
                }

                if flags.is_regional_indicator() {
                    return Self::Ri;
                }

                if flags.is_emoji_presentation() {
                    return Self::EmojiEmojiPresentation;
                }

                if flags.is_emoji() {
                    if !flags.is_emoji_presentation() {
                        return Self::EmojiTextPresentation;
                    }
                    return Self::Emoji;
                }

                Self::None
            }
        }
    }

    #[inline]
    pub(crate) const fn as_usize(self) -> usize {
        self as usize
    }

    #[inline]
    pub(crate) const fn as_u8(self) -> u8 {
        self as u8
    }

    #[inline]
    pub(crate) const fn eq(self, other: Self) -> bool {
        self.as_u8() == other.as_u8()
    }
}

impl<T> core::ops::Index<EmojiSegmentationCategory> for [T] {
    type Output = T;

    #[inline]
    fn index(&self, index: EmojiSegmentationCategory) -> &T {
        &self[index.as_usize()]
    }
}

impl<T> core::ops::IndexMut<EmojiSegmentationCategory> for [T] {
    #[inline]
    fn index_mut(&mut self, index: EmojiSegmentationCategory) -> &mut T {
        &mut self[index.as_usize()]
    }
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum EmojiSequence {
    Basic,
    Keycap,
    Modifier,
    Flag,
    Zwj,
    Tag,
}

impl EmojiSequence {
    #[inline]
    pub(crate) const fn as_u8(self) -> u8 {
        self as u8
    }

    #[inline]
    pub(crate) const fn eq(self, other: Self) -> bool {
        self.as_u8() == other.as_u8()
    }
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub enum EmojiPresentationStyle {
    Emoji,
    Text,
    #[default]
    Default,
}

impl EmojiPresentationStyle {
    #[inline]
    pub(crate) const fn is_emoji(self) -> bool {
        self.eq(Self::Emoji)
    }

    #[inline]
    pub(crate) const fn as_u8(self) -> u8 {
        self as u8
    }

    #[inline]
    pub(crate) const fn eq(self, other: Self) -> bool {
        self.as_u8() == other.as_u8()
    }
}
