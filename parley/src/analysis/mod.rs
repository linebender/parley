pub(crate) mod cluster;
mod provider;

use crate::analysis::provider::{COMPOSITE_BLOB, PROVIDER};
use crate::resolve::RangedStyle;
use crate::{Brush, LayoutContext};
use icu_collections::codepointtrie::TrieValue;
use icu_normalizer::{
    ComposingNormalizer, ComposingNormalizerBorrowed, DecomposingNormalizer,
    DecomposingNormalizerBorrowed,
};
use icu_properties::props::{
    BasicEmoji, BidiClass, Emoji, ExtendedPictographic, GeneralCategory, GraphemeClusterBreak,
    LineBreak, RegionalIndicator, Script, VariationSelector,
};
use icu_properties::{
    CodePointMapData, CodePointMapDataBorrowed, CodePointSetData, CodePointSetDataBorrowed,
    EmojiSetData, EmojiSetDataBorrowed,
};
use icu_provider::DataMarker;
use icu_provider::buf::AsDeserializingBufferProvider;
use icu_provider::{DataRequest, DataResponse, DynamicDataProvider};
use icu_segmenter::options::{LineBreakOptions, LineBreakWordOption, WordBreakOptions};
use icu_segmenter::{
    GraphemeClusterSegmenter, GraphemeClusterSegmenterBorrowed, LineSegmenter,
    LineSegmenterBorrowed, WordSegmenter, WordSegmenterBorrowed,
};
use std::marker::PhantomData;
use unicode_bidi::TextSource;
use unicode_data::{CompositePropsV1, CompositePropsV1Data};

pub(crate) struct AnalysisDataSources {
    grapheme_segmenter: GraphemeClusterSegmenter,
    word_segmenter: WordSegmenter,
    line_segmenters: LineSegmenters,
    composing_normalizer: ComposingNormalizer,
    decomposing_normalizer: DecomposingNormalizer,

    composite: DataResponse<CompositePropsV1>,
}

#[derive(Default)]
struct LineSegmenters {
    normal: Option<LineSegmenter>,
    keep_all: Option<LineSegmenter>,
    break_all: Option<LineSegmenter>,
}

impl LineSegmenters {
    fn get(&mut self, word_break_strength: LineBreakWordOption) -> LineSegmenterBorrowed<'_> {
        let segmenter = match word_break_strength {
            LineBreakWordOption::Normal => &mut self.normal,
            LineBreakWordOption::KeepAll => &mut self.keep_all,
            LineBreakWordOption::BreakAll => &mut self.break_all,
            _ => unreachable!(),
        };

        segmenter
            .get_or_insert_with(|| {
                let mut line_break_opts = LineBreakOptions::default();
                line_break_opts.word_option = Some(word_break_strength);
                LineSegmenter::try_new_auto_unstable(&PROVIDER, line_break_opts)
                    .expect("Failed to create LineSegmenter")
            })
            .as_borrowed()
    }
}

impl AnalysisDataSources {
    pub(crate) fn new() -> Self {
        let blob =
            icu_provider_blob::BlobDataProvider::try_new_from_static_blob(COMPOSITE_BLOB).unwrap();
        //let composite_props = blob.load_data(CompositePropsV1::INFO, DataRequest::default()).unwrap();
        // Convert blob (BufferProvider) to a typed DataProvider via deserialization:
        let dp = blob.as_deserializing();

        // Load typed data:
        let composite: DataResponse<CompositePropsV1> = dp
            .load_data(CompositePropsV1::INFO, DataRequest::default())
            .unwrap();

        Self {
            grapheme_segmenter: GraphemeClusterSegmenter::try_new_unstable(&PROVIDER).unwrap(),
            word_segmenter: WordSegmenter::try_new_lstm_unstable(
                &PROVIDER,
                WordBreakOptions::default(),
            )
            .unwrap(),
            line_segmenters: LineSegmenters::default(),
            composing_normalizer: ComposingNormalizer::try_new_nfc_unstable(&PROVIDER).unwrap(),
            decomposing_normalizer: DecomposingNormalizer::try_new_nfd_unstable(&PROVIDER).unwrap(),
            composite,
        }
    }

    pub(crate) fn composite(&self) -> &CompositePropsV1Data<'_> {
        self.composite.payload.get()
    }

    pub(crate) fn grapheme_segmenter(&self) -> GraphemeClusterSegmenterBorrowed<'_> {
        self.grapheme_segmenter.as_borrowed()
    }

    fn word_segmenter(&self) -> WordSegmenterBorrowed<'_> {
        self.word_segmenter.as_borrowed()
    }

    fn composing_normalizer(&self) -> ComposingNormalizerBorrowed<'_> {
        self.composing_normalizer.as_borrowed()
    }

    fn decomposing_normalizer(&self) -> DecomposingNormalizerBorrowed<'_> {
        self.decomposing_normalizer.as_borrowed()
    }
}

type BidiLevel = u8;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct CharInfo {
    /// The line/word breaking boundary classification of this character.
    pub boundary: Boundary,
    /// The bidirectional embedding level of the character (even = LTR, odd = RTL).
    pub bidi_embed_level: BidiLevel,
    /// The Unicode script this character belongs to.
    pub script: Script,
    /// The grapheme cluster boundary property of this character.
    pub grapheme_cluster_break: GraphemeClusterBreak,

    flags: u8,
}

impl CharInfo {
    const VARIATION_SELECTOR_SHIFT: u8 = 0;
    const REGION_INDICATOR_SHIFT: u8 = 1;
    const CONTROL_SHIFT: u8 = 2;
    const EMOJI_OR_PICTOGRAPH_SHIFT: u8 = 3;
    const CONTRIBUTES_TO_SHAPING_SHIFT: u8 = 4;
    const FORCE_NORMALIZE_SHIFT: u8 = 5;

