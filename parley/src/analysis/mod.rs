// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub(crate) mod cluster;
mod provider;

use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::analysis::provider::PROVIDER;
use crate::resolve::RangedStyle;
use crate::{Brush, LayoutContext, WordBreak};

use icu_normalizer::properties::{
    CanonicalComposition, CanonicalCompositionBorrowed, CanonicalDecomposition,
    CanonicalDecompositionBorrowed,
};
use icu_properties::props::{BidiMirroringGlyph, GeneralCategory, GraphemeClusterBreak, Script};
use icu_properties::{
    CodePointMapData, CodePointMapDataBorrowed, PropertyNamesShort, PropertyNamesShortBorrowed,
};
use icu_segmenter::options::{LineBreakOptions, LineBreakWordOption, WordBreakOptions};
use icu_segmenter::{
    GraphemeClusterSegmenter, GraphemeClusterSegmenterBorrowed, LineSegmenter,
    LineSegmenterBorrowed, WordSegmenter, WordSegmenterBorrowed,
};
use unicode_bidi::TextSource;
use unicode_data::CompositeProps;

pub(crate) struct AnalysisDataSources {
    grapheme_segmenter: GraphemeClusterSegmenter,
    word_segmenter: WordSegmenter,
    line_segmenters: LineSegmenters,
    composing_normalizer: CanonicalComposition,
    decomposing_normalizer: CanonicalDecomposition,
    script_short_name: PropertyNamesShort<Script>,
    brackets: CodePointMapData<BidiMirroringGlyph>,

    composite: CompositeProps,
}

#[derive(Default)]
struct LineSegmenters {
    normal: Option<LineSegmenter>,
    keep_all: Option<LineSegmenter>,
    break_all: Option<LineSegmenter>,
}

impl LineSegmenters {
    fn get(&mut self, word_break_strength: WordBreak) -> LineSegmenterBorrowed<'_> {
        let segmenter = match word_break_strength {
            WordBreak::Normal => &mut self.normal,
            WordBreak::KeepAll => &mut self.keep_all,
            WordBreak::BreakAll => &mut self.break_all,
        };

        segmenter
            .get_or_insert_with(|| {
                let mut line_break_opts = LineBreakOptions::default();
                let word_break_strength_icu = match word_break_strength {
                    WordBreak::Normal => LineBreakWordOption::Normal,
                    WordBreak::BreakAll => LineBreakWordOption::BreakAll,
                    WordBreak::KeepAll => LineBreakWordOption::KeepAll,
                };
                line_break_opts.word_option = Some(word_break_strength_icu);
                LineSegmenter::try_new_auto_unstable(&PROVIDER, line_break_opts)
                    .expect("Failed to create LineSegmenter")
            })
            .as_borrowed()
    }
}

impl AnalysisDataSources {
    pub(crate) fn new() -> Self {
        Self {
            grapheme_segmenter: GraphemeClusterSegmenter::try_new_unstable(&PROVIDER).unwrap(),
            word_segmenter: WordSegmenter::try_new_lstm_unstable(
                &PROVIDER,
                WordBreakOptions::default(),
            )
            .unwrap(),
            line_segmenters: LineSegmenters::default(),
            composing_normalizer: CanonicalComposition::try_new_unstable(&PROVIDER).unwrap(),
            decomposing_normalizer: CanonicalDecomposition::try_new_unstable(&PROVIDER).unwrap(),
            script_short_name: PropertyNamesShort::<Script>::try_new_unstable(&PROVIDER).unwrap(),
            brackets: CodePointMapData::<BidiMirroringGlyph>::try_new_unstable(&PROVIDER).unwrap(),
            composite: CompositeProps,
        }
    }

    pub(crate) fn composite(&self) -> &CompositeProps {
        &self.composite
    }

    pub(crate) fn grapheme_segmenter(&self) -> GraphemeClusterSegmenterBorrowed<'_> {
        self.grapheme_segmenter.as_borrowed()
    }

    fn word_segmenter(&self) -> WordSegmenterBorrowed<'_> {
        self.word_segmenter.as_borrowed()
    }

    fn composing_normalizer(&self) -> CanonicalCompositionBorrowed<'_> {
        self.composing_normalizer.as_borrowed()
    }

    fn decomposing_normalizer(&self) -> CanonicalDecompositionBorrowed<'_> {
        self.decomposing_normalizer.as_borrowed()
    }

    pub(crate) fn script_short_name(&self) -> PropertyNamesShortBorrowed<'_, Script> {
        self.script_short_name.as_borrowed()
    }

    pub(crate) fn brackets(&self) -> CodePointMapDataBorrowed<'_, BidiMirroringGlyph> {
        self.brackets.as_borrowed()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct CharInfo {
    /// The line/word breaking boundary classification of this character.
    pub boundary: Boundary,
    /// The Unicode script this character belongs to.
    pub script: Script,
    /// The grapheme cluster boundary property of this character.
    pub grapheme_cluster_break: GraphemeClusterBreak,
    /// The impact this character has on directionality.
    pub bidi_class: icu_properties::props::BidiClass,
    /// Whether or not the character is a bracket, plus mirror data if so.
    pub bracket: BidiMirroringGlyph,

    flags: u8,
}

