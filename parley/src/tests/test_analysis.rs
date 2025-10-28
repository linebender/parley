use crate::analysis::{BidiLevel, Boundary};
use crate::{FontContext, LayoutContext, RangedBuilder, StyleProperty};
use fontique::FontWeight;
use icu_properties::props::{GraphemeClusterBreak, Script};
use icu_segmenter::options::LineBreakWordOption;

#[derive(Default)]
struct TestContext {
    pub layout_context: LayoutContext,
    pub font_context: FontContext,
}

impl TestContext {
    fn expect_boundary_list(self, expected: Vec<Boundary>) -> Self {
        let actual: Vec<_> = self
            .layout_context
            .info
            .iter()
            .map(|(info, _)| info.boundary)
            .collect();
        assert_eq!(actual, expected, "Boundary list mismatch");
        self
    }

    fn expect_bidi_embed_level_list(self, expected: Vec<BidiLevel>) -> Self {
        let actual: Vec<_> = self
            .layout_context
            .info
            .iter()
            .map(|(info, _)| info.bidi_embed_level)
            .collect();
        assert_eq!(actual, expected, "Bidi embed level list mismatch");
        self
    }

    fn expect_script_list(self, expected: Vec<Script>) -> Self {
        let actual: Vec<_> = self
            .layout_context
            .info
            .iter()
            .map(|(info, _)| info.script)
            .collect();
        assert_eq!(actual, expected, "Script list mismatch");
        self
    }

    fn expect_grapheme_cluster_break_list(self, expected: Vec<GraphemeClusterBreak>) -> Self {
        let actual: Vec<_> = self
            .layout_context
            .info
            .iter()
            .map(|(info, _)| info.grapheme_cluster_break)
            .collect();
        assert_eq!(actual, expected, "Grapheme cluster break list mismatch");
        self
    }

    fn expect_is_control_list(self, expected: Vec<bool>) -> Self {
        let actual: Vec<_> = self
            .layout_context
            .info
            .iter()
            .map(|(info, _)| info.is_control())
            .collect();
        assert_eq!(actual, expected, "Is control list mismatch");
        self
    }

    fn expect_contributes_to_shaping_list(self, expected: Vec<bool>) -> Self {
        let actual: Vec<_> = self
            .layout_context
            .info
            .iter()
            .map(|(info, _)| info.contributes_to_shaping())
            .collect();
        assert_eq!(actual, expected, "Contributes to shaping list mismatch");
        self
    }

    fn expect_force_normalize_list(self, expected: Vec<bool>) -> Self {
        let actual: Vec<_> = self
            .layout_context
            .info
            .iter()
            .map(|(info, _)| info.force_normalize())
            .collect();
        assert_eq!(actual, expected, "Force normalize list mismatch");
        self
    }
}

fn verify_analysis(
    text: &str,
    configure_builder: impl for<'a> FnOnce(&mut RangedBuilder<'a, [u8; 4]>),
) -> TestContext {
    let mut test_context = TestContext::default();

    {
        let mut builder = test_context.layout_context.ranged_builder(
            &mut test_context.font_context,
            text,
            1.,
            true,
        );

        // Apply test-specific configuration
        configure_builder(&mut builder);

        _ = builder.build(&text);
    }

    test_context
}

#[test]
fn test_latin_mixed_keep_all_last() {
    verify_analysis("AB", |builder| {
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 0..1);
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::KeepAll), 1..2);
    })
        .expect_boundary_list(vec![Boundary::Word, Boundary::None])
        .expect_bidi_embed_level_list(vec![0, 0])
        .expect_script_list(vec![
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
        ])
        .expect_grapheme_cluster_break_list(vec![
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
        ])
        .expect_is_control_list(vec![false, false])
        .expect_contributes_to_shaping_list(vec![true, true])
        .expect_force_normalize_list(vec![false, false]);
}

