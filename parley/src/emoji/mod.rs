//! The Core algorithm is based on [Emoji Segmenter]'s Ragel grammar.
//!
//! Follow the [UTS51](Unicode Technical Standard #51).
//!
//! [Emoji Segmenter]: <https://github.com/google/emoji-segmenter>
//! [TR51]: <https://www.unicode.org/reports/tr51/>

#[derive(Clone, Copy, Default, Debug)]
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

    #[inline(always)]
    pub(crate) const fn new() -> Self {
        Self(0)
    }

    #[inline(always)]
    pub(crate) const fn with_emoji(mut self, is_emoji: bool) -> Self {
        self.0 |= (is_emoji as u32) << Self::EMOJI_SHIFT;
        self
    }

    #[inline(always)]
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

    #[inline(always)]
    pub(crate) const fn is_emoji(&self) -> bool {
        self.0 & Self::EMOJI_MASK != 0
    }

    #[inline(always)]
    pub(crate) const fn is_emoji_modifier(&self) -> bool {
        self.0 & Self::EMOJI_MODIFIER_MASK != 0
    }

    #[inline(always)]
    pub(crate) const fn is_emoji_modifier_base(&self) -> bool {
        self.0 & Self::EMOJI_MODIFIER_BASE_MASK != 0
    }

    #[inline(always)]
    pub(crate) const fn is_emoji_presentation(&self) -> bool {
        self.0 & Self::EMOJI_PRESENTATION_MASK != 0
    }

    #[allow(unused)]
    #[inline(always)]
    pub(crate) const fn is_emoji_component(&self) -> bool {
        self.0 & Self::EMOJI_COMPONENT_MASK != 0
    }

    #[inline(always)]
    pub(crate) const fn is_regional_indicator(&self) -> bool {
        self.0 & Self::REGIONAL_INDICATOR_MASK != 0
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    #[inline(always)]
    pub(crate) const fn from_codepoint(cp: u32, flags: EmojiFlags) -> Self {
        match cp {
            // '0'..'9', '#', '*'
            0x30..=0x39 | 0x23 | 0x2a => Self::KeycapBase,
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

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub(crate) struct ScannedEmojiPresetation {
    pub is_emoji: bool,
    pub has_vs: bool,
}

impl ScannedEmojiPresetation {
    pub(crate) fn is_emoji(&self) -> bool {
        self.is_emoji
    }

    pub(crate) fn clear(&mut self) {
        self.is_emoji = false;
        self.has_vs = false;
    }
}

pub(crate) const fn scan_emoji_presetation(
    categories: &[EmojiSegmentationCategory],
) -> ScannedEmojiPresetation {
    let len = categories.len();

    if len == 0 {
        return ScannedEmojiPresetation {
            is_emoji: false,
            has_vs: false,
        };
    }

    let (is_any_emoji, is_emoji_modifier_base, is_emoji_presentation) =
        emoji_matches(categories[0]);

    // In order to give the the VS15 sequences higher priority than detecting
    //
    // text_emoji_run_with_vs
    let is_text_emoji_presentation_sequence =
        is_any_emoji && len >= 2 && EmojiSegmentationCategory::Vs15.eq(categories[1]);
    if is_text_emoji_presentation_sequence && len == 2 || is_text_emoji_keycap_sequence(categories)
    {
        return ScannedEmojiPresetation {
            is_emoji: false,
            has_vs: true,
        };
    }

    // emoji_run
    if is_emoji_presentation && len == 1 {
        return ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        };
    }

    if is_unqualified_keycap_sequence(categories) {
        return ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        };
    }

    let is_emoji_combining_enclosing_circle_backslash_sequence = is_any_emoji
        && len == 2
        && EmojiSegmentationCategory::CombiningEnclosingCircleBackslash.eq(categories[1]);
    if is_emoji_combining_enclosing_circle_backslash_sequence {
        return ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        };
    }

    if is_emoji_flag_sequence(categories) {
        return ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        };
    }

    // TAG_BASE TAG_SEQUENCE+ TAG_TERM;
    if is_emoji_tag_sequence(categories) {
        return ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        };
    }

    // emoji_run_with_vs
    let is_emoji_presentation_sequence =
        is_any_emoji && len >= 2 && EmojiSegmentationCategory::Vs16.eq(categories[1]);
    if (is_emoji_presentation_sequence && len == 2) || is_emoji_keycap_sequence(categories) {
        return ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: true,
        };
    }

    let is_emoji_modifier_sequence = is_emoji_modifier_base
        && len >= 2
        && EmojiSegmentationCategory::EmojiModifier.eq(categories[1]);
    if is_emoji_modifier_sequence && len == 2 {
        return ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        };
    }

    let mut cursor = if is_emoji_presentation_sequence || is_emoji_modifier_sequence {
        2
    } else if is_any_emoji {
        1
    } else {
        len
    };

    // fast path
    if cursor == len {
        return ScannedEmojiPresetation {
            is_emoji: false,
            has_vs: is_text_emoji_presentation_sequence,
        };
    }

    // zwj sequences
    //
    // emoji_zwj_element = emoji_presentation_sequence | emoji_modifier_sequence | any_emoji
    // emoji_zwj_element (zwj emoji_zwj_element)+
    while cursor < len && EmojiSegmentationCategory::Zwj.eq(categories[cursor]) {
        cursor += 1;

        let (is_any_emoji, is_emoji_modifier_base, _) = emoji_matches(categories[cursor]);

        if cursor + 1 < len {
            let is_emoji_presentation_sequence =
                is_any_emoji && EmojiSegmentationCategory::Vs16.eq(categories[cursor + 1]);
            if is_emoji_presentation_sequence {
                cursor += 2;
                continue;
            }

            let is_emoji_modifier_sequence = is_emoji_modifier_base
                && EmojiSegmentationCategory::EmojiModifier.eq(categories[cursor + 1]);
            if is_emoji_modifier_sequence {
                cursor += 2;
                continue;
            }
        }

        if is_any_emoji {
            cursor += 1;
            continue;
        }
    }

    ScannedEmojiPresetation {
        is_emoji: cursor == len || is_emoji_presentation_sequence || is_emoji_modifier_sequence,
        has_vs: false,
    }
}