impl CharInfo {
    const VARIATION_SELECTOR_SHIFT: u8 = 0;
    const REGION_INDICATOR_SHIFT: u8 = 1;
    const CONTROL_SHIFT: u8 = 2;
    const EMOJI_OR_PICTOGRAPH_SHIFT: u8 = 3;
    const CONTRIBUTES_TO_SHAPING_SHIFT: u8 = 4;
    const FORCE_NORMALIZE_SHIFT: u8 = 5;

    #[allow(
        dead_code,
        reason = "To be used in more complete emoji checking, in select_font"
    )]
    const VARIATION_SELECTOR_MASK: u8 = 1 << Self::VARIATION_SELECTOR_SHIFT;
    #[allow(
        dead_code,
        reason = "To be used in more complete emoji checking, in select_font"
    )]
    const REGION_INDICATOR_MASK: u8 = 1 << Self::REGION_INDICATOR_SHIFT;
    const CONTROL_MASK: u8 = 1 << Self::CONTROL_SHIFT;
    const EMOJI_OR_PICTOGRAPH_MASK: u8 = 1 << Self::EMOJI_OR_PICTOGRAPH_SHIFT;
    const CONTRIBUTES_TO_SHAPING_MASK: u8 = 1 << Self::CONTRIBUTES_TO_SHAPING_SHIFT;
    const FORCE_NORMALIZE_MASK: u8 = 1 << Self::FORCE_NORMALIZE_SHIFT;

    fn new(
        boundary: Boundary,
        script: Script,
        grapheme_cluster_break: GraphemeClusterBreak,
        bidi_class: icu_properties::props::BidiClass,
        bracket: BidiMirroringGlyph,
        is_variation_selector: bool,
        is_region_indicator: bool,
        is_control: bool,
        is_emoji_or_pictograph: bool,
        contributes_to_shaping: bool,
        force_normalize: bool,
    ) -> Self {
        Self {
            boundary,
            script,
            grapheme_cluster_break,
            bidi_class,
            bracket,
            flags: (is_variation_selector as u8) << Self::VARIATION_SELECTOR_SHIFT
                | (is_region_indicator as u8) << Self::REGION_INDICATOR_SHIFT
                | (is_control as u8) << Self::CONTROL_SHIFT
                | (is_emoji_or_pictograph as u8) << Self::EMOJI_OR_PICTOGRAPH_SHIFT
                | (contributes_to_shaping as u8) << Self::CONTRIBUTES_TO_SHAPING_SHIFT
                | (force_normalize as u8) << Self::FORCE_NORMALIZE_SHIFT,
        }
    }

    #[allow(
        dead_code,
        reason = "To be used in more complete emoji checking, in select_font"
    )]
    #[inline(always)]
    pub(crate) fn is_variation_selector(self) -> bool {
        self.flags & Self::VARIATION_SELECTOR_MASK != 0
    }

    #[allow(
        dead_code,
        reason = "To be used in more complete emoji checking, in select_font"
    )]
    #[inline(always)]
    pub(crate) fn is_region_indicator(self) -> bool {
        self.flags & Self::REGION_INDICATOR_MASK != 0
    }

    #[inline(always)]
    pub(crate) fn is_control(self) -> bool {
        self.flags & Self::CONTROL_MASK != 0
    }

    #[inline(always)]
    pub(crate) fn is_emoji_or_pictograph(self) -> bool {
        self.flags & Self::EMOJI_OR_PICTOGRAPH_MASK != 0
    }

    #[inline(always)]
    pub(crate) fn contributes_to_shaping(self) -> bool {
        self.flags & Self::CONTRIBUTES_TO_SHAPING_MASK != 0
    }

    #[inline(always)]
    pub(crate) fn force_normalize(self) -> bool {
        self.flags & Self::FORCE_NORMALIZE_MASK != 0
    }
}