#[test]
fn test_mandatory_break_in_text() {
    verify_analysis("ABC DEF\nG", |_| {})
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::None,
            Boundary::None,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
            Boundary::None,
            Boundary::Word,
            Boundary::Mandatory,
        ])
        .expect_bidi_embed_level_list(vec![0, 0, 0, 0, 0, 0, 0, 0, 0])
        .expect_script_list(vec![
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
        ])
        .expect_grapheme_cluster_break_list(vec![
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(5),
            GraphemeClusterBreak::from_icu4c_value(0),
        ])
        .expect_is_control_list(vec![
            false, false, false, false, false, false, false, true, false,
        ])
        .expect_contributes_to_shaping_list(vec![
            true, true, true, true, true, true, true, false, true,
        ])
        .expect_force_normalize_list(vec![
            false, false, false, false, false, false, false, false, false,
        ]);
}

#[test]
fn test_blank() {
    verify_analysis("", |_| {})
        .expect_boundary_list(vec![Boundary::Word])
        .expect_bidi_embed_level_list(vec![0])
        .expect_script_list(vec![Script::from_icu4c_value(0)])
        .expect_grapheme_cluster_break_list(vec![GraphemeClusterBreak::from_icu4c_value(0)])
        .expect_is_control_list(vec![false])
        .expect_contributes_to_shaping_list(vec![true])
        .expect_force_normalize_list(vec![false]);
}

#[test]
fn test_latin_mixed_keep_all_first() {
    verify_analysis("AB", |builder| {
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::KeepAll), 0..1);
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 1..2);
    })
        .expect_boundary_list(vec![Boundary::Word, Boundary::None]);
}

#[test]
fn test_mixed_break_four_segments() {
    verify_analysis("ABCD 123", |builder| {
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 0..1);
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::KeepAll), 1..2);
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            2..4,
        );
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::KeepAll), 4..8);
    })
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::None,
            Boundary::Line,
            Boundary::Line,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
            Boundary::None,
        ]);
}

#[test]
fn test_alternate_twice_within_word_normal_break_normal() {
    verify_analysis("ABC", |builder| {
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 0..1);
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            1..2,
        );
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 2..3);
    })
        .expect_boundary_list(vec![Boundary::Word, Boundary::Line, Boundary::None]);
}

#[test]
fn test_alternate_twice_within_word_break_normal_break() {
    verify_analysis("ABC", |builder| {
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            0..1,
        );
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 1..2);
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            2..3,
        );
    })
        .expect_boundary_list(vec![Boundary::Word, Boundary::None, Boundary::Line]);
}

#[test]
fn test_latin_trailing_space_mixed() {
    verify_analysis("AB ", |builder| {
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            0..1,
        );
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 1..3);
    })
        .expect_boundary_list(vec![Boundary::Word, Boundary::None, Boundary::Word])
        .expect_bidi_embed_level_list(vec![0, 0, 0])
        .expect_script_list(vec![
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
        ]);
}

#[test]
fn test_latin_leading_space_mixed() {
    verify_analysis(" AB", |builder| {
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            0..1,
        );
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 1..3);
    })
        .expect_boundary_list(vec![Boundary::Word, Boundary::Line, Boundary::None])
        .expect_bidi_embed_level_list(vec![0, 0, 0])
        .expect_script_list(vec![
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
        ]);
}

#[test]
fn test_latin_mixed_break_all_last() {
    verify_analysis("AB", |builder| {
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 0..1);
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            1..2,
        );
    })
        .expect_boundary_list(vec![Boundary::Word, Boundary::Line]);
}

#[test]
fn test_latin_mixed_break_all_first() {
    verify_analysis("AB", |builder| {
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            0..1,
        );
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 1..2);
    })
        .expect_boundary_list(vec![Boundary::Word, Boundary::None]);
}

#[test]
fn test_all_whitespace() {
    verify_analysis("   ", |_| {})
        .expect_boundary_list(vec![Boundary::Word, Boundary::None, Boundary::None])
        .expect_bidi_embed_level_list(vec![0, 0, 0])
        .expect_script_list(vec![
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
        ]);
}

#[test]
fn test_multi_char_grapheme() {
    verify_analysis("A e\u{301} B", |_| {})
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
            Boundary::Word,
            Boundary::Line,
        ])
        .expect_script_list(vec![
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(1),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
        ])
        .expect_grapheme_cluster_break_list(vec![
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(3),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
        ])
        .expect_is_control_list(vec![false, false, false, false, false, false])
        .expect_contributes_to_shaping_list(vec![true, true, true, true, true, true])
        .expect_force_normalize_list(vec![false, false, false, true, false, false]);
}

