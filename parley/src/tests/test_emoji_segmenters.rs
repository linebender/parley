// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tests extracted from the [emoji segmenter].
//!
//! [emoji segmenter]: <https://github.com/google/emoji-segmenter>

use alloc::vec::Vec;
use core::char;

use crate::{
    analysis::AnalysisDataSources,
    emoji::{EmojiDFA, EmojiPresentationStyle, EmojiSegmentationCategory},
};

struct TestEntity<'a> {
    sequence: &'a [u32],
    categories: &'a [EmojiSegmentationCategory],
    style: EmojiPresentationStyle,
}

fn assert_emoji_segmenters_produce_same_result(entity: TestEntity<'_>) {
    let analysis = AnalysisDataSources::new();

    let mut emoji_dfa = EmojiDFA::new();

    let result = entity
        .sequence
        .iter()
        .copied()
        .map(|cp| {
            let ch = char::from_u32(cp).unwrap();
            let emoji_properties = analysis.emoji_properties(ch);

            let category = EmojiSegmentationCategory::from_codepoint(cp, emoji_properties);

            emoji_dfa.step_record(category);

            category
        })
        .collect::<Vec<_>>();

    assert_eq!(result, entity.categories);
    assert_eq!(emoji_dfa.presentation_style(), entity.style);
}

// Emoji presentation default; Encoded: 😀
#[test]
fn emoji_presentation_default() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F600, // GRINNING FACE
        ],
        categories: &[EmojiSegmentationCategory::EmojiPresentation],
        style: EmojiPresentationStyle::Emoji,
    });
}

// Text presentation default (copyright); Encoded: ©
#[test]
fn text_presentation_default() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x00A9, // COPYRIGHT SIGN
        ],
        categories: &[EmojiSegmentationCategory::Emoji],
        style: EmojiPresentationStyle::Default,
    });
}

// Lone keycap base; Encoded: 1
#[test]
fn long_keycap_base() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[0x0031], // DIGIT ONE
        categories: &[EmojiSegmentationCategory::KeycapBase],
        style: EmojiPresentationStyle::Default,
    });
}

// Keycap base + VS-15 (no term); Encoded: 1︎
#[test]
fn keycap_base_vs15() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x0031, // DIGIT ONE
            0xFE0E, // VARIATION SELECTOR-15
        ],
        categories: &[
            EmojiSegmentationCategory::KeycapBase,
            EmojiSegmentationCategory::Vs15,
        ],
        style: EmojiPresentationStyle::Text,
    });
}

// Keycap base + VS-16 (no term); Encoded: 1️
#[test]
fn keycap_base_vs16() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x0031, // DIGIT ONE
            0xFE0F, // VARIATION SELECTOR-16
        ],
        categories: &[
            EmojiSegmentationCategory::KeycapBase,
            EmojiSegmentationCategory::Vs16,
        ],
        style: EmojiPresentationStyle::Emoji,
    });
}

// Unqualified keycap; Encoded: #⃣
#[test]
fn unqualified_keycap() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x0023, // NUMBER SIGN
            0x20E3, // COMBINING ENCLOSING KEYCAP
        ],
        categories: &[
            EmojiSegmentationCategory::KeycapBase,
            EmojiSegmentationCategory::KeycapEnd,
        ],
        style: EmojiPresentationStyle::Default,
    });
}

// Keycap + VS-15 + term; Encoded: 1︎⃣
#[test]
fn keycap_vs15_term() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x0031, // DIGIT ONE
            0xFE0E, // VARIATION SELECTOR-15
            0x20E3, // COMBINING ENCLOSING KEYCAP
        ],
        categories: &[
            EmojiSegmentationCategory::KeycapBase,
            EmojiSegmentationCategory::Vs15,
            EmojiSegmentationCategory::KeycapEnd,
        ],
        style: EmojiPresentationStyle::Text,
    });
}

// Qualified keycap; Encoded: *️⃣
#[test]
fn qualified_keycap() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x002A, // ASTERISK
            0xFE0F, // VARIATION SELECTOR-16
            0x20E3, // COMBINING ENCLOSING KEYCAP
        ],
        categories: &[
            EmojiSegmentationCategory::KeycapBase,
            EmojiSegmentationCategory::Vs16,
            EmojiSegmentationCategory::KeycapEnd,
        ],
        style: EmojiPresentationStyle::Emoji,
    });
}

