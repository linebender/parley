// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The analyzer API.

use parlance::BaseDirection;

use crate::bidi::BidiResolver;

use crate::analysis::{Analysis, analyze_text};

/// Reusable scratch for [`Analyzer::analyze`].
#[derive(Default)]
pub struct Analyzer {
    pub(crate) bidi: BidiResolver,
}

impl core::fmt::Debug for Analyzer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Analyzer").finish_non_exhaustive()
    }
}

impl Analyzer {
    /// Creates a new analyzer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Analyze `text`, overwriting `analysis`.
    ///
    /// This reuses the allocations of `analysis`.
    pub fn analyze(&mut self, text: &str, options: &AnalysisOptions<'_>, analysis: &mut Analysis) {
        analysis.clear();
        analyze_text(self, text, options, analysis);
    }
}

/// Options controlling [`Analyzer::analyze`].
#[derive(Clone, Copy, Default)]
pub struct AnalysisOptions<'a> {
    /// The paragraph's base direction.
    ///
    /// Defaults to [`BaseDirection::Auto`], which infers the direction from the text.
    pub base_direction: BaseDirection,
    /// Sorted UTF-8 byte offsets at which a soft line break is allowed.
    ///
    /// Each offset is before the character that starts there and must be both a
    /// character boundary and a grapheme-cluster boundary. An empty slice disables
    /// soft wrapping; mandatory line breaks are still detected by the analyzer.
    /// APIs that report UTF-16 indices, such as JavaScript's `Intl.Segmenter`, must
    /// have their results converted to UTF-8 byte offsets by the caller.
    ///
    /// # Panics
    ///
    /// [`Analyzer::analyze`] panics if these offsets are not sorted, do not fall on
    /// UTF-8 character boundaries, or fall within a grapheme cluster.
    pub line_break_opportunities: &'a [usize],
}

impl core::fmt::Debug for AnalysisOptions<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AnalysisOptions")
            .field("base_direction", &self.base_direction)
            .field("line_break_opportunities", &self.line_break_opportunities)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use parlance::BidiLevel;

    use super::{AnalysisOptions, Analyzer};
    use crate::{Analysis, BaseDirection, Boundary};

    fn analyze(text: &str, base_direction: BaseDirection) -> Analysis {
        let mut analyzer = Analyzer::new();
        let mut analysis = Analysis::new();
        analyzer.analyze(
            text,
            &AnalysisOptions {
                base_direction,
                ..AnalysisOptions::default()
            },
            &mut analysis,
        );
        analysis
    }

    #[test]
    fn explicit_rtl_resolves_numeric_and_neutral_text() {
        let text = "123 / 456";
        let auto = analyze(text, BaseDirection::Auto);
        let rtl = analyze(text, BaseDirection::Rtl);

        assert_eq!(auto.paragraph_level(), BidiLevel::new(0));
        assert!(auto.paragraph_level().is_ltr());
        assert!(auto.bidi_levels().is_empty());

        assert_eq!(rtl.paragraph_level(), BidiLevel::new(1));
        assert!(rtl.paragraph_level().is_rtl());
        assert_eq!(rtl.bidi_levels().len(), text.chars().count());
        for (ch, level) in text.chars().zip(rtl.bidi_levels()) {
            if ch.is_ascii_digit() {
                assert!(level.is_ltr());
            }
        }
        assert!(rtl.bidi_levels().iter().any(|level| level.is_rtl()));
    }

    #[test]
    fn explicit_ltr_takes_precedence_over_first_strong_direction() {
        let text = "مرحبا hello";
        let auto = analyze(text, BaseDirection::Auto);
        let ltr = analyze(text, BaseDirection::Ltr);

        assert!(auto.paragraph_level().is_rtl());
        assert!(ltr.paragraph_level().is_ltr());
        assert_ne!(auto.bidi_levels(), ltr.bidi_levels());
    }

    #[test]
    fn explicit_rtl_preserves_ltr_run_direction() {
        let analysis = analyze("hello", BaseDirection::Rtl);

        assert!(analysis.paragraph_level().is_rtl());
        assert!(analysis.bidi_levels().iter().all(|level| level.is_ltr()));
    }

    #[test]
    fn explicit_direction_applies_to_empty_text() {
        let analysis = analyze("", BaseDirection::Rtl);

        assert!(analysis.paragraph_level().is_rtl());
        assert!(analysis.bidi_levels().is_empty());
    }

    #[test]
    fn caller_provided_line_breaks_are_used() {
        let mut analyzer = Analyzer::new();
        let mut analysis = Analysis::new();
        analyzer.analyze(
            "abc",
            &AnalysisOptions {
                line_break_opportunities: &[1],
                ..AnalysisOptions::default()
            },
            &mut analysis,
        );
        assert_eq!(analysis.char_info()[1].boundary, Boundary::Line);
    }

    #[test]
    fn empty_line_breaks_disable_soft_wrapping() {
        let analysis = analyze("abc def", BaseDirection::Auto);
        assert!(
            analysis
                .char_info()
                .iter()
                .all(|info| info.boundary == Boundary::None)
        );
    }

    #[test]
    #[should_panic(expected = "must be sorted")]
    fn rejects_unsorted_line_breaks() {
        let mut analyzer = Analyzer::new();
        analyzer.analyze(
            "abc",
            &AnalysisOptions {
                line_break_opportunities: &[2, 1],
                ..AnalysisOptions::default()
            },
            &mut Analysis::new(),
        );
    }

    #[test]
    #[should_panic(expected = "UTF-8 character boundaries")]
    fn rejects_non_character_line_breaks() {
        let mut analyzer = Analyzer::new();
        analyzer.analyze(
            "aé",
            &AnalysisOptions {
                line_break_opportunities: &[2],
                ..AnalysisOptions::default()
            },
            &mut Analysis::new(),
        );
    }

    #[test]
    #[should_panic(expected = "grapheme-cluster boundaries")]
    fn rejects_mid_grapheme_line_breaks() {
        let mut analyzer = Analyzer::new();
        analyzer.analyze(
            "a\u{301}",
            &AnalysisOptions {
                line_break_opportunities: &[1],
                ..AnalysisOptions::default()
            },
            &mut Analysis::new(),
        );
    }
}