#[test]
fn test_mixed_break_frequent_alternation() {
    verify_analysis("ABCD 123", |builder| {
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 0..1);
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::KeepAll), 1..2);
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            2..3,
        );
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 3..4);
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::KeepAll), 4..5);
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            5..6,
        );
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 6..7);
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::KeepAll), 7..8);
    })
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::None,
            Boundary::Line,
            Boundary::None,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
            Boundary::None,
        ])
        .expect_script_list(vec![
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
        ]);
}

#[test]
fn test_mixed_style() {
    verify_analysis("A  B  C D", |builder| {
        builder.push(StyleProperty::FontWeight(FontWeight::new(400.0)), 0..3);
        builder.push(StyleProperty::FontWeight(FontWeight::new(700.0)), 3..9);
    })
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::Word,
            Boundary::None,
            Boundary::Line,
            Boundary::Word,
            Boundary::None,
            Boundary::Line,
            Boundary::Word,
            Boundary::Line,
        ])
        .expect_script_list(vec![
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
        ]);
}

#[test]
fn test_mixed_ltr_rtl() {
    verify_analysis("Hello مرحبا", |_| {})
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::None,
        ])
        .expect_bidi_embed_level_list(vec![0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1])
        .expect_script_list(vec![
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
        ]);
}

#[test]
fn test_multi_byte_chars_alternating_break_all() {
    verify_analysis("€你€你AA", |builder| {
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            0..3,
        );
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 3..6);
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            6..9,
        );
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 9..12);
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            12..13,
        );
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::Normal),
            13..14,
        );
    })
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::Word,
            Boundary::Line,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
        ])
        .expect_script_list(vec![
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(17),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(17),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
        ]);
}

#[test]
fn test_multi_byte_chars_varying_utf8_lengths_whitespace_separated() {
    verify_analysis("ß € 𝓗 你 ą", |builder| {
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            0..3,
        );
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 3..7);
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            7..12,
        );
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::Normal),
            12..16,
        );
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            16..19,
        );
    })
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::Word,
            Boundary::Line,
            Boundary::Word,
            Boundary::Line,
            Boundary::Word,
            Boundary::Line,
            Boundary::Word,
            Boundary::Line,
        ])
        .expect_script_list(vec![
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(17),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
        ]);
}

#[test]
fn test_multi_byte_chars_varying_utf8_lengths() {
    verify_analysis("ß€𝓗你ą", |builder| {
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            0..2,
        );
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 2..5);
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            5..9,
        );
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 9..12);
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            12..14,
        );
    })
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::Word,
            Boundary::Word,
            Boundary::Line,
            Boundary::Line,
        ])
        .expect_script_list(vec![
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(17),
            Script::from_icu4c_value(25),
        ]);
}

#[test]
fn test_mixed_ltr_rtl_nested_embedding() {
    verify_analysis("In Hebrew: שנת 2024 היא...", |_| {})
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::None,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::Word,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
            Boundary::None,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
            Boundary::None,
            Boundary::Word,
            Boundary::Word,
            Boundary::Word,
        ])
        .expect_bidi_embed_level_list(vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 1, 1, 1, 1, 0, 0, 0,
        ])
        .expect_script_list(vec![
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(19),
            Script::from_icu4c_value(19),
            Script::from_icu4c_value(19),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(19),
            Script::from_icu4c_value(19),
            Script::from_icu4c_value(19),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
        ]);
}

#[test]
fn test_mixed_break_simple() {
    verify_analysis("ABCD 123", |builder| {
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 0..1);
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::KeepAll), 1..8);
    })
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
            Boundary::None,
        ])
        .expect_script_list(vec![
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
        ]);
}

#[test]
fn test_multi_char_grapheme_mixed_break_all() {
    verify_analysis("A e\u{301} B", |builder| {
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            0..1,
        );
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 1..2);
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            2..5,
        );
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 5..6);
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            6..7,
        );
    })
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
            Boundary::Word,
            Boundary::Line,
        ])
        .expect_bidi_embed_level_list(vec![0, 0, 0, 0, 0, 0])
        .expect_script_list(vec![
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(1),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
        ])
        .expect_grapheme_cluster_break_list(vec![
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(3),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
        ])
        .expect_is_control_list(vec![false, false, false, false, false, false])
        .expect_contributes_to_shaping_list(vec![true, true, true, true, true, true])
        .expect_force_normalize_list(vec![false, false, false, true, false, false]);
}

