// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::char;
use std::vec::Vec;

use crate::{
    analysis::AnalysisDataSources,
    emoji::{
        EmojiFlags, EmojiSegmentationCategory, ScannedEmojiPresetation, scan_emoji_presetation,
    },
};

struct TestEntity<'a> {
    sequence: &'a [u32],
    categories: &'a [EmojiSegmentationCategory],
    scanned: ScannedEmojiPresetation,
}

fn assert_emoji_segmenters_produce_same_result(entity: TestEntity<'_>) {
    let analysis = AnalysisDataSources::new();
    let emoji_modifier = analysis.emoji_modifier();
    let emoji_modifier_base = analysis.emoji_modifier_base();
    let emoji_component = analysis.emoji_component();
    let emoji_presentation = analysis.emoji_presentation();

    let result = entity
        .sequence
        .iter()
        .copied()
        .map(|cp| {
            let props = analysis.properties(char::from_u32(cp).unwrap());

            let is_emoji = props.is_emoji_or_pictograph();
            let is_emoji_modifier = emoji_modifier.contains32(cp);
            let is_emoji_modifier_base = emoji_modifier_base.contains32(cp);
            let is_emoji_presentation = emoji_presentation.contains32(cp);
            let is_emoji_component = emoji_component.contains32(cp);
            let is_regional_indicator = props.is_region_indicator();

            let emoji_flags = EmojiFlags::new().with_emoji(is_emoji).with_extra(
                is_emoji_modifier,
                is_emoji_modifier_base,
                is_emoji_presentation,
                is_emoji_component,
                is_regional_indicator,
            );

            EmojiSegmentationCategory::from_codepoint(cp, emoji_flags)
        })
        .collect::<Vec<_>>();

    assert_eq!(result, entity.categories);

    assert_eq!(scan_emoji_presetation(&result), entity.scanned);
}

// Emoji presentation default; Encoded: 😀
#[test]
fn emoji_presentation_default() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F600, // GRINNING FACE
        ],
        categories: &[EmojiSegmentationCategory::EmojiEmojiPresentation],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        },
    });
}

// Text presentation default (copyright); Encoded: ©
#[test]
fn text_presentation_default() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x00A9, // COPYRIGHT SIGN
        ],
        categories: &[EmojiSegmentationCategory::EmojiTextPresentation],
        scanned: ScannedEmojiPresetation {
            is_emoji: false,
            has_vs: false,
        },
    });
}

// Lone keycap base; Encoded: 1
#[test]
fn long_keycap_base() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[0x0031], // DIGIT ONE
        categories: &[EmojiSegmentationCategory::KeycapBase],
        scanned: ScannedEmojiPresetation {
            is_emoji: false,
            has_vs: false,
        },
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
        scanned: ScannedEmojiPresetation {
            is_emoji: false,
            has_vs: true,
        },
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
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: true,
        },
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
            EmojiSegmentationCategory::CombiningEnclosingKeycap,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        },
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
            EmojiSegmentationCategory::CombiningEnclosingKeycap,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: false,
            has_vs: true,
        },
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
            EmojiSegmentationCategory::CombiningEnclosingKeycap,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: true,
        },
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
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        },
    });
}

// Bare modifier base, text default; Encoded: ☝
#[test]
fn bare_modifier_base_text_default() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x261D, // WHITE UP POINTING INDEX
        ],
        categories: &[EmojiSegmentationCategory::EmojiModifierBaseText],
        scanned: ScannedEmojiPresetation {
            is_emoji: false,
            has_vs: false,
        },
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
            EmojiSegmentationCategory::EmojiModifierBaseText,
            EmojiSegmentationCategory::Vs16,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: true,
        },
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
            EmojiSegmentationCategory::EmojiModifierBaseText,
            EmojiSegmentationCategory::EmojiModifier,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        },
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
            EmojiSegmentationCategory::EmojiModifierBaseEmoji,
            EmojiSegmentationCategory::EmojiModifier,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        },
    });
}

// Lone regional indicator; Encoded: 🇺
#[test]
fn lone_regional_indicator() {
    assert_emoji_segmenters_produce_same_result(TestEntity {
        sequence: &[
            0x1F1FA, // REGIONAL INDICATOR SYMBOL LETTER U
        ],
        categories: &[EmojiSegmentationCategory::RegionalIndicator],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        },
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
        categories: &[
            EmojiSegmentationCategory::RegionalIndicator,
            EmojiSegmentationCategory::RegionalIndicator,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        },
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
            EmojiSegmentationCategory::RegionalIndicator,
            EmojiSegmentationCategory::RegionalIndicator,
            EmojiSegmentationCategory::RegionalIndicator,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        },
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
            EmojiSegmentationCategory::EmojiTextPresentation,
            EmojiSegmentationCategory::Vs15,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: false,
            has_vs: true,
        },
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
            EmojiSegmentationCategory::EmojiTextPresentation,
            EmojiSegmentationCategory::Vs16,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: true,
        },
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
            EmojiSegmentationCategory::EmojiEmojiPresentation,
            EmojiSegmentationCategory::Vs15,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: false,
            has_vs: true,
        },
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
            EmojiSegmentationCategory::EmojiEmojiPresentation,
            EmojiSegmentationCategory::Vs16,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: true,
        },
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
            EmojiSegmentationCategory::EmojiModifierBaseEmoji,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiModifierBaseEmoji,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiModifierBaseEmoji,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        },
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
            EmojiSegmentationCategory::EmojiModifierBaseEmoji,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiModifierBaseEmoji,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiModifierBaseEmoji,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiModifierBaseEmoji,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        },
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
            EmojiSegmentationCategory::EmojiModifierBaseEmoji,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiTextPresentation,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiModifierBaseEmoji,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        },
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
            EmojiSegmentationCategory::EmojiModifierBaseEmoji,
            EmojiSegmentationCategory::Vs16,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiModifierBaseEmoji,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        },
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
            EmojiSegmentationCategory::EmojiModifierBaseEmoji,
            EmojiSegmentationCategory::Vs16,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiModifierBaseEmoji,
            EmojiSegmentationCategory::Vs16,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        },
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
            EmojiSegmentationCategory::EmojiModifierBaseEmoji,
            EmojiSegmentationCategory::EmojiModifier,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiEmojiPresentation,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        },
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
            EmojiSegmentationCategory::EmojiModifierBaseEmoji,
            EmojiSegmentationCategory::EmojiModifier,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiEmojiPresentation,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        },
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
            EmojiSegmentationCategory::EmojiTextPresentation,
            EmojiSegmentationCategory::Vs16,
            EmojiSegmentationCategory::Zwj,
            EmojiSegmentationCategory::EmojiModifierBaseEmoji,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        },
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
            EmojiSegmentationCategory::TagSequence,
            EmojiSegmentationCategory::TagSequence,
            EmojiSegmentationCategory::TagSequence,
            EmojiSegmentationCategory::TagSequence,
            EmojiSegmentationCategory::TagSequence,
            EmojiSegmentationCategory::TagTerm,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        },
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
            EmojiSegmentationCategory::EmojiEmojiPresentation,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        },
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
            EmojiSegmentationCategory::EmojiEmojiPresentation,
        ],
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: false,
        },
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
        scanned: ScannedEmojiPresetation {
            is_emoji: false,
            has_vs: true,
        },
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
        scanned: ScannedEmojiPresetation {
            is_emoji: true,
            has_vs: true,
        },
    });
}
