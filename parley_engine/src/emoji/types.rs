// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use parley_data::emoji::EmojiProperties;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum EmojiState {
    Reject = 0,
    Start,

    Terminal,
    Emoji,
    EmojiModifierBase,
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
    pub(crate) const fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Start,
            2 => Self::Terminal,
            3 => Self::Emoji,
            4 => Self::EmojiModifierBase,
            5 => Self::OptionalZwj,
            6 => Self::KeycapVs,
            7 => Self::TagBase,
            8 => Self::Ri,
            9 => Self::TagSpec,
            10 => Self::TagEmpty,
            11 => Self::KeycapBase,
            12 => Self::Zwj,
            _ => Self::Reject,
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
    EmojiPresentation,
    EmojiModifier,
    EmojiModifierBase,
    KeycapBase,
    KeycapEnd,
    TagBase,
    TagSpec,
    TagEnd,
    /// `RegionalIndicator`
    Ri,
    Vs15,
    Vs16,
    Zwj,
    None,
}

impl EmojiSegmentationCategory {
    /// Returns the category of the given codepoint and flags.
    ///
    /// <https://unicode.org/reports/tr51/#Definitions>
    #[inline]
    pub fn from_codepoint(cp: u32, properties: EmojiProperties) -> Self {
        match cp {
            // '0'..'9', '#', '*'
            0x30..=0x39 | 0x23 | 0x2A => Self::KeycapBase,
            0x200D => Self::Zwj,
            0x20E3 => Self::KeycapEnd,
            0xFE0E => Self::Vs15,
            0xFE0F => Self::Vs16,
            0x1F3F4 => Self::TagBase,
            0xE0030..=0xE0039 | 0xE0061..=0xE007A => Self::TagSpec,
            0xE007F => Self::TagEnd,
            _ => {
                if properties.is_regional_indicator() {
                    return Self::Ri;
                }

                if properties.is_emoji_modifier_base() {
                    return Self::EmojiModifierBase;
                }

                if properties.is_emoji_modifier() {
                    return Self::EmojiModifier;
                }

                if properties.is_emoji_presentation() {
                    return Self::EmojiPresentation;
                }

                if properties.is_emoji() {
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
