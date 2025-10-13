// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Context for layout.

use alloc::{vec, vec::Vec};
use std::collections::HashMap;
use std::marker::PhantomData;
use icu::collections::codepointtrie::TrieValue;
use icu::properties::props::BidiClass;
use icu::segmenter::{GraphemeClusterSegmenter, GraphemeClusterSegmenterBorrowed, LineSegmenter, LineSegmenterBorrowed, WordSegmenter, WordSegmenterBorrowed};
use icu::segmenter::options::{LineBreakOptions, LineBreakWordOption, WordBreakInvariantOptions};
use icu_properties::{CodePointMapDataBorrowed, CodePointSetData, CodePointSetDataBorrowed, EmojiSetData, EmojiSetDataBorrowed};
use icu_properties::props::{BasicEmoji, Emoji, ExtendedPictographic, GeneralCategory, GraphemeClusterBreak, LineBreak, RegionalIndicator, Script, VariationSelector};

use super::{icu_working, FontContext};
use super::bidi;
use super::builder::RangedBuilder;
use super::resolve::tree::TreeStyleBuilder;
use super::resolve::{RangedStyle, RangedStyleBuilder, ResolveContext, ResolvedStyle };
use super::style::{Brush, TextStyle};

use swash::text::cluster::{Boundary, CharInfo};
use swash::text::WordBreakStrength;
use unicode_bidi::TextSource;
use crate::bidi::BidiLevel;
use crate::builder::TreeBuilder;
use crate::inline_box::InlineBox;
use crate::shape::ShapeContext;

pub(crate) struct AnalysisDataSources {
    pub(crate) grapheme_segmenter: GraphemeClusterSegmenterBorrowed<'static>,
    pub(crate) variation_selector: CodePointSetDataBorrowed<'static>,
    pub(crate) basic_emoji: EmojiSetDataBorrowed<'static>,
    pub(crate) emoji: CodePointSetDataBorrowed<'static>,
    pub(crate) extended_pictographic: CodePointSetDataBorrowed<'static>,
    pub(crate) regional_indicator: CodePointSetDataBorrowed<'static>,

    script: CodePointMapDataBorrowed::<'static, Script>,
    general_category: CodePointMapDataBorrowed::<'static, GeneralCategory>,
    bidi_class: CodePointMapDataBorrowed::<'static, BidiClass>,
    line_break: CodePointMapDataBorrowed::<'static, LineBreak>,
    grapheme_cluster_break: CodePointMapDataBorrowed::<'static, GraphemeClusterBreak>,
    word_segmenter: WordSegmenterBorrowed<'static>,
    // Key: icu_segmenter::line::LineBreakWordOption as u8
    line_segmenters: HashMap<u8, LineSegmenterBorrowed<'static>>,
}

impl AnalysisDataSources {
    fn new() -> Self {
        Self {
            grapheme_segmenter: GraphemeClusterSegmenter::new(),
            variation_selector: CodePointSetData::new::<VariationSelector>(),
            basic_emoji: EmojiSetData::new::<BasicEmoji>(),
            emoji: CodePointSetData::new::<Emoji>(),
            extended_pictographic: CodePointSetData::new::<ExtendedPictographic>(),
            regional_indicator: CodePointSetData::new::<RegionalIndicator>(),
            script: CodePointMapDataBorrowed::<Script>::new(),
            general_category: CodePointMapDataBorrowed::<GeneralCategory>::new(),
            bidi_class: CodePointMapDataBorrowed::<BidiClass>::new(),
            line_break: CodePointMapDataBorrowed::<LineBreak>::new(),
            grapheme_cluster_break: CodePointMapDataBorrowed::<GraphemeClusterBreak>::new(),
            word_segmenter: WordSegmenter::new_auto(WordBreakInvariantOptions::default()),
            line_segmenters: HashMap::new(),
        }
    }
}

/// Shared scratch space used when constructing text layouts.
///
/// This type is designed to be a global resource with only one per-application (or per-thread).
pub struct LayoutContext<B: Brush = [u8; 4]> {
    pub(crate) bidi: bidi::BidiResolver,
    pub(crate) rcx: ResolveContext,
    pub(crate) styles: Vec<RangedStyle<B>>,
    pub(crate) inline_boxes: Vec<InlineBox>,

    // Reusable style builders (to amortise allocations)
    pub(crate) ranged_style_builder: RangedStyleBuilder<B>,
    pub(crate) tree_style_builder: TreeStyleBuilder<B>,

