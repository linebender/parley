// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! This implementation is based on [emoji segmenter]'s Ragel grammar (Apache-2.0).
//!
//! And follow the [UTS51](Unicode Technical Standard #51).
//!
//! [emoji segmenter]: <https://github.com/google/emoji-segmenter>
//! [UTS51]: <https://www.unicode.org/reports/tr51/>

/// Flags are used to identify [`EmojiSegmentationCategory`].
#[derive(Clone, Copy, Default)]
pub(crate) struct EmojiFlags(u32);

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
    pub(crate) const fn new() -> Self {
        Self(0)
    }

    #[inline]
    pub(crate) const fn with_emoji(mut self, is_emoji: bool) -> Self {
        self.0 |= (is_emoji as u32) << Self::EMOJI_SHIFT;
        self
    }

    #[inline]
    pub(crate) const fn with_extra(
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

/// Represents the category of an emoji segmentation.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum EmojiSegmentationCategory {
    Emoji = 0,
    EmojiTextPresentation,
    EmojiEmojiPresentation,
    EmojiModifierBaseText,
    EmojiModifierBaseEmoji,
    EmojiModifier,
    RegionalIndicator,
    KeycapBase,
    CombiningEnclosingKeycap,
    CombiningEnclosingCircleBackslash,
    Zwj,
    Vs15,
    Vs16,
    TagBase,
    TagSequence,
    TagTerm,
    None,
}

impl EmojiSegmentationCategory {
    /// Returns the category of the given codepoint and flags.
    #[inline]
    pub(crate) const fn from_codepoint(cp: u32, flags: EmojiFlags) -> Self {
        match cp {
            // '0'..'9', '#', '*'
            0x30..=0x39 | 0x23 | 0x2A => Self::KeycapBase,
            0x200D => Self::Zwj,
            0x20E0 => Self::CombiningEnclosingCircleBackslash,
            0x20E3 => Self::CombiningEnclosingKeycap,
            0xFE0E => Self::Vs15,
            0xFE0F => Self::Vs16,
            0x1F3F4 => Self::TagBase,
            0xE0030..=0xE0039 | 0xE0061..0xE007A => Self::TagSequence,
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
                    return Self::RegionalIndicator;
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

    const fn eq(self, other: Self) -> bool {
        self as u8 == other as u8
    }
}

/// Used to control the presentation style of the emoji.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub(crate) struct ScannedEmojiPresentation {
    pub is_emoji: bool,
    pub has_vs: bool,
}

impl ScannedEmojiPresentation {
    /// Returns true if the scanned sequence is an emoji presentation.
    pub(crate) fn is_emoji(self) -> bool {
        self.is_emoji
    }