// Lone emoji modifier (Fitzpatrick); Encoded: 🏻
#[test]
fn lone_emoji_modifier() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F3FB, // EMOJI MODIFIER FITZPATRICK TYPE-1-2
        ],
        categories: &[EmojiSegmentationCategory::EmojiModifier],
        style: EmojiPresentationStyle::Emoji,
    });
}

// Bare modifier base, text default; Encoded: ☝
#[test]
fn bare_modifier_base_text_default() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x261D, // WHITE UP POINTING INDEX
        ],
        categories: &[EmojiSegmentationCategory::EmojiModifierBase],
        style: EmojiPresentationStyle::Text,
    });
}

// Modifier base (text default) + VS-16; Encoded: ☝️
#[test]
fn modifier_base_text_default_vs16() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x261D, // WHITE UP POINTING INDEX
            0xFE0F, // VARIATION SELECTOR-16
        ],
        categories: &[
            EmojiSegmentationCategory::EmojiModifierBase,
            EmojiSegmentationCategory::Vs16,
        ],
        style: EmojiPresentationStyle::Emoji,
    });
}

// Modifier base (text default) + skin tone; Encoded: ☝🏻
#[test]
fn modifier_base_text_default_skin_tone() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x261D,  // WHITE UP POINTING INDEX
            0x1F3FB, // EMOJI MODIFIER FITZPATRICK TYPE-1-2
        ],
        categories: &[
            EmojiSegmentationCategory::EmojiModifierBase,
            EmojiSegmentationCategory::EmojiModifier,
        ],
        style: EmojiPresentationStyle::Emoji,
    });
}

// Modifier base (emoji default) + skin tone; Encoded: 👦🏻
#[test]
fn modifier_base_emoji_default_skin_tone() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F466, // BOY
            0x1F3FB, // EMOJI MODIFIER FITZPATRICK TYPE-1-2
        ],
        categories: &[
            EmojiSegmentationCategory::EmojiModifierBase,
            EmojiSegmentationCategory::EmojiModifier,
        ],
        style: EmojiPresentationStyle::Emoji,
    });
}

// Lone regional indicator; Encoded: 🇺
#[test]
fn lone_regional_indicator() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F1FA, // REGIONAL INDICATOR SYMBOL LETTER U
        ],
        categories: &[EmojiSegmentationCategory::Ri],
        style: EmojiPresentationStyle::Default,
    });
}

// Flag sequence (US); Encoded: 🇺🇸
#[test]
fn flag_sequence_us() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F1FA, // REGIONAL INDICATOR SYMBOL LETTER U
            0x1F1F8, // REGIONAL INDICATOR SYMBOL LETTER S
        ],
        categories: &[EmojiSegmentationCategory::Ri, EmojiSegmentationCategory::Ri],
        style: EmojiPresentationStyle::Emoji,
    });
}

// Double lone regional indicator + Flag sequence (US); Encoded: 🇺🇺🇸
//
// FIXME: segmented clusters are incorrect
//  ✖️, [[0x1F1FA, 0x1F1FA], [0x1F1F8]]
//  ✔️, [[0x1F1FA], [0x1F1FA, 0x1F1F8]]
#[test]
#[ignore]
fn double_lone_regional_indicator_flag_sequence_us() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F1FA, // REGIONAL INDICATOR SYMBOL LETTER U
            0x1F1FA, // REGIONAL INDICATOR SYMBOL LETTER U
            0x1F1F8, // REGIONAL INDICATOR SYMBOL LETTER S
        ],
        categories: &[
            EmojiSegmentationCategory::Ri,
            EmojiSegmentationCategory::Ri,
            EmojiSegmentationCategory::Ri,
        ],
        style: EmojiPresentationStyle::Emoji,
    });
}

// Text-default emoji + VS-15; Encoded: ☺︎
#[test]
fn text_default_emoji_vs15() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x263A, // WHITE SMILING FACE
            0xFE0E, // VARIATION SELECTOR-15
        ],
        categories: &[
            EmojiSegmentationCategory::Emoji,
            EmojiSegmentationCategory::Vs15,
        ],
        style: EmojiPresentationStyle::Text,
    });
}