    pub(crate) info: Vec<(CharInfo, u16)>,
    // u16: style index for character
    pub(crate) info_icu: Vec<(icu_working::CharInfo, u16)>,
    pub(crate) scx: ShapeContext,

    // Unicode analysis data sources (provided by icu)
    pub(crate) analysis_data_sources: AnalysisDataSources,
}

impl<B: Brush> LayoutContext<B> {
    pub fn new() -> Self {
        Self {
            bidi: bidi::BidiResolver::new(),
            rcx: ResolveContext::default(),
            styles: vec![],
            inline_boxes: vec![],
            ranged_style_builder: RangedStyleBuilder::default(),
            tree_style_builder: TreeStyleBuilder::default(),
            info: vec![],
            info_icu: vec![],
            analysis_data_sources: AnalysisDataSources::new(),
            scx: ShapeContext::default(),
        }
    }

    fn resolve_style_set(
        &mut self,
        font_ctx: &mut FontContext,
        scale: f32,
        raw_style: &TextStyle<'_, B>,
    ) -> ResolvedStyle<B> {
        self.rcx
            .resolve_entire_style_set(font_ctx, raw_style, scale)
    }

    /// Create a ranged style layout builder.
    ///
    /// Set `quantize` as `true` to have the layout coordinates aligned to pixel boundaries.
    /// That is the easiest way to avoid blurry text and to receive ready-to-paint layout metrics.
    ///
    /// For advanced rendering use cases you can set `quantize` as `false` and receive
    /// fractional coordinates. This ensures the most accurate results if you want to perform
    /// some post-processing on the coordinates before painting. To avoid blurry text you will
    /// still need to quantize the coordinates just before painting.
    ///
    /// Your should round at least the following:
    /// * Glyph run baseline
    /// * Inline box baseline
    ///   - `box.y = (box.y + box.height).round() - box.height`
    /// * Selection geometry's `y0` & `y1`
    /// * Cursor geometry's `y0` & `y1`
    ///
    /// Keep in mind that for the simple `f32::round` to be effective,
    /// you need to first ensure the coordinates are in physical pixel space.
    pub fn ranged_builder<'a>(
        &'a mut self,
        fcx: &'a mut FontContext,
        text: &'a str,
        scale: f32,
        quantize: bool,
    ) -> RangedBuilder<'a, B> {
        self.begin();

        let resolved_root_style = self.resolve_style_set(fcx, scale, &TextStyle::default());
        self.ranged_style_builder
            .begin(resolved_root_style, text.len());

        fcx.source_cache.prune(128, false);

        RangedBuilder {
            scale,
            quantize,
            lcx: self,
            fcx,
        }
    }

    /// Create a tree style layout builder.
    ///
    /// Set `quantize` as `true` to have the layout coordinates aligned to pixel boundaries.
    /// That is the easiest way to avoid blurry text and to receive ready-to-paint layout metrics.
    ///
    /// For advanced rendering use cases you can set `quantize` as `false` and receive
    /// fractional coordinates. This ensures the most accurate results if you want to perform
    /// some post-processing on the coordinates before painting. To avoid blurry text you will
    /// still need to quantize the coordinates just before painting.
    ///
    /// Your should round at least the following:
    /// * Glyph run baseline
    /// * Inline box baseline
    ///   - `box.y = (box.y + box.height).round() - box.height`
    /// * Selection geometry's `y0` & `y1`
    /// * Cursor geometry's `y0` & `y1`
    ///
    /// Keep in mind that for the simple `f32::round` to be effective,
    /// you need to first ensure the coordinates are in physical pixel space.
    pub fn tree_builder<'a>(
        &'a mut self,
        fcx: &'a mut FontContext,
        scale: f32,
        quantize: bool,
        root_style: &TextStyle<'_, B>,
    ) -> TreeBuilder<'a, B> {
        self.begin();

        let resolved_root_style = self.resolve_style_set(fcx, scale, root_style);
        self.tree_style_builder.begin(resolved_root_style);

        fcx.source_cache.prune(128, false);

        TreeBuilder {
            scale,
            quantize,
            lcx: self,
            fcx,
        }
    }

    pub(crate) fn analyze_text_icu(&mut self, text: &str) {
        fn swash_to_icu_lb(swash: WordBreakStrength) -> LineBreakWordOption {
            match swash {
                WordBreakStrength::Normal => LineBreakWordOption::Normal,
                WordBreakStrength::BreakAll => LineBreakWordOption::BreakAll,
                WordBreakStrength::KeepAll => LineBreakWordOption::KeepAll,
            }
        }

        // See: https://github.com/unicode-org/icu4x/blob/ee5399a77a6b94efb5d4b60678bb458c5eedb25d/components/segmenter/src/line.rs#L338-L351
        fn is_mandatory_line_break(line_break: LineBreak) -> bool {
            matches!(line_break, LineBreak::MandatoryBreak
                | LineBreak::CarriageReturn
                | LineBreak::LineFeed
                | LineBreak::NextLine)
        }

        struct WordBreakSegmentIter<'a, I: Iterator, B: Brush> {
            text: &'a str,
            styles: I,
            char_indices: std::str::CharIndices<'a>,
            current_char_index: usize,
            building_range_start: usize,
            previous_word_break_style: WordBreakStrength,
            done: bool,
            _phantom: PhantomData<B>,
        }

        impl<'a, I, B: Brush + 'a> WordBreakSegmentIter<'a, I, B>
        where
            I: Iterator<Item = &'a RangedStyle<B>>
        {
            fn new(
                text: &'a str,
                styles: I,
                first_style: &RangedStyle<B>
            ) -> Self {
                let mut char_indices = text.char_indices();
                let current_char_len = char_indices.next().unwrap().0;

                Self {
                    text,
                    styles,
                    char_indices,
                    current_char_index: current_char_len,
                    building_range_start: first_style.range.start,
                    previous_word_break_style: first_style.style.word_break,
                    done: false,
                    _phantom: PhantomData,
                }
            }
        }

        impl<'a, I, B: Brush + 'a> Iterator for WordBreakSegmentIter<'a, I, B>
        where
            I: Iterator<Item = &'a RangedStyle<B>>
        {
            type Item = (&'a str, WordBreakStrength, bool);

            fn next(&mut self) -> Option<Self::Item> {
                if self.done {
                    return None;
                }

                // TODO(conor) avoid `while`, twice
                while let Some(style) = self.styles.next() {
                    let style_start_index = style.range.start;
                    if style_start_index == style.range.end {
                        // Skip empty style ranges
                        continue;
                    }
                    let mut prev_char_index = self.current_char_index;

                    // Find the character at the style boundary
                    while self.current_char_index < style_start_index {
                        prev_char_index = self.current_char_index;
                        self.current_char_index = self.char_indices.next().unwrap().0;
                    }

                    let current_word_break_style = style.style.word_break;
                    // Produce one substring for each different word break style run
                    if self.previous_word_break_style != current_word_break_style {
                        let (_, prev_size) = self.text.char_at(prev_char_index).unwrap();
                        let (_, size) = self.text.char_at(style_start_index).unwrap();

                        let substring = self.text.subrange(
                            self.building_range_start..style_start_index + size
                        );
                        let result_style = self.previous_word_break_style;

                        self.building_range_start = style_start_index - prev_size;
                        self.previous_word_break_style = current_word_break_style;

                        return Some((substring, result_style, false));
                    }

                    self.previous_word_break_style = current_word_break_style;
                }

                // Final segment
                self.done = true;
                let last_substring = if self.building_range_start == 0 {
                    self.text
                } else {
                    self.text.subrange(self.building_range_start..self.text.len())
                };
                Some((last_substring, self.previous_word_break_style, true))
            }
        }

        let text = if text.is_empty() { " " } else { text };

        let mut all_boundaries_byte_indexed = vec![Boundary::None; text.len()];

        // Word boundaries:
        for wb in self.analysis_data_sources.word_segmenter.segment_str(text) {
            // icu produces a word boundary trailing the string, which we don't use.
            if wb == text.len() {
                continue;
            }
            all_boundaries_byte_indexed[wb] = Boundary::Word;
        }

        // Line boundaries (word break naming refers to the line boundary determination config).
        //
        // This breaks text into sequences with similar line boundary config (part of style
        // information). If this config is consistent for all text, we use a fast path through this.
        let Some((first_style, rest)) = self.styles.split_first() else {
            panic!("No style info");
        };
        let contiguous_word_break_substrings = WordBreakSegmentIter::new(
            text,
            rest.iter(),
            &first_style
        );
        let mut global_offset = 0;
        for (substring_index, (substring, word_break_strength, last)) in contiguous_word_break_substrings.enumerate() {
            // TODO(conor) Should we expose CSS line-breaking strictness as an option in Parley's style API?
            //line_break_opts.strictness = LineBreakStrictness::Strict;
            // TODO(conor) - Do we set this, to have it impact breaking? It doesn't look like Swash is
            //  It seems like we'd want to - this could enable script-based line breaking.
            //line_break_opts.content_locale = ?

            let word_break_strength = swash_to_icu_lb(word_break_strength);
            let line_segmenter = &mut self.analysis_data_sources.line_segmenters.entry(word_break_strength as u8).or_insert({
                let mut line_break_opts: LineBreakOptions<'static> = Default::default();
                line_break_opts.word_option = Some(word_break_strength);
                LineSegmenter::new_auto(line_break_opts)
            });
            let line_boundaries: Vec<usize> = line_segmenter.segment_str(substring).collect();

            // Fast path for text with a single word-break option.
            if substring_index == 0 && last {
                // icu adds leading and trailing line boundaries, which we don't use.
                let Some((_first, rest)) = line_boundaries.split_first() else {
                    continue;
                };
                let Some((_last, middle)) = rest.split_last() else {
                    continue;
                };
                for &b in middle {
                    all_boundaries_byte_indexed[b] = Boundary::Line;
                }
                break;
            }

            let mut substring_chars = substring.chars();
            if substring_index != 0 {
                global_offset -= substring_chars.next().unwrap().len_utf8();
            }
            // There will always be at least two characters if we are not taking the fast path for
            // a single word break style substring.
            let last_len = substring_chars.next_back().unwrap().len_utf8();

            // Mark line boundaries (overriding word boundaries where present).
            for (index, &pos) in line_boundaries.iter().enumerate() {
                // icu adds leading and trailing line boundaries, which we don't use.
                if index == 0 || index == line_boundaries.len() - 1 {
                    continue;
                }

                // For all but the last substring, we ignore line boundaries caused by the last
                // character, as this character is carried back from the next substring, and will be
                // accounted for there.
                if !last && pos == substring.len() - last_len {
                    continue;
                }
                all_boundaries_byte_indexed[pos + global_offset] = Boundary::Line;
            }

            if !last {
                global_offset += substring.len() - last_len;
            }
        }

        // BiDi embedding levels:
        let bidi_embedding_levels = unicode_bidi::BidiInfo::new_with_data_source(&self.analysis_data_sources.bidi_class, text, None).levels;

        let boundaries_and_levels_iter = text.char_indices()
            .map(|(byte_pos, _)| (
                all_boundaries_byte_indexed.get(byte_pos).unwrap(),
                bidi_embedding_levels.get(byte_pos).unwrap()
            ));

        // Grapheme cluster boundaries:
        /*let segmenter = GraphemeClusterSegmenter::new();
        //segmenter.segment_str()
        let mut clusters = segmenter.segment_str(text);

        // Print the actual cluster boundaries
        let boundaries: Vec<usize> = clusters.collect();
        println!("cluster boundaries: {:?}", boundaries);

        // Or to get the actual text segments:
        let segmenter = GraphemeClusterSegmenter::new();
        let clusters = segmenter.segment_str(text);
        let mut last = 0;
        for boundary in clusters {
            println!("cluster: {:?}", &text[last..boundary]);
            last = boundary;
        }*/

        fn unicode_data_iterator<'a, T: TrieValue>(text: &'a str, data_source: CodePointMapDataBorrowed::<'static, T>) -> impl Iterator<Item = T> + 'a {
            text.chars().map(move |c| (c, data_source.get32(c as u32)).1)
        }

        boundaries_and_levels_iter
            .zip(text.chars())
            .zip(unicode_data_iterator(text, self.analysis_data_sources.script))
            .zip(unicode_data_iterator(text, self.analysis_data_sources.general_category))
            .zip(unicode_data_iterator(text, self.analysis_data_sources.grapheme_cluster_break))
            // Shift line break data forward one, as line boundaries corresponding with line-breaking
            // characters (like '\n') exist at an index position one higher than the respective
            // character's index, but we need our iterators to align, and the rest are simply
            // character-indexed.
            // TODO(conor) have data iterator not resolve value unless its needed (line break data not always used)
            .zip(std::iter::once(LineBreak::from_icu4c_value(0)).chain(unicode_data_iterator(text, self.analysis_data_sources.line_break)))
            .for_each(|((((((boundary, embed_level), ch), script), general_category), grapheme_cluster_break), line_break)| {
                let embed_level: BidiLevel = (*embed_level).into();
                let boundary = if is_mandatory_line_break(line_break) {
                    Boundary::Mandatory
                } else {
                    *boundary
                };
                let is_control = matches!(general_category, GeneralCategory::Control);
                let contributes_to_shaping = !is_control || (matches!(general_category, GeneralCategory::Format) &&
                    !matches!(script, Script::Inherited));

                let force_normalize = {
                    // "Extend" break chars should be normalized first, with two exceptions
                    if matches!(grapheme_cluster_break, GraphemeClusterBreak::Extend) &&
                        ch as u32 != 0x200C && // Is not a Zero Width Non-Joiner &&
                        !self.analysis_data_sources.variation_selector.contains(ch)
                    {
                        true
                    } else {
                        // All spacing mark break chars should be normalized first.
                        matches!(grapheme_cluster_break, GraphemeClusterBreak::SpacingMark)
                    }
                };

                self.info_icu.push((
                    icu_working::CharInfo::new(ch, boundary, embed_level, script, grapheme_cluster_break, is_control, contributes_to_shaping, force_normalize),
                    0 // Style index is populated later
                ));
            });
    }

    pub(crate) fn analyze_text(&mut self, text: &str) {
        let text = if text.is_empty() { " " } else { text };
        let mut a = swash::text::analyze(text.chars());
        _ = self.analyze_text_icu(text);

        let mut word_break = Default::default();
        let mut style_idx = 0;

        let mut char_indices = text.char_indices();
        loop {
            let Some((char_idx, _)) = char_indices.next() else {
                break;
            };

            // Find the style for this character. If the text is empty, we may not have any styles. Otherwise,
            // self.styles should span the entire range of the text.
            while let Some(style) = self.styles.get(style_idx) {
                if style.range.end > char_idx {
                    word_break = style.style.word_break;
                    break;
                }
                style_idx += 1;
            }
            a.set_break_strength(word_break);

            let Some((properties, boundary)) = a.next() else {
                break;
            };

            self.info.push((CharInfo::new(properties, boundary), 0));
        }

        // TODO(conor) - add back later, this is just to bring swash/icu test data to parity
        //if a.needs_bidi_resolution() {
            self.bidi.resolve(
                text.chars()
                    .zip(self.info.iter().map(|info| info.0.bidi_class())),
                None,
            );
            println!("{:?}", self.bidi.levels());
        //}
    }

    fn begin(&mut self) {
        self.rcx.clear();
        self.styles.clear();
        self.inline_boxes.clear();
        self.info.clear();
        self.info_icu.clear();
        self.bidi.clear();
    }
}

