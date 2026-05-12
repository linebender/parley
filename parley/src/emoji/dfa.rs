// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::types::{EmojiPresentationStyle, EmojiSegmentationCategory, EmojiSequence, EmojiState};

/// The transition table for Emoji DFA.
///
/// <https://unicode.org/reports/tr51/#Definitions>
static DFA_TRANS: [[EmojiState; 16]; 15] = {
    use EmojiSegmentationCategory as Category;
    use EmojiState as State;

    let mut t = [[State::Reject; 16]; 15];

    {
        t[State::Start.as_usize()][Category::None.as_usize()] = State::Start;
        t[State::Start.as_usize()][Category::KeycapTerm.as_usize()] = State::Start;
        t[State::Start.as_usize()][Category::Zwj.as_usize()] = State::Start;
        t[State::Start.as_usize()][Category::Vs15.as_usize()] = State::Start;
        t[State::Start.as_usize()][Category::Vs16.as_usize()] = State::Start;
        t[State::Start.as_usize()][Category::TagSpec.as_usize()] = State::Start;
        t[State::Start.as_usize()][Category::TagTerm.as_usize()] = State::Start;
    }

    // Text and Emoji presentation sequences
    {
        t[State::Start.as_usize()][Category::Emoji.as_usize()] = State::Emoji;

        t[State::Start.as_usize()][Category::EmojiTextPresentation.as_usize()] = State::Emoji;
        t[State::Start.as_usize()][Category::EmojiEmojiPresentation.as_usize()] = State::Emoji;

        // Text presentation sequence
        //
        // <https://unicode.org/reports/tr51/#def_text_presentation_sequence>
        t[State::Emoji.as_usize()][Category::Vs15.as_usize()] = State::Terminal;

        // Emoji presentation sequence
        //
        // <https://unicode.org/reports/tr51/#def_emoji_presentation_sequence>
        t[State::Emoji.as_usize()][Category::Vs16.as_usize()] = State::OptionalZwj;

        // ZWJ
        t[State::Emoji.as_usize()][Category::Zwj.as_usize()] = State::Zwj;
    }

    // Emoji modifier sequence
    //
    // <https://unicode.org/reports/tr51/#def_emoji_modifier_sequence>
    {
        // text
        t[State::Start.as_usize()][Category::EmojiModifierBaseText.as_usize()] =
            State::EmojiModifierBaseText;

        t[State::EmojiModifierBaseText.as_usize()][Category::Vs15.as_usize()] = State::Terminal;
        t[State::EmojiModifierBaseText.as_usize()][Category::Vs16.as_usize()] = State::Terminal;
        t[State::EmojiModifierBaseText.as_usize()][Category::EmojiModifier.as_usize()] =
            State::OptionalZwj;

        // emoji
        t[State::Start.as_usize()][Category::EmojiModifierBaseEmoji.as_usize()] =
            State::EmojiModifierBaseEmoji;

        t[State::EmojiModifierBaseEmoji.as_usize()][Category::Vs16.as_usize()] = State::OptionalZwj;
        t[State::EmojiModifierBaseEmoji.as_usize()][Category::Zwj.as_usize()] = State::Zwj;
        t[State::EmojiModifierBaseEmoji.as_usize()][Category::EmojiModifier.as_usize()] =
            State::OptionalZwj;

        // other
        t[State::Start.as_usize()][Category::EmojiModifier.as_usize()] = State::Terminal;
    }

    // Emoji flag sequence -- A sequence of two Regional Indicator characters.
    //
    // <https://unicode.org/reports/tr51/#def_emoji_flag_sequence>
    {
        t[State::Start.as_usize()][Category::Ri.as_usize()] = State::Ri;

        t[State::Ri.as_usize()][Category::Ri.as_usize()] = State::Terminal;
    }

    // Emoji tag sequence (ETS).
    //
    // <https://unicode.org/reports/tr51/#def_emoji_tag_sequence>
    {
        t[State::Start.as_usize()][Category::TagBase.as_usize()] = State::TagBase;

        t[State::TagBase.as_usize()][Category::Vs15.as_usize()] = State::Terminal;
        t[State::TagBase.as_usize()][Category::Vs16.as_usize()] = State::OptionalZwj;
        t[State::TagBase.as_usize()][Category::TagSpec.as_usize()] = State::TagSpec;
        t[State::TagBase.as_usize()][Category::TagTerm.as_usize()] = State::TagEmpty; // without any `TagSpec`
        t[State::TagBase.as_usize()][Category::Zwj.as_usize()] = State::Zwj;

        // (seq)+
        t[State::TagSpec.as_usize()][Category::TagSpec.as_usize()] = State::TagSpec;
        t[State::TagSpec.as_usize()][Category::TagTerm.as_usize()] = State::Terminal;
    }

    // Emoji keycap sequence.
    //
    // <https://unicode.org/reports/tr51/#def_emoji_keycap_sequence>
    {
        t[State::Start.as_usize()][Category::KeycapBase.as_usize()] = State::KeycapBase;

        t[State::KeycapBase.as_usize()][Category::KeycapTerm.as_usize()] = State::Terminal;
        t[State::KeycapBase.as_usize()][Category::Vs15.as_usize()] = State::KeycapVs;
        t[State::KeycapBase.as_usize()][Category::Vs16.as_usize()] = State::KeycapVs;

        t[State::KeycapVs.as_usize()][Category::KeycapTerm.as_usize()] = State::Terminal;
    }

    // Emoji ZWJ sequence.
    //
    // <https://unicode.org/reports/tr51/#def_emoji_zwj_sequence>
    {
        t[State::OptionalZwj.as_usize()][Category::Zwj.as_usize()] = State::Zwj;

        // (zwj emoji_zwj_element)+
        t[State::Zwj.as_usize()][Category::Emoji.as_usize()] = State::Emoji;
        t[State::Zwj.as_usize()][Category::EmojiEmojiPresentation.as_usize()] = State::Emoji;
        t[State::Zwj.as_usize()][Category::EmojiModifierBaseEmoji.as_usize()] =
            State::EmojiModifierBaseEmoji;
    }

    t
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct EmojiDFA {
    state: EmojiState,
    // [state, category]
    recorded: [u16; 2],
}

impl EmojiDFA {
    const DEFAULT: Self = Self {
        state: EmojiState::Start,
        recorded: [0, 0],
    };

    #[inline]
    pub(crate) const fn new() -> Self {
        Self::DEFAULT
    }

    #[inline]
    pub(crate) const fn step(&mut self, category: EmojiSegmentationCategory) {
        self.state = DFA_TRANS[self.state.as_usize()][category.as_usize()];
    }

    #[inline]
    pub(crate) const fn step_record(&mut self, category: EmojiSegmentationCategory) {
        self.step(category);

        if self.is_rejected() || self.is_started() {
            return;
        }

        self.recorded[0] |= 1 << self.state.as_u8();
        self.recorded[1] |= 1 << category.as_u8();
    }

    #[inline]
    pub(crate) const fn is_rejected(&self) -> bool {
        self.state.eq(EmojiState::Reject)
    }

    #[inline]
    pub(crate) const fn is_started(&self) -> bool {
        self.state.eq(EmojiState::Start)
    }

    #[allow(unused)]
    #[inline]
    pub(crate) const fn is_accepting(&self) -> bool {
        const START: u8 = EmojiState::Terminal.as_u8();
        const END: u8 = EmojiState::Ri.as_u8();

        let cur = self.state.as_u8();

        START <= cur && cur <= END
    }

    #[inline]
    pub(crate) const fn contains_state(&self, state: EmojiState) -> bool {
        self.recorded[0] & (1 << state.as_u8()) != 0
    }

    #[inline]
    pub(crate) const fn contains_category(&self, category: EmojiSegmentationCategory) -> bool {
        self.recorded[1] & (1 << category.as_u8()) != 0
    }

    #[inline]
    pub(crate) const fn sequence(&self) -> EmojiSequence {
        if self.contains_category(EmojiSegmentationCategory::Zwj) {
            return EmojiSequence::Zwj;
        }

        if self.contains_state(EmojiState::TagBase)
            && self.contains_state(EmojiState::Terminal)
            && !self.contains_category(EmojiSegmentationCategory::Vs15)
        {
            return EmojiSequence::Tag;
        }

        if self.contains_state(EmojiState::Ri) && self.contains_state(EmojiState::Terminal) {
            return EmojiSequence::Flag;
        }

        if (self.contains_category(EmojiSegmentationCategory::EmojiModifierBaseEmoji)
            || self.contains_category(EmojiSegmentationCategory::EmojiModifierBaseText))
            && self.contains_category(EmojiSegmentationCategory::EmojiModifier)
        {
            return EmojiSequence::Modifier;
        }

        if self.contains_category(EmojiSegmentationCategory::KeycapBase)
            && self.contains_category(EmojiSegmentationCategory::Vs16)
            && self.contains_category(EmojiSegmentationCategory::KeycapTerm)
        {
            return EmojiSequence::Keycap;
        }

        if self.contains_category(EmojiSegmentationCategory::KeycapTerm)
            && self.contains_category(EmojiSegmentationCategory::Vs16)
        {
            return EmojiSequence::Keycap;
        }

        EmojiSequence::Basic
    }

    #[inline]
    pub(crate) const fn presentation_style(&self) -> EmojiPresentationStyle {
        if self.contains_category(EmojiSegmentationCategory::Vs15) {
            return EmojiPresentationStyle::Text;
        }
        if self.contains_category(EmojiSegmentationCategory::Vs16) {
            return EmojiPresentationStyle::Emoji;
        }

        if self.contains_category(EmojiSegmentationCategory::EmojiTextPresentation) {
            return EmojiPresentationStyle::Text;
        }
        if self.contains_category(EmojiSegmentationCategory::EmojiEmojiPresentation) {
            return EmojiPresentationStyle::Emoji;
        }

        if !self.sequence().eq(EmojiSequence::Basic) {
            return EmojiPresentationStyle::Emoji;
        }

        // single emoji character
        if self.contains_category(EmojiSegmentationCategory::EmojiModifierBaseText) {
            return EmojiPresentationStyle::Text;
        }

        EmojiPresentationStyle::Default
    }
}
