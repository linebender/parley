// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The analyzer API.

use core::ops::Range;

use parlance::{BaseDirection, WordBreak};

use crate::{bidi::BidiResolver, break_overrides::LineBreakOverrideFn};

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

    /// Word break configuration for ranges of the source text.
    ///
    /// Ranges must be sorted and non-overlapping. Gaps use [`WordBreak::Normal`].
    pub word_break: &'a [(Range<usize>, WordBreak)],

    /// The callback which will be called as a first provider of line breaking decisions.
    ///
    /// See [`LineBreakOverrideFn`] for more details.
    pub line_break_override: Option<&'a LineBreakOverrideFn>,
}

impl core::fmt::Debug for AnalysisOptions<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AnalysisOptions")
            .field("base_direction", &self.base_direction)
            .field("word_break", &self.word_break)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use parlance::BidiLevel;

    use super::{AnalysisOptions, Analyzer};
    use crate::{Analysis, BaseDirection};

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
}
