// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The analyzer API.

use core::ops::Range;

use parlance::{BaseDirection, Language, LineBreak, WordBreak};

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
    ///
    /// # Panics
    ///
    /// If the provider in `data_sources` returns an error other than `IdentifierNotFound`.
    pub fn analyze(
        &mut self,
        text: &str,
        options: &AnalysisOptions<'_>,
        analysis: &mut Analysis,
        data_sources: &crate::AnalysisDataSources<'_>,
    ) {
        analysis.clear();
        analyze_text(self, text, options, analysis, data_sources);
    }
}

/// Configuration of line break opportunities for a range of text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct LineBreakConfig {
    /// CSS `word-break`.
    pub word_break: WordBreak,
    /// CSS `line-break`.
    pub line_break: LineBreak,
    /// The content language.
    ///
    /// Chinese and Japanese content uses tailored line breaking rules, which for example allow
    /// breaks before small kana and some punctuation under [`LineBreak::Normal`] and
    /// [`LineBreak::Loose`].
    pub language: Option<Language>,
}

/// Options controlling [`Analyzer::analyze`].
#[derive(Clone, Copy, Default)]
pub struct AnalysisOptions<'a> {
    /// The paragraph's base direction.
    ///
    /// Defaults to [`BaseDirection::Auto`], which infers the direction from the text.
    pub base_direction: BaseDirection,

    /// Line breaking configuration for ranges of the source text.
    ///
    /// Ranges must be sorted and non-overlapping, and must start and end on character boundaries of
    /// the text. Empty ranges are ignored. Gaps use [`LineBreakConfig::default`].
    pub line_break: &'a [(Range<usize>, LineBreakConfig)],

    /// Ranges of the source text in which a soft wrap opportunity follows every space, tab, and
    /// ideographic space.
    ///
    /// This implements the additional soft wrap opportunities of
    /// [CSS's white-space-collapse: break-spaces][css-break-spaces].
    ///
    /// Outside these ranges, [UAX #14 § 6][uax-14-algorithm] is followed, which defines where soft
    /// wrap opportunities exist. For example, rules LB7 and LB18 give a sequence of spaces an
    /// opportunity only at its end. The ranges specified here add an opportunity after every space,
    /// tab, and ideographic space, except directly before a mandatory break. This overrides the
    /// "non-tailorable" UAX #14 Rule LB7, and so deviates from Unicode's line breaking algorithm.
    ///
    /// Ranges must be sorted and non-overlapping, and must start and end on character boundaries
    /// of the text. Empty ranges are ignored. Gaps apply the default rules.
    ///
    /// [css-break-spaces]: https://www.w3.org/TR/css-text-4/#valdef-white-space-collapse-break-spaces
    /// [uax-14-algorithm]: https://unicode.org/reports/tr14/#Algorithm
    pub break_spaces: &'a [Range<usize>],

    /// The callback which will be called as a first provider of line breaking decisions.
    ///
    /// See [`LineBreakOverrideFn`] for more details.
    pub line_break_override: Option<&'a LineBreakOverrideFn>,

    /// The inline objects in the source text.
    ///
    /// These must be in ascending order of [`InlineObject::index`] without duplicates. Each entry
    /// must point at a U+FFFC OBJECT REPLACEMENT CHARACTER in the source text. Any U+FFFC character
    /// that is not in this list is treated as ordinary text (some fonts may render `.notdef`,
    /// others may have a glyph for it).
    ///
    /// Such an inline object is part of the paragraph, but is not part of the paragraph's text;
    /// e.g., it may be an image. In CSS, this is known as an [atomic inline][css-atomic-inline].
    ///
    /// The analysis marks these objects with
    /// [`CharInfo::is_inline_object`](crate::CharInfo::is_inline_object), and they are treated
    /// as follows.
    ///
    /// - The soft wrap opportunities before and after an object can be configured using
    ///   [`break_before`](InlineObject::break_before) and
    ///   [`break_after`](InlineObject::break_after). A side left unconfigured follows
    ///   [UAX #14 LB20][uax-14-lb20]: there's an opportunity, unless suppressed (e.g. before a full
    ///   stop).
    /// - Each object is a grapheme of its own.
    /// - Each object is shaped as a [`ShapedRun`](crate::ShapedRun) of its own. It is a
    ///   [`ShapedCluster`](crate::shape::ShapedCluster) without glyphs, whose advance can only be
    ///   set using
    ///   [`ShapedText::set_inline_object_advance`](crate::ShapedText::set_inline_object_advance).
    ///
    /// [css-atomic-inline]: https://www.w3.org/TR/css-display-3/#atomic-inline
    /// [uax-14-lb20]: https://unicode.org/reports/tr14/#LB20
    pub inline_objects: &'a [InlineObject],
}