/// Extracts the emoji category flags from the given category.
///
/// `is_any_emoji`:
///     EmojiTextPresentation | EmojiEmojiPresentation | KeycapBase |
///     EmojiModifierBaseText | EmojiModifierBaseEmoji | TagBase | Emoji
///
/// `is_emoji_modifier_base`: EmojiModifierBaseText | EmojiModifierBaseEmoji
///
/// `is_emoji_presentation`:
///     EmojiEmojiPresentation | TagBase | EmojiModifierBaseEmoji |
///     EmojiModifier | RegionalIndicator
///
/// Returns `(is_any_emoji, is_emoji_modifier_base, is_emoji_presentation)`
#[inline(always)]
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

#[inline(always)]
const fn is_text_emoji_keycap_sequence(categories: &[EmojiSegmentationCategory]) -> bool {
    categories.len() == 3
        && EmojiSegmentationCategory::KeycapBase.eq(categories[0])
        && EmojiSegmentationCategory::Vs15.eq(categories[1])
        && EmojiSegmentationCategory::CombiningEnclosingKeycap.eq(categories[2])
}

#[inline(always)]
const fn is_emoji_keycap_sequence(categories: &[EmojiSegmentationCategory]) -> bool {
    categories.len() == 3
        && EmojiSegmentationCategory::KeycapBase.eq(categories[0])
        && EmojiSegmentationCategory::Vs16.eq(categories[1])
        && EmojiSegmentationCategory::CombiningEnclosingKeycap.eq(categories[2])
}

#[inline(always)]
const fn is_emoji_flag_sequence(categories: &[EmojiSegmentationCategory]) -> bool {
    categories.len() == 2
        && EmojiSegmentationCategory::RegionalIndicator.eq(categories[0])
        && EmojiSegmentationCategory::RegionalIndicator.eq(categories[1])
}

#[inline(always)]
const fn is_emoji_tag_sequence(categories: &[EmojiSegmentationCategory]) -> bool {
    let is_tag_sequence = categories.len() >= 2
        && EmojiSegmentationCategory::TagBase.eq(categories[0])
        && EmojiSegmentationCategory::TagTerm.eq(categories[categories.len() - 1]);

    let mut i = 1;
    while i < categories.len() - 1 {
        if !EmojiSegmentationCategory::TagSequence.eq(categories[i]) {
            return false;
        }
        i += 1;
    }

    is_tag_sequence
}

#[inline(always)]
const fn is_unqualified_keycap_sequence(categories: &[EmojiSegmentationCategory]) -> bool {
    categories.len() == 2
        && EmojiSegmentationCategory::KeycapBase.eq(categories[0])
        && EmojiSegmentationCategory::CombiningEnclosingKeycap.eq(categories[1])
}