// Text-default emoji + VS-16; Encoded: ☺️
#[test]
fn text_default_emoji_vs16() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x263A, // WHITE SMILING FACE
            0xFE0F, // VARIATION SELECTOR-16
        ],
        categories: &[
            EmojiSegmentationCategory::Emoji,
            EmojiSegmentationCategory::Vs16,
        ],
        style: EmojiPresentationStyle::Emoji,
    });
}

// Emoji-default emoji + VS-15; Encoded: 😀︎
#[test]
fn emoji_default_emoji_vs15() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F600, // GRINNING FACE
            0xFE0E,  // VARIATION SELECTOR-15
        ],
        categories: &[
            EmojiSegmentationCategory::EmojiPresentation,
            EmojiSegmentationCategory::Vs15,
        ],
        style: EmojiPresentationStyle::Text,
    });
}

// Emoji-default emoji + VS-16; Encoded: 😀️
#[test]
fn emoji_default_emoji_vs16() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F600, // GRINNING FACE
            0xFE0F,  // VARIATION SELECTOR-16
        ],
        categories: &[
            EmojiSegmentationCategory::EmojiPresentation,
            EmojiSegmentationCategory::Vs16,
        ],
        style: EmojiPresentationStyle::Emoji,
    });
}

// ZWJ family; Encoded: 👨‍👩‍👧
#[test]
fn zwj_family() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F468, // MAN
            0x200D,  // ZERO WIDTH JOINER
            0x1F469, // WOMAN
            0x200D,  // ZERO WIDTH JOINER
            0x1F467, // GIRL
        ],
        categories: &[
            EmojiSegmentationCategory::EmojiModifierBase,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiModifierBase,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiModifierBase,
        ],
        style: EmojiPresentationStyle::Emoji,
    });
}

// Long ZWJ family (4 members); Encoded: 👨‍👩‍👧‍👦
#[test]
fn long_zwj_family() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F468, // MAN
            0x200D,  // ZERO WIDTH JOINER
            0x1F469, // WOMAN
            0x200D,  // ZERO WIDTH JOINER
            0x1F467, // GIRL
            0x200D,  // ZERO WIDTH JOINER
            0x1F466, // BOY
        ],
        categories: &[
            EmojiSegmentationCategory::EmojiModifierBase,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiModifierBase,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiModifierBase,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiModifierBase,
        ],
        style: EmojiPresentationStyle::Emoji,
    });
}

// ZWJ couple; Encoded: 👨‍❤‍👨
#[test]
fn zwj_couple() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F468, // MAN
            0x200D,  // ZERO WIDTH JOINER
            0x2764,  // HEAVY BLACK HEART
            0x200D,  // ZERO WIDTH JOINER
            0x1F468, // MAN
        ],
        categories: &[
            EmojiSegmentationCategory::EmojiModifierBase,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::Emoji,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiModifierBase,
        ],
        style: EmojiPresentationStyle::Emoji,
    });
}

// ZWJ with VS-16 element; Encoded: 👨️‍👩
#[test]
fn zwj_with_vs16_element() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F468, // MAN
            0xFE0F,  // VARIATION SELECTOR-16
            0x200D,  // ZERO WIDTH JOINER
            0x1F469, // WOMAN
        ],
        categories: &[
            EmojiSegmentationCategory::EmojiModifierBase,
            EmojiSegmentationCategory::Vs16,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiModifierBase,
        ],
        style: EmojiPresentationStyle::Emoji,
    });
}

// ZWJ with VS-16 on both elements; Encoded: 👨️‍👩️
#[test]
fn zwj_with_vs16_on_both_elements() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F468, // MAN
            0xFE0F,  // VARIATION SELECTOR-16
            0x200D,  // ZERO WIDTH JOINER
            0x1F469, // WOMAN
            0xFE0F,  // VARIATION SELECTOR-16
        ],
        categories: &[
            EmojiSegmentationCategory::EmojiModifierBase,
            EmojiSegmentationCategory::Vs16,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiModifierBase,
            EmojiSegmentationCategory::Vs16,
        ],
        style: EmojiPresentationStyle::Emoji,
    });
}

// ZWJ after modifier sequence; Encoded: 👦🏻‍💻
#[test]
fn zwj_after_modifier_sequence() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F466, // BOY
            0x1F3FB, // EMOJI MODIFIER FITZPATRICK TYPE-1-2
            0x200D,  // ZERO WIDTH JOINER
            0x1F4BB, // PERSONAL COMPUTER
        ],
        categories: &[
            EmojiSegmentationCategory::EmojiModifierBase,
            EmojiSegmentationCategory::EmojiModifier,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiPresentation,
        ],
        style: EmojiPresentationStyle::Emoji,
    });
}