/// An inline object in the source text, see [`AnalysisOptions::inline_objects`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InlineObject {
    /// The byte index of the object's U+FFFC OBJECT REPLACEMENT CHARACTER in the source text.
    pub text_byte_start: usize,

    /// Whether there is a soft wrap opportunity before the object.
    ///
    /// `None` follows UAX #14. `Some(true)` forces one and `Some(false)` suppresses one.
    ///
    /// Between two objects, a `Some(false)` of either one wins.
    /// [`AnalysisOptions::line_break_override`] takes precedence over this.
    pub break_before: Option<bool>,

    /// Whether there is a soft wrap opportunity after the object.
    ///
    /// This works like [`Self::break_before`].
    pub break_after: Option<bool>,
}

impl InlineObject {
    /// An object at byte `text_byte_start`.
    pub const fn new(text_byte_start: usize) -> Self {
        Self {
            text_byte_start,
            break_before: None,
            break_after: None,
        }
    }
}

impl core::fmt::Debug for AnalysisOptions<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AnalysisOptions")
            .field("base_direction", &self.base_direction)
            .field("line_break", &self.line_break)
            .field("break_spaces", &self.break_spaces)
            .field("inline_objects", &self.inline_objects)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;
    use parlance::{BidiLevel, LineBreak};

    use super::{AnalysisOptions, Analyzer, InlineObject, LineBreakConfig};
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
            &crate::AnalysisDataSources::default(),
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
    fn inline_objects() {
        fn objects(inline_objects: &[InlineObject]) -> AnalysisOptions<'_> {
            AnalysisOptions {
                inline_objects,
                ..AnalysisOptions::default()
            }
        }
        let analyze_with = |text: &str, options: &AnalysisOptions<'_>| {
            let mut analysis = Analysis::new();
            Analyzer::new().analyze(
                text,
                options,
                &mut analysis,
                &crate::AnalysisDataSources::default(),
            );
            analysis
        };
        let opportunities = |text: &str, options: &AnalysisOptions<'_>| -> Vec<usize> {
            let analysis = analyze_with(text, options);
            let infos = analysis.char_info().iter().enumerate();
            infos
                .filter_map(|(i, info)| info.is_soft_wrap_opportunity().then_some(i))
                .collect()
        };
        let (n, t, f) = (None, Some(true), Some(false));
        let obj = |index, break_before, break_after| InlineObject {
            text_byte_start: index,
            break_before,
            break_after,
        };

        for (text, inline_objects, expected) in [
            // By default, an object breaks like any U+FFFC under UAX #14 (LB20), so not before a
            // full stop (LB15d).
            ("a\u{FFFC}.", &[obj(1, n, n)][..], &[1][..]),
            // An object's own conditions win over the text's.
            ("a\u{FFFC}.", &[obj(1, f, t)], &[2]),
            // Between two objects, suppressing an opportunity wins.
            ("\u{FFFC}\u{FFFC}a", &[obj(0, n, t), obj(3, f, n)], &[2]),
            ("\u{FFFC}\u{FFFC}a", &[obj(0, n, f), obj(3, t, n)], &[2]),
            // There is never one next to a mandatory break.
            ("\n\u{FFFC}", &[obj(1, t, n)], &[]),
        ] {
            assert_eq!(opportunities(text, &objects(inline_objects)), expected);
        }

        // `line-break: anywhere` breaks between graphemes, so after an object too. A line break
        // override has the last word on either side of one.
        let object_at_0 = [obj(0, n, n)];
        let anywhere = LineBreakConfig {
            line_break: LineBreak::Anywhere,
            ..LineBreakConfig::default()
        };
        let options = AnalysisOptions {
            line_break: &[(0..5, anywhere)],
            ..objects(&object_at_0)
        };
        assert_eq!(opportunities("\u{FFFC}\u{301}", &options), [1]);
        let object_at_1 = [obj(1, t, t)];
        let options = AnalysisOptions {
            line_break_override: Some(&|_| Some(false)),
            ..objects(&object_at_1)
        };
        assert_eq!(opportunities("a\u{FFFC}a", &options), []);

        // An object is a grapheme of its own, and needs no font.
        let analysis = analyze_with("\u{FFFC}\u{301}", &objects(&object_at_0));
        let [object, mark] = analysis.char_info() else {
            unreachable!()
        };
        assert!(object.is_inline_object() && !object.contributes_to_shaping());
        assert!(mark.is_grapheme_start() && !mark.is_inline_object());
    }

    #[test]
    fn explicit_direction_applies_to_empty_text() {
        let analysis = analyze("", BaseDirection::Rtl);

        assert!(analysis.paragraph_level().is_rtl());
        assert!(analysis.bidi_levels().is_empty());
    }
}