    const VARIATION_SELECTOR_MASK: u8 = 1 << Self::VARIATION_SELECTOR_SHIFT;
    const REGION_INDICATOR_MASK: u8 = 1 << Self::REGION_INDICATOR_SHIFT;
    const CONTROL_MASK: u8 = 1 << Self::CONTROL_SHIFT;
    const EMOJI_OR_PICTOGRAPH_MASK: u8 = 1 << Self::EMOJI_OR_PICTOGRAPH_SHIFT;
    const CONTRIBUTES_TO_SHAPING_MASK: u8 = 1 << Self::CONTRIBUTES_TO_SHAPING_SHIFT;
    const FORCE_NORMALIZE_MASK: u8 = 1 << Self::FORCE_NORMALIZE_SHIFT;

    fn new(
        boundary: Boundary,
        bidi_embed_level: BidiLevel,
        script: Script,
        grapheme_cluster_break: GraphemeClusterBreak,
        is_variation_selector: bool,
        is_region_indicator: bool,
        is_control: bool,
        is_emoji_or_pictograph: bool,
        contributes_to_shaping: bool,
        force_normalize: bool,
    ) -> Self {
        Self {
            boundary,
            bidi_embed_level,
            script,
            grapheme_cluster_break,
            flags: (is_variation_selector as u8) << Self::VARIATION_SELECTOR_SHIFT
                | (is_region_indicator as u8) << Self::REGION_INDICATOR_SHIFT
                | (is_control as u8) << Self::CONTROL_SHIFT
                | (is_emoji_or_pictograph as u8) << Self::EMOJI_OR_PICTOGRAPH_SHIFT
                | (contributes_to_shaping as u8) << Self::CONTRIBUTES_TO_SHAPING_SHIFT
                | (force_normalize as u8) << Self::FORCE_NORMALIZE_SHIFT,
        }
    }

    #[inline(always)]
    pub fn is_variation_selector(&self) -> bool {
        self.flags & Self::VARIATION_SELECTOR_MASK != 0
    }

    #[inline(always)]
    pub fn is_region_indicator(&self) -> bool {
        self.flags & Self::REGION_INDICATOR_MASK != 0
    }

    #[inline(always)]
    pub fn is_control(&self) -> bool {
        self.flags & Self::CONTROL_MASK != 0
    }

    #[inline(always)]
    pub fn is_emoji_or_pictograph(&self) -> bool {
        self.flags & Self::EMOJI_OR_PICTOGRAPH_MASK != 0
    }

    #[inline(always)]
    pub fn contributes_to_shaping(&self) -> bool {
        self.flags & Self::CONTRIBUTES_TO_SHAPING_MASK != 0
    }

    #[inline(always)]
    pub fn force_normalize(&self) -> bool {
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
        char_indices: std::str::CharIndices<'a>,
        current_char: (usize, char),
        building_range_start: usize,
        previous_word_break_style: LineBreakWordOption,
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
        type Item = (&'a str, LineBreakWordOption, bool);

        fn next(&mut self) -> Option<Self::Item> {
            if self.done {
                return None;
            }

            while let Some(style) = self.styles.next() {
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
        WordBreakSegmentIter::new(text, rest.iter(), &first_style);
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
    let boundary_iter = text.char_indices().map(|(byte_pos, _)| {
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

        boundary
    });

    let composite = lcx.analysis_data_sources.composite();

    let mut needs_bidi_resolution = false;

    lcx.info.reserve(text.len());
    boundary_iter
        .zip(text.chars())
        // Shift line break data forward one, as line boundaries corresponding with line-breaking
        // characters (like '\n') exist at an index position one higher than the respective
        // character's index, but we need our iterators to align, and the rest are simply
        // character-indexed.
        .fold(false, |is_mandatory_linebreak, (boundary, ch)| {
            let properties = composite.properties(ch as u32);
            let grapheme_cluster_break = properties.grapheme_cluster_break();
            let general_category = properties.general_category();
            let script = properties.script();
            let is_emoji_or_pictograph = properties.is_emoji_or_pictograph();
            let is_variation_selector = properties.is_variation_selector();
            let is_region_indicator = properties.is_region_indicator();
            let bidi_embed_level: BidiLevel = 0;
            let next_mandatory_linebreak = properties.is_mandatory_linebreak();

            let boundary = if is_mandatory_linebreak {
                Boundary::Mandatory
            } else {
                boundary
            };

            let is_control = matches!(general_category, GeneralCategory::Control);
            let contributes_to_shaping = !is_control
                || (matches!(general_category, GeneralCategory::Format)
                    && !matches!(script, Script::Inherited));
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

            lcx.info.push((
                CharInfo::new(
                    boundary,
                    bidi_embed_level,
                    script,
                    grapheme_cluster_break,
                    is_variation_selector,
                    is_region_indicator,
                    is_control,
                    is_emoji_or_pictograph,
                    contributes_to_shaping,
                    force_normalize,
                ),
                0, // Style index is populated later
            ));

            needs_bidi_resolution |= properties.needs_bidi_resolution();

            return next_mandatory_linebreak;
        });

    if needs_bidi_resolution {
        let bidi_embedding_levels = unicode_bidi::BidiInfo::new_with_data_source(
            lcx.analysis_data_sources.composite(),
            text,
            None,
        )
        .levels;

        for ((byte_pos, _), (info, _)) in text.char_indices().zip(lcx.info.iter_mut()) {
            info.bidi_embed_level = bidi_embedding_levels[byte_pos].into();
        }
    }

    // Restore line segmenters
    lcx.analysis_data_sources.line_segmenters = line_segmenters;
}

#[cfg(test)]
mod tests {
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
}