#[test]
fn test_multi_byte_chars_alternating_keep_all() {
    verify_analysis("€你€你AA", |builder| {
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::KeepAll), 0..3);
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 3..6);
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::KeepAll), 6..9);
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 9..12);
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::KeepAll),
            12..13,
        );
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::Normal),
            13..14,
        );
    })
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::Word,
            Boundary::Line,
            Boundary::Word,
            Boundary::Word,
            Boundary::None,
        ])
        .expect_script_list(vec![
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(17),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(17),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
        ]);
}

#[test]
fn test_mixed_ltr_rtl_multiple_segments() {
    verify_analysis("Hello مرحبا World عالم Test اختبار", |_| {})
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::None,
        ])
        .expect_bidi_embed_level_list(vec![
            0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0,
            1, 1, 1, 1, 1, 1,
        ])
        .expect_script_list(vec![
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
        ])
        .expect_grapheme_cluster_break_list(vec![
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
        ]);
}

#[test]
fn test_multi_char_grapheme_mixed_break_and_keep_all() {
    verify_analysis("A e\u{301} B", |builder| {
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::KeepAll), 0..1);
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            1..2,
        );
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::KeepAll), 2..5);
        builder.push(
            StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
            5..6,
        );
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::KeepAll), 6..7);
    })
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
            Boundary::Word,
            Boundary::Line,
        ])
        .expect_bidi_embed_level_list(vec![0, 0, 0, 0, 0, 0])
        .expect_script_list(vec![
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(1),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
        ])
        .expect_grapheme_cluster_break_list(vec![
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(3),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
        ])
        .expect_is_control_list(vec![false, false, false, false, false, false])
        .expect_contributes_to_shaping_list(vec![true, true, true, true, true, true])
        .expect_force_normalize_list(vec![false, false, false, true, false, false]);
}

#[test]
fn test_multi_char_grapheme_mixed_keep_all() {
    verify_analysis("A e\u{301} B", |builder| {
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::KeepAll), 0..1);
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 1..2);
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::KeepAll), 2..5);
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 5..6);
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::KeepAll), 6..7);
    })
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
            Boundary::Word,
            Boundary::Line,
        ])
        .expect_bidi_embed_level_list(vec![0, 0, 0, 0, 0, 0])
        .expect_script_list(vec![
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(1),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
        ])
        .expect_grapheme_cluster_break_list(vec![
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(3),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
        ])
        .expect_is_control_list(vec![false, false, false, false, false, false])
        .expect_contributes_to_shaping_list(vec![true, true, true, true, true, true])
        .expect_force_normalize_list(vec![false, false, false, true, false, false]);
}

#[test]
fn test_multi_paragraph_bidi() {
    verify_analysis("Hello مرحبا \nTest اختبار", |_| {})
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::Word,
            Boundary::Word,
            Boundary::Mandatory,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::Word,
            Boundary::Line,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::None,
        ])
        .expect_bidi_embed_level_list(vec![
            0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1,
        ])
        .expect_script_list(vec![
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
        ])
        .expect_grapheme_cluster_break_list(vec![
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(5),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
        ])
        .expect_is_control_list(vec![
            false, false, false, false, false, false, false, false, false, false, false, false,
            true, false, false, false, false, false, false, false, false, false, false, false,
        ])
        .expect_contributes_to_shaping_list(vec![
            true, true, true, true, true, true, true, true, true, true, true, true, false,
            true, true, true, true, true, true, true, true, true, true, true,
        ]);
}

#[test]
fn test_single_char() {
    verify_analysis("A", |_| {}).expect_boundary_list(vec![Boundary::Word]);
}

#[test]
fn test_rtl_paragraph_with_non_authoritative_logical_first_char_two_paragraphs() {
    verify_analysis("حدا\u{64b} \nحدا\u{64b} ", |_| {})
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::Word,
            Boundary::Word,
            Boundary::Mandatory,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::Word,
        ])
        .expect_bidi_embed_level_list(vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1])
        .expect_script_list(vec![
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(1),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(1),
            Script::from_icu4c_value(0),
        ])
        .expect_grapheme_cluster_break_list(vec![
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(3),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(5),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(3),
            GraphemeClusterBreak::from_icu4c_value(0),
        ])
        .expect_is_control_list(vec![
            false, false, false, false, false, true, false, false, false, false, false,
        ])
        .expect_contributes_to_shaping_list(vec![
            true, true, true, true, true, false, true, true, true, true, true,
        ])
        .expect_force_normalize_list(vec![
            false, false, false, true, false, false, false, false, false, true, false,
        ]);
}