    /// Clears the emoji presentation state.
    pub(crate) fn clear(&mut self) {
        self.is_emoji = false;
        self.has_vs = false;
    }
}

/// Scan the given categories for an emoji presentation sequence.
///
/// Returns a [`ScannedEmojiPresentation`] indicating whether the sequence is an emoji presentation.
pub(crate) const fn scan_emoji_presentation(
    categories: &[EmojiSegmentationCategory],
) -> ScannedEmojiPresentation {
    let len = categories.len();

    if len == 0 {
        return ScannedEmojiPresentation {
            is_emoji: false,
            has_vs: false,
        };
    }

    let (is_any_emoji, is_emoji_modifier_base, is_emoji_presentation) =
        emoji_matches(categories[0]);

    // text_emoji_run_with_vs
    let is_text_emoji_presentation_sequence =
        is_any_emoji && len >= 2 && categories[1].eq(EmojiSegmentationCategory::Vs15);
    if is_text_emoji_presentation_sequence && len == 2 || is_text_emoji_keycap_sequence(categories)
    {
        return ScannedEmojiPresentation {
            is_emoji: false,
            has_vs: true,
        };
    }

    // emoji_run
    if is_emoji_presentation && len == 1 {
        return ScannedEmojiPresentation {
            is_emoji: true,
            has_vs: false,
        };
    }

    let is_emoji_combining_enclosing_circle_backslash_sequence = is_any_emoji
        && len == 2
        && categories[1].eq(EmojiSegmentationCategory::CombiningEnclosingCircleBackslash);
    if is_emoji_combining_enclosing_circle_backslash_sequence {
        return ScannedEmojiPresentation {
            is_emoji: true,
            has_vs: false,
        };
    }

    if is_emoji_flag_sequence(categories) {
        return ScannedEmojiPresentation {
            is_emoji: true,
            has_vs: false,
        };
    }

    if is_emoji_tag_sequence(categories) {
        return ScannedEmojiPresentation {
            is_emoji: true,
            has_vs: false,
        };
    }

    // emoji_run_with_vs
    let is_emoji_presentation_sequence =
        is_any_emoji && len >= 2 && categories[1].eq(EmojiSegmentationCategory::Vs16);
    if (is_emoji_presentation_sequence && len == 2) || is_emoji_keycap_sequence(categories) {
        return ScannedEmojiPresentation {
            is_emoji: true,
            has_vs: true,
        };
    }

    let is_emoji_modifier_sequence = is_emoji_modifier_base
        && len >= 2
        && categories[1].eq(EmojiSegmentationCategory::EmojiModifier);
    if is_emoji_modifier_sequence && len == 2 {
        return ScannedEmojiPresentation {
            is_emoji: true,
            has_vs: false,
        };
    }

    let cursor = if is_emoji_presentation_sequence || is_emoji_modifier_sequence {
        2
    } else if is_any_emoji {
        1
    } else {
        len
    };

    // fast path
    if cursor == len {
        return ScannedEmojiPresentation {
            is_emoji: false,
            has_vs: is_text_emoji_presentation_sequence,
        };
    }

    if is_emoji_zwj_sequence(categories, cursor) {
        return ScannedEmojiPresentation {
            is_emoji: true,
            has_vs: false,
        };
    }

    if is_unqualified_keycap_sequence(categories) {
        return ScannedEmojiPresentation {
            is_emoji: true,
            has_vs: false,
        };
    }

    ScannedEmojiPresentation {
        is_emoji: is_emoji_presentation_sequence || is_emoji_modifier_sequence,
        has_vs: false,
    }
}

/// Extracts the emoji category flags from the given category.
///
/// - `is_any_emoji`:
///     `EmojiTextPresentation` | `EmojiEmojiPresentation` | `KeycapBase` |
///     `EmojiModifierBaseText` | `EmojiModifierBaseEmoji` | `TagBase` | `Emoji`
///
/// - `is_emoji_modifier_base`: `EmojiModifierBaseText` | `EmojiModifierBaseEmoji`
///
/// - `is_emoji_presentation`:
///     `EmojiEmojiPresentation` | `TagBase` | `EmojiModifierBaseEmoji` |
///     `EmojiModifier` | `RegionalIndicator`
///
/// Returns a tuple: `(is_any_emoji, is_emoji_modifier_base, is_emoji_presentation)`.
///
/// <https://unicode.org/reports/tr51/#Definitions>
#[inline]
const fn emoji_matches(category: EmojiSegmentationCategory) -> (bool, bool, bool) {
    use EmojiSegmentationCategory::*;
    match category {
        EmojiTextPresentation | KeycapBase | Emoji => (true, false, false),
        EmojiEmojiPresentation | TagBase => (true, false, true),
        EmojiModifierBaseText => (true, true, false),
        EmojiModifierBaseEmoji => (true, true, true),
        EmojiModifier | RegionalIndicator => (false, false, true),
        _ => (false, false, false),
    }
}

/// Text emoji keycap sequence.
///
/// This is a special case of text emoji presentation sequence.
#[inline]
const fn is_text_emoji_keycap_sequence(categories: &[EmojiSegmentationCategory]) -> bool {
    categories.len() == 3
        && categories[0].eq(EmojiSegmentationCategory::KeycapBase)
        && categories[1].eq(EmojiSegmentationCategory::Vs15)
        && categories[2].eq(EmojiSegmentationCategory::CombiningEnclosingKeycap)
}

/// Emoji flag sequence.
///
/// <https://unicode.org/reports/tr51/#def_emoji_flag_sequence>
#[inline]
const fn is_emoji_flag_sequence(categories: &[EmojiSegmentationCategory]) -> bool {
    categories.len() == 2
        && categories[0].eq(EmojiSegmentationCategory::RegionalIndicator)
        && categories[1].eq(EmojiSegmentationCategory::RegionalIndicator)
}

/// Emoji tag sequence (ETS).
///
/// <https://unicode.org/reports/tr51/#def_emoji_tag_sequence>
#[inline]
const fn is_emoji_tag_sequence(categories: &[EmojiSegmentationCategory]) -> bool {
    let is_tag_sequence = categories.len() >= 2
        && categories[0].eq(EmojiSegmentationCategory::TagBase)
        && categories[categories.len() - 1].eq(EmojiSegmentationCategory::TagTerm);

    let mut i = 1;
    while i < categories.len() - 1 {
        if !categories[i].eq(EmojiSegmentationCategory::TagSequence) {
            return false;
        }
        i += 1;
    }

    is_tag_sequence
}

/// Emoji keycap sequence.
///
/// <https://unicode.org/reports/tr51/#def_emoji_keycap_sequence>
#[inline]
const fn is_emoji_keycap_sequence(categories: &[EmojiSegmentationCategory]) -> bool {
    categories.len() == 3
        && categories[0].eq(EmojiSegmentationCategory::KeycapBase)
        && categories[1].eq(EmojiSegmentationCategory::Vs16)
        && categories[2].eq(EmojiSegmentationCategory::CombiningEnclosingKeycap)
}

/// Emoji ZWJ sequence.
///
/// <https://unicode.org/reports/tr51/#def_emoji_zwj_sequence>
const fn is_emoji_zwj_sequence(
    categories: &[EmojiSegmentationCategory],
    mut cursor: usize,
) -> bool {
    while cursor + 1 < categories.len() && categories[cursor].eq(EmojiSegmentationCategory::Zwj) {
        cursor += 1;

        let (is_any_emoji, is_emoji_modifier_base, _) = emoji_matches(categories[cursor]);

        if cursor + 1 < categories.len() {
            let is_emoji_presentation_sequence =
                is_any_emoji && categories[cursor + 1].eq(EmojiSegmentationCategory::Vs16);
            if is_emoji_presentation_sequence {
                cursor += 2;
                continue;
            }

            let is_emoji_modifier_sequence = is_emoji_modifier_base
                && categories[cursor + 1].eq(EmojiSegmentationCategory::EmojiModifier);
            if is_emoji_modifier_sequence {
                cursor += 2;
                continue;
            }
        }

        if is_any_emoji {
            cursor += 1;
        }
    }

    cursor == categories.len()
}

#[inline]
const fn is_unqualified_keycap_sequence(categories: &[EmojiSegmentationCategory]) -> bool {
    categories.len() == 2
        && categories[0].eq(EmojiSegmentationCategory::KeycapBase)
        && categories[1].eq(EmojiSegmentationCategory::CombiningEnclosingKeycap)
}