/// Boundary type of a character or cluster.
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Debug)]
#[repr(u8)]
pub(crate) enum Boundary {
    /// Not a boundary.
    None = 0,
    /// Start of a word.
    Word = 1,
    /// Potential line break.
    Line = 2,
    /// Mandatory line break.
    Mandatory = 3,
}

pub(crate) fn analyze_text<B: Brush>(lcx: &mut LayoutContext<B>, text: &str) {
    struct WordBreakSegmentIter<'a, I: Iterator, B: Brush> {
        text: &'a str,
        styles: I,
        char_indices: core::str::CharIndices<'a>,
        current_char: (usize, char),
        building_range_start: usize,
        previous_word_break_style: WordBreak,
        done: bool,
        _phantom: PhantomData<B>,
    }

    impl<'a, I, B: Brush + 'a> WordBreakSegmentIter<'a, I, B>
    where
        I: Iterator<Item = &'a RangedStyle<B>>,
    {
        fn new(text: &'a str, styles: I, first_style: &RangedStyle<B>) -> Self {
            let mut char_indices = text.char_indices();
            let current_char_len = char_indices.next().unwrap();

            Self {
                text,
                styles,
                char_indices,
                current_char: current_char_len,
                building_range_start: first_style.range.start,
                previous_word_break_style: first_style.style.word_break,
                done: false,
                _phantom: PhantomData,
            }
        }
    }

    impl<'a, I, B: Brush + 'a> Iterator for WordBreakSegmentIter<'a, I, B>
    where
        I: Iterator<Item = &'a RangedStyle<B>>,
    {
        type Item = (&'a str, WordBreak, bool);

        fn next(&mut self) -> Option<Self::Item> {
            if self.done {
                return None;
            }

            for style in self.styles.by_ref() {
                // Empty style ranges are disallowed.
                assert!(style.range.start < style.range.end);

                let style_start_index = style.range.start;
                let mut prev_char_index = self.current_char;

                // Find the character at the style boundary
                while self.current_char.0 < style_start_index {
                    prev_char_index = self.current_char;
                    self.current_char = self.char_indices.next().unwrap();
                }

                let current_word_break_style = style.style.word_break;
                if self.previous_word_break_style == current_word_break_style {
                    continue;
                }

                // Produce one substring for each different word break style run
                let prev_size = prev_char_index.1.len_utf8();
                let size = self.current_char.1.len_utf8();

                let substring = self
                    .text
                    .subrange(self.building_range_start..style_start_index + size);
                let result_style = self.previous_word_break_style;

                self.building_range_start = style_start_index - prev_size;
                self.previous_word_break_style = current_word_break_style;

                return Some((substring, result_style, false));
            }

            // Final segment
            self.done = true;
            let last_substring = self
                .text
                .subrange(self.building_range_start..self.text.len());
            Some((last_substring, self.previous_word_break_style, true))
        }
    }

    let text = if text.is_empty() { " " } else { text };

    let mut line_segmenters = core::mem::take(&mut lcx.analysis_data_sources.line_segmenters);

    // Collect boundary byte positions compactly
    let mut wb_iter = lcx
        .analysis_data_sources
        .word_segmenter()
        .segment_str(text)
        .peekable();

    // Line boundaries (word break naming refers to the line boundary determination config).
    //
    // This breaks text into sequences with similar line boundary config (part of style
    // information). If this config is consistent for all text, we use a fast path through this.
    let Some((first_style, rest)) = lcx.styles.split_first() else {
        panic!("No style info");
    };

    let contiguous_word_break_substrings =
        WordBreakSegmentIter::new(text, rest.iter(), first_style);
    let mut global_offset = 0;
    let mut line_boundary_positions: Vec<usize> = Vec::new();
    for (substring_index, (substring, word_break_strength, last)) in
        contiguous_word_break_substrings.enumerate()
    {
        // Fast path for text with a single word-break option.
        if substring_index == 0 && last {
            let mut lb_iter = line_segmenters
                .get(word_break_strength)
                .segment_str(substring);

            let _first = lb_iter.next();
            let second = lb_iter.next();
            if second.is_none() {
                continue;
            }
            let third = lb_iter.next();
            if third.is_none() {
                continue;
            }

            let iter = [second.unwrap(), third.unwrap()].into_iter().chain(lb_iter);

            line_boundary_positions.extend(iter);
            // Remove the unnecessary boundary at the end added by ICU4X.
            line_boundary_positions.pop();
            break;
        }

        let line_boundaries_iter = line_segmenters
            .get(word_break_strength)
            .segment_str(substring);

        let mut substring_chars = substring.chars();
        if substring_index != 0 {
            global_offset -= substring_chars.next().unwrap().len_utf8();
        }
        // There will always be at least two characters if we are not taking the fast path for
        // a single word break style substring.
        let last_len = substring_chars.next_back().unwrap().len_utf8();

        // Mark line boundaries (overriding word boundaries where present).
        for (index, pos) in line_boundaries_iter.enumerate() {
            // icu adds leading and trailing line boundaries, which we don't use.
            if index == 0 || pos == substring.len() {
                continue;
            }

            // For all but the last substring, we ignore line boundaries caused by the last
            // character, as this character is carried back from the next substring, and will be
            // accounted for there.
            if !last && pos == substring.len() - last_len {
                continue;
            }
            line_boundary_positions.push(pos + global_offset);
        }

        if !last {
            global_offset += substring.len() - last_len;
        }
    }

    // Merge boundaries - line takes precedence over word
    let mut lb_iter = line_boundary_positions.iter().peekable();
    let boundary_iter = text.char_indices().map(|(byte_pos, ch)| {
        // advance any stale word boundary positions
        while let Some(&w) = wb_iter.peek() {
            if w < byte_pos {
                _ = wb_iter.next();
            } else {
                break;
            }
        }
        // advance any stale line boundary positions
        while let Some(&l) = lb_iter.peek() {
            if *l < byte_pos {
                _ = lb_iter.next();
            } else {
                break;
            }
        }

        let mut boundary = Boundary::None;
        if let Some(&w) = wb_iter.peek() {
            if w == byte_pos {
                boundary = Boundary::Word;
                _ = wb_iter.next();
            }
        }
        if let Some(&l) = lb_iter.peek() {
            if *l == byte_pos {
                boundary = Boundary::Line;
                _ = lb_iter.next();
            }
        }

        (boundary, ch)
    });

    let composite = lcx.analysis_data_sources.composite();

    let mut needs_bidi_resolution = false;

    lcx.info.reserve(text.len());
    boundary_iter
        // Shift line break data forward one, as line boundaries corresponding with line-breaking
        // characters (like '\n') exist at an index position one higher than the respective
        // character's index, but we need our iterators to align, and the rest are simply
        // character-indexed.
        .fold(false, |is_mandatory_linebreak, (boundary, ch)| {
            let properties = composite.properties(ch as u32);
            let script = properties.script();
            let grapheme_cluster_break = properties.grapheme_cluster_break();
            let bidi_class = properties.bidi_class();
            let general_category = properties.general_category();
            let is_emoji_or_pictograph = properties.is_emoji_or_pictograph();
            let is_variation_selector = properties.is_variation_selector();
            let is_region_indicator = properties.is_region_indicator();
            let next_mandatory_linebreak = properties.is_mandatory_linebreak();

            let boundary = if is_mandatory_linebreak {
                Boundary::Mandatory
            } else {
                boundary
            };

            let force_normalize = {
                // "Extend" break chars should be normalized first, with two exceptions
                if matches!(grapheme_cluster_break, GraphemeClusterBreak::Extend) &&
                    ch as u32 != 0x200C && // Is not a Zero Width Non-Joiner &&
                    !is_variation_selector
                {
                    true
                } else {
                    // All spacing mark break chars should be normalized first.
                    matches!(grapheme_cluster_break, GraphemeClusterBreak::SpacingMark)
                }
            };

            needs_bidi_resolution |= properties.needs_bidi_resolution();
            let bracket = lcx.analysis_data_sources.brackets().get(ch);

            lcx.info.push((
                CharInfo::new(
                    boundary,
                    script,
                    grapheme_cluster_break,
                    bidi_class,
                    bracket,
                    is_variation_selector,
                    is_region_indicator,
                    is_control(general_category),
                    is_emoji_or_pictograph,
                    contributes_to_shaping(general_category, script),
                    force_normalize,
                ),
                0, // Style index is populated later
            ));

            next_mandatory_linebreak
        });

    if needs_bidi_resolution {
        lcx.bidi.resolve(
            text.chars().zip(
                lcx.info
                    .iter()
                    .map(|info| (info.0.bidi_class, info.0.bracket)),
            ),
            None,
        );
    }

    // Restore line segmenters
    lcx.analysis_data_sources.line_segmenters = line_segmenters;
}

/// All characters contribute to shaping except:
/// - Control characters
/// - Format characters, unless they use the "Inherited" script
pub(crate) fn contributes_to_shaping(general_category: GeneralCategory, script: Script) -> bool {
    if is_control(general_category) {
        return false;
    }

    !(general_category == GeneralCategory::Format && script != Script::Inherited)
}

fn is_control(general_category: GeneralCategory) -> bool {
    general_category == GeneralCategory::Control
}