#[test]
fn test_three_chars() {
    verify_analysis("ABC", |builder| {
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 0..3);
    })
        .expect_boundary_list(vec![Boundary::Word, Boundary::None, Boundary::None]);
}

#[test]
fn test_single_char_multi_byte() {
    verify_analysis("€", |builder| {
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::KeepAll), 0..3);
    })
        .expect_boundary_list(vec![Boundary::Word])
        .expect_bidi_embed_level_list(vec![0])
        .expect_script_list(vec![Script::from_icu4c_value(0)])
        .expect_grapheme_cluster_break_list(vec![GraphemeClusterBreak::from_icu4c_value(0)]);
}

#[test]
fn test_rtl_paragraph_with_non_authoritative_logical_first_character() {
    verify_analysis("حدا\u{64b} ", |_| {})
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::None,
            Boundary::None,
            Boundary::None,
            Boundary::Word,
        ])
        .expect_bidi_embed_level_list(vec![1, 1, 1, 1, 1])
        .expect_script_list(vec![
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(2),
            Script::from_icu4c_value(1),
            Script::from_icu4c_value(0),
        ])
        .expect_grapheme_cluster_break_list(vec![
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(0),
            GraphemeClusterBreak::from_icu4c_value(3),
            GraphemeClusterBreak::from_icu4c_value(0),
        ])
        .expect_is_control_list(vec![false, false, false, false, false])
        .expect_contributes_to_shaping_list(vec![true, true, true, true, true])
        .expect_force_normalize_list(vec![false, false, false, true, false]);
}

#[test]
fn test_two_newlines() {
    verify_analysis("\n\n", |_| {})
        .expect_boundary_list(vec![Boundary::Word, Boundary::Mandatory])
        .expect_bidi_embed_level_list(vec![0, 0])
        .expect_script_list(vec![
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
        ])
        .expect_grapheme_cluster_break_list(vec![
            GraphemeClusterBreak::from_icu4c_value(5),
            GraphemeClusterBreak::from_icu4c_value(5),
        ]);
}

#[test]
fn test_newline() {
    verify_analysis("\n", |_| {})
        .expect_boundary_list(vec![Boundary::Word])
        .expect_bidi_embed_level_list(vec![0])
        .expect_script_list(vec![Script::from_icu4c_value(0)])
        .expect_grapheme_cluster_break_list(vec![GraphemeClusterBreak::from_icu4c_value(5)]);
}

#[test]
fn test_two_chars_keep_all() {
    verify_analysis("AB", |builder| {
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::KeepAll), 0..2);
    })
        .expect_boundary_list(vec![Boundary::Word, Boundary::None])
        .expect_bidi_embed_level_list(vec![0, 0])
        .expect_script_list(vec![
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(25),
        ]);
}

#[test]
fn test_whitespace_contiguous_interspersed_in_latin() {
    verify_analysis("A  B  C D", |_| {})
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::Word,
            Boundary::None,
            Boundary::Line,
            Boundary::Word,
            Boundary::None,
            Boundary::Line,
            Boundary::Word,
            Boundary::Line,
        ])
        .expect_script_list(vec![
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
        ]);
}

#[test]
fn test_whitespace_contiguous_interspersed_in_latin_mixed() {
    verify_analysis("A  B  C D", |builder| {
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::KeepAll), 0..3);
        builder.push(StyleProperty::WordBreak(LineBreakWordOption::Normal), 3..9);
    })
        .expect_boundary_list(vec![
            Boundary::Word,
            Boundary::Word,
            Boundary::None,
            Boundary::Line,
            Boundary::Word,
            Boundary::None,
            Boundary::Line,
            Boundary::Word,
            Boundary::Line,
        ])
        .expect_script_list(vec![
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
            Script::from_icu4c_value(0),
            Script::from_icu4c_value(25),
        ]);
}