impl<B: Brush> Default for LayoutContext<B> {
    fn default() -> Self {
        Self::new()
    }
}

impl<B: Brush> Clone for LayoutContext<B> {
    fn clone(&self) -> Self {
        // None of the internal state is visible so just return a new instance.
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use fontique::FontWeight;
    use swash::text::WordBreakStrength;
    use crate::{FontContext, FontStack, LayoutContext, LineHeight, RangedBuilder, StyleProperty};

    // TODO(conor) - Rework/rename once Swash is fully removed
    fn verify_swash_icu_equivalence(text: &str, configure_builder: impl for<'a> FnOnce(&mut RangedBuilder<'a, [u8; 4]>))
    {
        #[derive(Default)]
        struct TestContext {
            pub layout_context: LayoutContext,
            pub font_context: FontContext,
        }

        let mut test_context = TestContext::default();

        {
            test_context.layout_context.begin();
            let mut builder = test_context.layout_context.ranged_builder(
                &mut test_context.font_context,
                text,
                1.,
                true
            );

            // TODO(conor) Remove this if really not needed
            let font_stack = FontStack::from("system-ui");
            builder.push_default(StyleProperty::Brush(Default::default()));
            builder.push_default(font_stack);
            builder.push_default(StyleProperty::FontSize(24.0));
            builder.push_default(LineHeight::FontSizeRelative(1.3));

            // Apply test-specific configuration
            configure_builder(&mut builder);

            _ = builder.build(&text);
        }

        // Now we can create iterators
        let swash_iter = test_context.layout_context.info.iter_mut();
        let bidi_iter = test_context.layout_context.bidi.levels();
        let icu_iter = test_context.layout_context.info_icu.iter();

        // Zip all three iterators
        for (idx, (((swash_info, _glyph_data), bidi_level), (icu_info, _icu_glyph_data))) in
            swash_iter.zip(bidi_iter).zip(icu_iter).enumerate() {

            // Print comparison for debugging
            println!(
                "[Char {}] SWASH vs ICU4X - boundary: {:?} vs {:?}, bidi: {:?} vs {:?}, script: {:?} vs {:?}",
                idx,
                swash_info.boundary(),
                icu_info.boundary,
                bidi_level,  // SWASH bidi level
                icu_info.bidi_embed_level,  // ICU4X bidi level
                swash_info.script(),
                crate::swash_convert::script_icu_to_swash(icu_info.script), // TODO(conor)
            );

            // Assert equality
            assert_eq!(
                swash_info.boundary(),
                icu_info.boundary,
                "Boundary mismatch at character position {} in text: '{}'",
                idx, text
            );
            assert_eq!(
                *bidi_level,
                icu_info.bidi_embed_level,
                "Bidi level mismatch at character position {} in text: '{}'",
                idx, text
            );
            assert_eq!(
                swash_info.script(),
                crate::swash_convert::script_icu_to_swash(icu_info.script), // TODO(conor)
                "Script mismatch at character position {} in text: '{}'",
                idx, text
            );
        }
    }