// ZWJ technologist with skin tone; Encoded: 👨🏻‍💻
#[test]
fn zwj_technologist_with_skin_tone() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F468, // MAN
            0x1F3FB, // EMOJI MODIFIER FITZPATRICK TYPE-1-2
            0x200D,  // ZERO WIDTH JOINER
            0x1F4BB, // PERSONAL COMPUTER
        ],
        categories: &[
            EmojiSegmentationCategory::EmojiModifierBase,
            EmojiSegmentationCategory::EmojiModifier,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiPresentation,
        ],
        style: EmojiPresentationStyle::Emoji,
    });
}

// VS-16 enables ZWJ continuation; Encoded: ☺️‍👩
#[test]
fn vs16_enables_zwj_continuation() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x263A,  // WHITE SMILING FACE
            0xFE0F,  // VARIATION SELECTOR-16
            0x200D,  // ZERO WIDTH JOINER
            0x1F469, // WOMAN
        ],
        categories: &[
            EmojiSegmentationCategory::Emoji,
            EmojiSegmentationCategory::Vs16,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiModifierBase,
        ],
        style: EmojiPresentationStyle::Emoji,
    });
}

// Tag sequence (England); Encoded: 🏴󠁧󠁢󠁥󠁮󠁧󠁿
#[test]
fn tag_sequence_england() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F3F4, // WAVING BLACK FLAG
            0xE0067, // TAG LATIN SMALL LETTER G
            0xE0062, // TAG LATIN SMALL LETTER B
            0xE0065, // TAG LATIN SMALL LETTER E
            0xE006E, // TAG LATIN SMALL LETTER N
            0xE0067, // TAG LATIN SMALL LETTER G
            0xE007F, // CANCEL TAG
        ],
        categories: &[
            EmojiSegmentationCategory::TagBase,
            EmojiSegmentationCategory::TagSpec,
            EmojiSegmentationCategory::TagSpec,
            EmojiSegmentationCategory::TagSpec,
            EmojiSegmentationCategory::TagSpec,
            EmojiSegmentationCategory::TagSpec,
            EmojiSegmentationCategory::TagEnd,
        ],
        style: EmojiPresentationStyle::Emoji,
    });
}

// TAG_BASE as ZWJ element; Encoded: 🏴‍😀"
#[test]
fn tag_base_as_zwj_element() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F3F4, // WAVING BLACK FLAG
            0x200D,  // ZERO WIDTH JOINER
            0x1F600, // GRINNING FACE
        ],
        categories: &[
            EmojiSegmentationCategory::TagBase,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiPresentation,
        ],
        style: EmojiPresentationStyle::Emoji,
    });
}

// TAG_BASE + VS-16 + ZWJ; Encoded: 🏴️‍😀",
#[test]
fn tag_base_vs16_as_zwj() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F3F4, // WAVING BLACK FLAG
            0xFE0F,  // VARIATION SELECTOR-16
            0x200D,  // ZERO WIDTH JOINER
            0x1F600, // GRINNING FACE
        ],
        categories: &[
            EmojiSegmentationCategory::TagBase,
            EmojiSegmentationCategory::Vs16,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiPresentation,
        ],
        style: EmojiPresentationStyle::Emoji,
    });
}

// TAG_BASE + VS-15; Encoded: 🏴︎
#[test]
fn tag_base_vs15() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F3F4, // WAVING BLACK FLAG
            0xFE0E,  // VARIATION SELECTOR-15
        ],
        categories: &[
            EmojiSegmentationCategory::TagBase,
            EmojiSegmentationCategory::Vs15,
        ],
        style: EmojiPresentationStyle::Text,
    });
}

// TAG_BASE + VS-16; Encoded: 🏴️
#[test]
fn tag_base_vs16() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F3F4, // WAVING BLACK FLAG
            0xFE0F,  // VARIATION SELECTOR-16
        ],
        categories: &[
            EmojiSegmentationCategory::TagBase,
            EmojiSegmentationCategory::Vs16,
        ],
        style: EmojiPresentationStyle::Emoji,
    });
}
