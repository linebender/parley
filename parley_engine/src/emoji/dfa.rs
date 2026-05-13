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

    // Add a state transition to the DFA transition table.
    macro_rules! add {
        ($state:expr, $category:expr, $next_state:expr) => {
            t[$state.as_usize()][$category.as_usize()] = $next_state
        };
    }

    {
        add!(State::Start, Category::None, State::Start);
        add!(State::Start, Category::KeycapTerm, State::Start);
        add!(State::Start, Category::Zwj, State::Start);
        add!(State::Start, Category::Vs15, State::Start);
        add!(State::Start, Category::Vs16, State::Start);
        add!(State::Start, Category::TagSpec, State::Start);
        add!(State::Start, Category::TagTerm, State::Start);
    }

    // Text and Emoji presentation sequences
    {
        add!(State::Start, Category::Emoji, State::Emoji);

        add!(State::Start, Category::EmojiTextPresentation, State::Emoji);
        add!(State::Start, Category::EmojiEmojiPresentation, State::Emoji);

        // Text presentation sequence
        //
        // <https://unicode.org/reports/tr51/#def_text_presentation_sequence>
        add!(State::Emoji, Category::Vs15, State::Terminal);

        // Emoji presentation sequence
        //
        // <https://unicode.org/reports/tr51/#def_emoji_presentation_sequence>
        add!(State::Emoji, Category::Vs16, State::OptionalZwj);

        // ZWJ
        add!(State::Emoji, Category::Zwj, State::Zwj);
    }

    // Emoji modifier sequence
    //
    // <https://unicode.org/reports/tr51/#def_emoji_modifier_sequence>
    {
        // text
        add!(
            State::Start,
            Category::EmojiModifierBaseText,
            State::EmojiModifierBaseText
        );

        add!(
            State::EmojiModifierBaseText,
            Category::Vs15,
            State::Terminal
        );
        add!(
            State::EmojiModifierBaseText,
            Category::Vs16,
            State::Terminal
        );
        add!(
            State::EmojiModifierBaseText,
            Category::EmojiModifier,
            State::OptionalZwj
        );

        // emoji
        add!(
            State::Start,
            Category::EmojiModifierBaseEmoji,
            State::EmojiModifierBaseEmoji
        );

        add!(
            State::EmojiModifierBaseEmoji,
            Category::Vs16,
            State::OptionalZwj
        );
        add!(State::EmojiModifierBaseEmoji, Category::Zwj, State::Zwj);
        add!(
            State::EmojiModifierBaseEmoji,
            Category::EmojiModifier,
            State::OptionalZwj
        );

        // other
        add!(State::Start, Category::EmojiModifier, State::Terminal);
    }

    // Emoji flag sequence -- A sequence of two Regional Indicator characters.
    //
    // <https://unicode.org/reports/tr51/#def_emoji_flag_sequence>
    {
        add!(State::Start, Category::Ri, State::Ri);

        add!(State::Ri, Category::Ri, State::Terminal);
    }

    // Emoji tag sequence (ETS).
    //
    // <https://unicode.org/reports/tr51/#def_emoji_tag_sequence>
    {
        add!(State::Start, Category::TagBase, State::TagBase);

        add!(State::TagBase, Category::Vs15, State::Terminal);
        add!(State::TagBase, Category::Vs16, State::OptionalZwj);
        add!(State::TagBase, Category::TagSpec, State::TagSpec);
        add!(State::TagBase, Category::TagTerm, State::TagEmpty); // without any `TagSpec`
        add!(State::TagBase, Category::Zwj, State::Zwj);

        // (seq)+
        add!(State::TagSpec, Category::TagSpec, State::TagSpec);
        add!(State::TagSpec, Category::TagTerm, State::Terminal);
    }

    // Emoji keycap sequence.
    //
    // <https://unicode.org/reports/tr51/#def_emoji_keycap_sequence>
    {
        add!(State::Start, Category::KeycapBase, State::KeycapBase);

        add!(State::KeycapBase, Category::KeycapTerm, State::Terminal);
        add!(State::KeycapBase, Category::Vs15, State::KeycapVs);
        add!(State::KeycapBase, Category::Vs16, State::KeycapVs);

        add!(State::KeycapVs, Category::KeycapTerm, State::Terminal);
    }

    // Emoji ZWJ sequence.
    //
    // <https://unicode.org/reports/tr51/#def_emoji_zwj_sequence>
    {
        add!(State::OptionalZwj, Category::Zwj, State::Zwj);

        // (zwj emoji_zwj_element)+
        add!(State::Zwj, Category::Emoji, State::Emoji);
        add!(State::Zwj, Category::EmojiEmojiPresentation, State::Emoji);
        add!(
            State::Zwj,
            Category::EmojiModifierBaseEmoji,
            State::EmojiModifierBaseEmoji
        );
    }

    t
};

#[derive(Clone, Copy, Debug)]
pub struct EmojiDFA {
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
    pub const fn new() -> Self {
        Self::DEFAULT
    }

    #[inline]
    pub(crate) const fn step(&mut self, category: EmojiSegmentationCategory) {
        self.state = DFA_TRANS[self.state.as_usize()][category.as_usize()];
    }

    // pub(crate) const fn step_record(&mut self, category: EmojiSegmentationCategory) {
    pub const fn step_record(&mut self, category: EmojiSegmentationCategory) {
        self.step(category);

        if self.is_rejected() || self.is_started() {
            return;
        }

        self.recorded[0] |= 1 << self.state.as_u8();
        self.recorded[1] |= 1 << category.as_u8();
    }

    #[inline]
    pub(crate) const fn is_rejected(self) -> bool {
        self.state.eq(EmojiState::Reject)
    }

    #[inline]
    pub(crate) const fn is_started(self) -> bool {
        self.state.eq(EmojiState::Start)
    }

    #[allow(unused)]
    #[inline]
    pub(crate) const fn is_accepting(self) -> bool {
        const START: u8 = EmojiState::Terminal.as_u8();
        const END: u8 = EmojiState::Ri.as_u8();

        let cur = self.state.as_u8();

        START <= cur && cur <= END
    }

    #[inline]
    pub(crate) const fn contains_state(self, state: EmojiState) -> bool {
        self.recorded[0] & (1 << state.as_u8()) != 0
    }

    #[inline]
    pub(crate) const fn contains_category(self, category: EmojiSegmentationCategory) -> bool {
        self.recorded[1] & (1 << category.as_u8()) != 0
    }

    #[inline]
    pub(crate) const fn sequence(self) -> EmojiSequence {
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

    pub const fn presentation_style(&self) -> EmojiPresentationStyle {
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