    // ==================== Basic Tests ====================

    #[test]
    fn test_blank() {
        verify_swash_icu_equivalence("", |_| {});
    }

    #[test]
    fn test_all_whitespace() {
        verify_swash_icu_equivalence("   ", |_| {});
    }

    #[test]
    fn test_single_char() {
        verify_swash_icu_equivalence("A", |_| {});
    }

    #[test]
    fn test_two_chars_keep_all() {
        verify_swash_icu_equivalence("AB", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::KeepAll), 0..2);
        });
    }

    #[test]
    fn test_three_chars() {
        verify_swash_icu_equivalence("ABC", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 0..3);
        });
    }

    #[test]
    fn test_single_char_multi_byte() {
        verify_swash_icu_equivalence("€", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::KeepAll), 0..3);
        });
    }

    #[test]
    fn test_multi_char_grapheme() {
        verify_swash_icu_equivalence("A e\u{0301} B", |_| {});
    }

    #[test]
    fn test_whitespace_contiguous_interspersed_in_latin() {
        verify_swash_icu_equivalence("A  B  C D", |_| {});
    }

    // ==================== Mixed Style Tests ====================

    #[test]
    fn test_mixed_style() {
        verify_swash_icu_equivalence("A  B  C D", |builder| {
            builder.push(StyleProperty::FontWeight(FontWeight::new(400.0)), 0..3);
            builder.push(StyleProperty::FontWeight(FontWeight::new(700.0)), 3..9);
        });
    }

    // ==================== Mixed Break Strength Tests ====================

    #[test]
    fn test_whitespace_contiguous_interspersed_in_latin_mixed() {
        // Drops expected line boundary on char 3 (just before B)
        verify_swash_icu_equivalence("A  B  C D", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::KeepAll), 0..3);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 3..9);
        });
    }

    #[test]
    fn test_latin_mixed_break_all_first() {
        verify_swash_icu_equivalence("AB", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 0..1);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 1..2);
        });
    }

    #[test]
    fn test_latin_mixed_break_all_last() {
        verify_swash_icu_equivalence("AB", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 0..1);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 1..2);
        });
    }

    #[test]
    fn test_latin_mixed_keep_all_first() {
        verify_swash_icu_equivalence("AB", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::KeepAll), 0..1);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 1..2);
        });
    }

    #[test]
    fn test_latin_mixed_keep_all_last() {
        verify_swash_icu_equivalence("AB", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 0..1);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::KeepAll), 1..2);
        });
    }

    #[test]
    fn test_latin_trailing_space_mixed() {
        verify_swash_icu_equivalence("AB ", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 0..1);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 1..3);
        });
    }

    #[test]
    fn test_latin_leading_space_mixed() {
        verify_swash_icu_equivalence(" AB", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 0..1);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 1..3);
        });
    }

    #[test]
    fn test_alternate_twice_within_word_normal_break_normal() {
        verify_swash_icu_equivalence("ABC", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 0..1);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 1..2);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 2..3);
        });
    }

    #[test]
    fn test_alternate_twice_within_word_break_normal_break() {
        verify_swash_icu_equivalence("ABC", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 0..1);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 1..2);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 2..3);
        });
    }

    #[test]
    fn test_mixed_break_simple() {
        verify_swash_icu_equivalence("ABCD 123", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 0..1);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::KeepAll), 1..8);
        });
    }

    #[test]
    fn test_mixed_break_four_segments() {
        verify_swash_icu_equivalence("ABCD 123", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 0..1);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::KeepAll), 1..2);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 2..4);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::KeepAll), 4..8);
        });
    }

    #[test]
    fn test_mixed_break_frequent_alternation() {
        verify_swash_icu_equivalence("ABCD 123", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 0..1);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::KeepAll), 1..2);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 2..3);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 3..4);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::KeepAll), 4..5);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 5..6);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 6..7);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::KeepAll), 7..8);
        });
    }

    #[test]
    fn test_multi_char_grapheme_mixed_break_all() {
        verify_swash_icu_equivalence("A e\u{0301} B", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 0..1);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 1..2);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 2..5);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 5..6);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 6..7);
        });
    }

    #[test]
    fn test_multi_char_grapheme_mixed_keep_all() {
        verify_swash_icu_equivalence("A e\u{0301} B", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::KeepAll), 0..1);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 1..2);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::KeepAll), 2..5);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 5..6);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::KeepAll), 6..7);
        });
    }

    #[test]
    fn test_multi_char_grapheme_mixed_break_and_keep_all() {
        verify_swash_icu_equivalence("A e\u{0301} B", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::KeepAll), 0..1);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 1..2);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::KeepAll), 2..5);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 5..6);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::KeepAll), 6..7);
        });
    }

    #[test]
    fn test_multi_byte_chars_alternating_break_all() {
        verify_swash_icu_equivalence("€你€你AA", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 0..3);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 3..6);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 6..9);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 9..12);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 12..13);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 13..14);
        });
    }

    #[test]
    fn test_multi_byte_chars_alternating_keep_all() {
        verify_swash_icu_equivalence("€你€你AA", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::KeepAll), 0..3);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 3..6);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::KeepAll), 6..9);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 9..12);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::KeepAll), 12..13);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 13..14);
        });
    }

    #[test]
    fn test_multi_byte_chars_varying_utf8_lengths() {
        // 2-3-4-3-2 byte pattern
        verify_swash_icu_equivalence("ß€𝓗你ą", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 0..2);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 2..5);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 5..9);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 9..12);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 12..14);
        });
    }

    #[test]
    fn test_multi_byte_chars_varying_utf8_lengths_whitespace_separated() {
        verify_swash_icu_equivalence("ß € 𝓗 你 ą", |builder| {
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 0..3);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 3..7);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 7..12);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::Normal), 12..16);
            builder.push(StyleProperty::WordBreak(WordBreakStrength::BreakAll), 16..19);
        });
    }

    // ==================== Newline Tests ====================

    #[test]
    fn test_newline() {
        verify_swash_icu_equivalence("\n", |_| {});
    }

    #[test]
    fn test_two_newlines() {
        verify_swash_icu_equivalence("\n\n", |_| {});
    }

    #[test]
    fn test_mandatory_break_in_text() {
        verify_swash_icu_equivalence("ABC DEF\nG", |_| {});
    }

    #[test]
    fn test_rtl_paragraph_with_non_authoritative_logical_first_character() {
        verify_swash_icu_equivalence("حداً ", |_| {});
    }
    #[test]
    fn test_rtl_paragraph_with_non_authoritative_logical_first_char_two_paragraphs() {
        verify_swash_icu_equivalence("حداً \nحداً ", |_| {});
    }

    #[test]
    fn test_multi_paragraph_bidi() {
        verify_swash_icu_equivalence("Hello مرحبا \nTest اختبار", |_| {});
    }

    // ==================== RTL and Bidirectional Tests ====================

    #[test]
    fn test_mixed_ltr_rtl() {
        verify_swash_icu_equivalence("Hello مرحبا", |_| {});
    }

    #[test]
    fn test_mixed_ltr_rtl_multiple_segments() {
        verify_swash_icu_equivalence("Hello مرحبا World عالم Test اختبار", |_| {});
    }

    #[test]
    fn test_mixed_ltr_rtl_nested_embedding() {
        verify_swash_icu_equivalence("In Hebrew: שנת 2024 היא...", |_| {});
    }
}