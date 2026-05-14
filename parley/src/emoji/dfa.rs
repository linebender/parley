// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::types::{EmojiPresentationStyle, EmojiSegmentationCategory, EmojiSequence, EmojiState};

/// The transition table for Emoji DFA.
///
/// <https://unicode.org/reports/tr51/#Definitions>
static DFA_TRANS: [[u8; 13]; 14] = {
    use EmojiSegmentationCategory as Category;
    use EmojiState as State;

    let mut t = [[0; 13]; 14];

    /// Add a state transition to the DFA transition table.
    macro_rules! add {
        ($state:expr, $category:expr, $next_state:expr) => {
            t[$state.as_usize()][$category.as_usize()] = $next_state.as_u8()
        };
    }

    // Text and Emoji presentation sequences
    {
        add!(State::Start, Category::Emoji, State::Emoji);

        add!(State::Start, Category::EmojiPresentation, State::Emoji);

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
        add!(
            State::Start,
            Category::EmojiModifierBase,
            State::EmojiModifierBase
        );

        add!(State::EmojiModifierBase, Category::Vs16, State::OptionalZwj);
        add!(State::EmojiModifierBase, Category::Zwj, State::Zwj);
        add!(
            State::EmojiModifierBase,
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
        add!(State::TagBase, Category::TagEnd, State::TagEmpty); // without any `TagSpec`
        add!(State::TagBase, Category::Zwj, State::Zwj);

        // (seq)+
        add!(State::TagSpec, Category::TagSpec, State::TagSpec);
        add!(State::TagSpec, Category::TagEnd, State::Terminal);
    }

    // Emoji keycap sequence.
    //
    // <https://unicode.org/reports/tr51/#def_emoji_keycap_sequence>
    {
        add!(State::Start, Category::KeycapBase, State::KeycapBase);

        add!(State::KeycapBase, Category::KeycapEnd, State::Terminal);
        add!(State::KeycapBase, Category::Vs15, State::KeycapVs);
        add!(State::KeycapBase, Category::Vs16, State::KeycapVs);

        add!(State::KeycapVs, Category::KeycapEnd, State::Terminal);
    }

    // Emoji ZWJ sequence.
    //
    // <https://unicode.org/reports/tr51/#def_emoji_zwj_sequence>
    {
        add!(State::OptionalZwj, Category::Zwj, State::Zwj);

        // (zwj emoji_zwj_element)+
        add!(State::Zwj, Category::Emoji, State::Emoji);
        add!(State::Zwj, Category::EmojiPresentation, State::Emoji);
        add!(
            State::Zwj,
            Category::EmojiModifierBase,
            State::EmojiModifierBase
        );
    }

    t
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct EmojiDFA {
    state: EmojiState,
    // (state, category)
    recorded: (u16, u16),
}

impl EmojiDFA {
    const DEFAULT: Self = Self {
        state: EmojiState::Start,
        recorded: (0, 0),
    };

    #[inline]
    pub(crate) const fn new() -> Self {
        Self::DEFAULT
    }

    #[inline]
    pub(crate) const fn step(&mut self, category: EmojiSegmentationCategory) {
        self.state = EmojiState::from_u8(DFA_TRANS[self.state.as_usize()][category.as_usize()]);
    }

    #[inline]
    pub(crate) const fn step_record(&mut self, category: EmojiSegmentationCategory) {
        self.step(category);

        if self.is_rejected() || self.is_started() {
            return;
        }

        self.recorded.0 |= 1 << self.state.as_u8();
        self.recorded.1 |= 1 << category.as_u8();
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
        self.recorded.0 & (1 << state.as_u8()) != 0
    }

    #[inline]
    pub(crate) const fn contains_category(self, category: EmojiSegmentationCategory) -> bool {
        self.recorded.1 & (1 << category.as_u8()) != 0
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

        if self.contains_category(EmojiSegmentationCategory::EmojiModifierBase)
            && self.contains_category(EmojiSegmentationCategory::EmojiModifier)
        {
            return EmojiSequence::Modifier;
        }

        if self.contains_category(EmojiSegmentationCategory::KeycapBase)
            && self.contains_category(EmojiSegmentationCategory::Vs16)
            && self.contains_category(EmojiSegmentationCategory::KeycapEnd)
        {
            return EmojiSequence::Keycap;
        }

        if self.contains_category(EmojiSegmentationCategory::KeycapEnd)
            && self.contains_category(EmojiSegmentationCategory::Vs16)
        {
            return EmojiSequence::Keycap;
        }

        EmojiSequence::Basic
    }

    #[inline]
    pub(crate) const fn presentation_style(self) -> EmojiPresentationStyle {
        if self.contains_category(EmojiSegmentationCategory::Vs15) {
            return EmojiPresentationStyle::Text;
        }
        if self.contains_category(EmojiSegmentationCategory::Vs16) {
            return EmojiPresentationStyle::Emoji;
        }

        if self.contains_category(EmojiSegmentationCategory::EmojiPresentation) {
            return EmojiPresentationStyle::Emoji;
        }

        if !self.sequence().eq(EmojiSequence::Basic) {
            return EmojiPresentationStyle::Emoji;
        }

        // single emoji modifier; e.g. 🏻
        if self.contains_category(EmojiSegmentationCategory::EmojiModifier) {
            return EmojiPresentationStyle::Emoji;
        }

        // single emoji modifier base; e.g ☝
        if self.contains_category(EmojiSegmentationCategory::EmojiModifierBase) {
            return EmojiPresentationStyle::Text;
        }

        EmojiPresentationStyle::Default
    }
}
