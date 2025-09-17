// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Context for layout.

use alloc::{vec, vec::Vec};
use std::collections::HashMap;
use icu::collections::codepointtrie::TrieValue;
use icu::properties::props::BidiClass;
use icu::segmenter::{LineSegmenter, LineSegmenterBorrowed, WordSegmenter, WordSegmenterBorrowed};
use icu::segmenter::options::{LineBreakOptions, LineBreakWordOption, WordBreakInvariantOptions};
use icu_properties::CodePointMapDataBorrowed;
use icu_properties::props::{LineBreak, Script};
use self::tree::TreeStyleBuilder;

use super::{icu_working, FontContext};
use super::bidi;
use super::builder::RangedBuilder;
use super::resolve::{RangedStyle, RangedStyleBuilder, ResolveContext, ResolvedStyle, tree};
use super::style::{Brush, TextStyle};

use swash::text::cluster::{Boundary, CharInfo};
use swash::text::WordBreakStrength;
use unicode_bidi::{Level, ParagraphInfo, TextSource};
use crate::bidi::BidiLevel;
use crate::builder::TreeBuilder;
use crate::inline_box::InlineBox;
use crate::shape::ShapeContext;

struct UnicodeDataSources {
    // TODO(conor) Review lifetime specifier
    script: CodePointMapDataBorrowed::<'static, Script>,
    bidi_class: CodePointMapDataBorrowed::<'static, BidiClass>,
    line_break: CodePointMapDataBorrowed::<'static, LineBreak>,
    word_segmenter: WordSegmenterBorrowed<'static>,
    // Key: LineBreakWordOption as u8
    line_segmenters: HashMap<u8, LineSegmenterBorrowed<'static>>,
}

impl UnicodeDataSources {
    fn new() -> Self {
        Self {
            script: CodePointMapDataBorrowed::<Script>::new(),
            bidi_class: CodePointMapDataBorrowed::<BidiClass>::new(),
            line_break: CodePointMapDataBorrowed::<LineBreak>::new(),
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
    pub(crate) info_icu: Vec<(icu_working::CharInfo, u16)>,
    pub(crate) scx: ShapeContext,

    // TODO(conor) revise name (*Segmenters are not as such), or decompose entirely?
    unicode_data_sources: UnicodeDataSources,
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
            unicode_data_sources: UnicodeDataSources::new(),
            info_icu: vec![],
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

    // TODO(conor) consistent idx vs index naming
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

        let text = if text.is_empty() { " " } else { text };

        let Some((first_style, rest)) = self.styles.split_first() else {
            panic!("No style info");
        };

        let mut building_range_start = first_style.range.start;
        let mut previous_word_break_style = first_style.style.word_break;
        let mut contiguous_word_break_substrings: Vec<(&str, WordBreakStrength)> = Vec::new();

        // TODO(conor) can we improve fast path, so that the subsequent loop is unnecessary?
        //  would it be better to check for single word break style config first (vast majority of cases)
        //  and skip this work if so?
        // icu doesn't support alternating the break configuration used in determining line
        // boundaries across a string, which we support in Parley. This segments a string where
        // line break options change, and looks forward/back one character in each, so that we have
        // all the context we need for boundary calculation, per segment.
        // TODO(conor) could this also be combined with the bidi/boundary iterator that consumes char_indices below?
        let mut char_indices = text.char_indices();
        let mut current_char = char_indices.next().unwrap();
        let mut prev_char;
        // TODO(conor) just produce iterator for `contiguous_word_break_substrings` and consume
        //  in later loop
        for style in rest {
            let style_start_index = style.range.start;
            // Loop until we know the first character of our span, and the previous character:
            // the last character of the previous span.
            loop {
                prev_char = current_char;
                current_char = char_indices.next().unwrap();
                if style_start_index == current_char.0 {
                    break;
                }
            }

            let (_, prev_size) = text.char_at(prev_char.0).unwrap();

            let current_word_break_style = style.style.word_break;
            if previous_word_break_style != current_word_break_style {
                let (_, size) = text.char_at(style_start_index).unwrap();
                contiguous_word_break_substrings.push((
                    // End one character late, to grab the first character from the next span,
                    // for all but the last span.
                    text.subrange(building_range_start..style_start_index + size),
                    previous_word_break_style
                ));
                // Start one character early, to get the last character from the previous span,
                // for all but the first span
                building_range_start = style_start_index - prev_size;
            }
            previous_word_break_style = current_word_break_style;
        }
        let last_substring = if building_range_start == 0 {
            // Don't allocate a new string if we aren't segmenting
            text
        } else {
            text.subrange(building_range_start..text.len())
        };
        contiguous_word_break_substrings.push((
            last_substring,
            previous_word_break_style,
        ));

        let mut all_boundaries_byte_indexed = vec![Boundary::None; text.len()];

        // Word boundaries:
        for wb in self.unicode_data_sources.word_segmenter.segment_str(text) {
            // icu will produce a word boundary trailing the string, which we don't use.
            if wb == text.len() {
                continue;
            }
            all_boundaries_byte_indexed[wb] = Boundary::Word;
        }

        // Line boundaries:
        let substring_count = contiguous_word_break_substrings.len();
        let mut global_offset = 0;
        for (substring_index, (substring, word_break_strength)) in contiguous_word_break_substrings.iter().enumerate() {
            // Boundaries
            // TODO(conor) Should we expose CSS line-breaking strictness as an option in Parley's style API?
            //line_break_opts.strictness = LineBreakStrictness::Strict;
            // TODO(conor) - Do we set this, to have it impact breaking? It doesn't look like Swash is
            //  It seems like we'd want to - this could enable script-based line breaking.
            //line_break_opts.content_locale = ?

            let word_break_strength = swash_to_icu_lb(*word_break_strength);
            let line_segmenter = &mut self.unicode_data_sources.line_segmenters.entry(word_break_strength as u8).or_insert({
                let mut line_break_opts: LineBreakOptions<'static> = Default::default();
                line_break_opts.word_option = Some(word_break_strength);
                LineSegmenter::new_auto(line_break_opts)
            });
            let line_boundaries: Vec<usize> = line_segmenter.segment_str(substring).collect();

            // Fast path for text with a single word-break option.
            if substring_count == 1 {
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
            for (lb_idx, &pos) in line_boundaries.iter().enumerate() {
                // icu adds leading and trailing line boundaries, which we don't use.
                if lb_idx == 0 || lb_idx == line_boundaries.len() - 1 {
                    continue;
                }

                // For all but the last substring, we ignore line boundaries caused by the last
                // character, as this character is carried back from the next substring, and will be
                // accounted for there.
                if substring_index != substring_count - 1 && pos == substring.len() - last_len {
                    continue;
                }
                all_boundaries_byte_indexed[pos + global_offset] = Boundary::Line;
            }

            if substring_index != substring_count - 1 {
                global_offset += substring.len() - last_len;
            }
        }

        // BiDi embedding levels:
        let bidi_info = unicode_bidi::BidiInfo::new_with_data_source(&self.unicode_data_sources.bidi_class, text, None);
        let full_text_range = 0..text.len();
        let paragraph = ParagraphInfo {
            range: full_text_range.clone(),
            level: Level::ltr(),
        };
        let bidi_embed_levels_byte_indexed = bidi_info.reordered_levels(&paragraph, full_text_range);

        let boundaries_and_levels_iter = text.char_indices()
            .map(|(byte_pos, _)| (
                all_boundaries_byte_indexed.get(byte_pos).unwrap(),
                bidi_embed_levels_byte_indexed.get(byte_pos).unwrap()
            ));

        fn unicode_data_iterator<'a, T: TrieValue>(text: &'a str, data_source: CodePointMapDataBorrowed::<'static, T>) -> impl Iterator<Item = T> + 'a {
            text.chars().map(move |c| (c, data_source.get32(c as u32)).1)
        }
        boundaries_and_levels_iter
            .zip(unicode_data_iterator(text, self.unicode_data_sources.script))
            // Shift line break data forward one, as line boundaries corresponding with line-breaking
            // characters (like '\n') exist at an index position one higher than the respective
            // character's index, but we need our iterators to align, and the rest are simply
            // character-indexed.
            // TODO(conor) have data iterator not resolve value unless its needed (line break data not always used)
            .zip(std::iter::once(LineBreak::from_icu4c_value(0)).chain(unicode_data_iterator(text, self.unicode_data_sources.line_break)))
            .for_each(|(((boundary, embed_level), script), line_break)| {
                let embed_level: BidiLevel = (*embed_level).into();
                let swash_script: swash::text::Script = script_from_u8(script.to_icu4c_value() as u8).unwrap();
                let boundary = if is_mandatory_line_break(line_break) {
                    Boundary::Mandatory
                } else {
                    *boundary
                };
                self.info_icu.push((
                    icu_working::CharInfo::new(boundary, embed_level, swash_script),
                    0
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

fn script_from_u8(value: u8) -> Option<swash::text::Script> {
    match value {
        0 => Some(swash::text::Script::Common),
        1 => Some(swash::text::Script::Inherited),
        2 => Some(swash::text::Script::Arabic),
        3 => Some(swash::text::Script::Armenian),
        4 => Some(swash::text::Script::Bengali),
        5 => Some(swash::text::Script::Bopomofo),
        6 => Some(swash::text::Script::Cherokee),
        7 => Some(swash::text::Script::Coptic),
        8 => Some(swash::text::Script::Cyrillic),
        9 => Some(swash::text::Script::Deseret),
        10 => Some(swash::text::Script::Devanagari),
        11 => Some(swash::text::Script::Ethiopic),
        12 => Some(swash::text::Script::Georgian),
        13 => Some(swash::text::Script::Gothic),
        14 => Some(swash::text::Script::Greek),
        15 => Some(swash::text::Script::Gujarati),
        16 => Some(swash::text::Script::Gurmukhi),
        17 => Some(swash::text::Script::Han),
        18 => Some(swash::text::Script::Hangul),
        19 => Some(swash::text::Script::Hebrew),
        20 => Some(swash::text::Script::Hiragana),
        21 => Some(swash::text::Script::Kannada),
        22 => Some(swash::text::Script::Katakana),
        23 => Some(swash::text::Script::Khmer),
        24 => Some(swash::text::Script::Lao),
        25 => Some(swash::text::Script::Latin),
        26 => Some(swash::text::Script::Malayalam),
        27 => Some(swash::text::Script::Mongolian),
        28 => Some(swash::text::Script::Myanmar),
        29 => Some(swash::text::Script::Ogham),
        30 => Some(swash::text::Script::OldItalic),
        31 => Some(swash::text::Script::Oriya),
        32 => Some(swash::text::Script::Runic),
        33 => Some(swash::text::Script::Sinhala),
        34 => Some(swash::text::Script::Syriac),
        35 => Some(swash::text::Script::Tamil),
        36 => Some(swash::text::Script::Telugu),
        37 => Some(swash::text::Script::Thaana),
        38 => Some(swash::text::Script::Thai),
        39 => Some(swash::text::Script::Tibetan),
        40 => Some(swash::text::Script::CanadianAboriginal),
        41 => Some(swash::text::Script::Yi),
        42 => Some(swash::text::Script::Tagalog),
        43 => Some(swash::text::Script::Hanunoo),
        44 => Some(swash::text::Script::Buhid),
        45 => Some(swash::text::Script::Tagbanwa),
        46 => Some(swash::text::Script::Braille),
        47 => Some(swash::text::Script::Cypriot),
        48 => Some(swash::text::Script::Limbu),
        49 => Some(swash::text::Script::LinearB),
        50 => Some(swash::text::Script::Osmanya),
        51 => Some(swash::text::Script::Shavian),
        52 => Some(swash::text::Script::TaiLe),
        53 => Some(swash::text::Script::Ugaritic),
        55 => Some(swash::text::Script::Buginese),
        56 => Some(swash::text::Script::Glagolitic),
        57 => Some(swash::text::Script::Kharoshthi),
        58 => Some(swash::text::Script::SylotiNagri),
        59 => Some(swash::text::Script::NewTaiLue),
        60 => Some(swash::text::Script::Tifinagh),
        61 => Some(swash::text::Script::OldPersian),
        62 => Some(swash::text::Script::Balinese),
        63 => Some(swash::text::Script::Batak),
        65 => Some(swash::text::Script::Brahmi),
        66 => Some(swash::text::Script::Cham),
        71 => Some(swash::text::Script::EgyptianHieroglyphs),
        75 => Some(swash::text::Script::PahawhHmong),
        76 => Some(swash::text::Script::OldHungarian),
        78 => Some(swash::text::Script::Javanese),
        79 => Some(swash::text::Script::KayahLi),
        82 => Some(swash::text::Script::Lepcha),
        83 => Some(swash::text::Script::LinearA),
        84 => Some(swash::text::Script::Mandaic),
        86 => Some(swash::text::Script::MeroiticHieroglyphs),
        87 => Some(swash::text::Script::Nko),
        88 => Some(swash::text::Script::OldTurkic),
        89 => Some(swash::text::Script::OldPermic),
        90 => Some(swash::text::Script::PhagsPa),
        91 => Some(swash::text::Script::Phoenician),
        92 => Some(swash::text::Script::Miao),
        99 => Some(swash::text::Script::Vai),
        101 => Some(swash::text::Script::Cuneiform),
        103 => Some(swash::text::Script::Unknown),
        104 => Some(swash::text::Script::Carian),
        106 => Some(swash::text::Script::TaiTham),
        107 => Some(swash::text::Script::Lycian),
        108 => Some(swash::text::Script::Lydian),
        109 => Some(swash::text::Script::OlChiki),
        110 => Some(swash::text::Script::Rejang),
        111 => Some(swash::text::Script::Saurashtra),
        112 => Some(swash::text::Script::SignWriting),
        113 => Some(swash::text::Script::Sundanese),
        115 => Some(swash::text::Script::MeeteiMayek),
        116 => Some(swash::text::Script::ImperialAramaic),
        117 => Some(swash::text::Script::Avestan),
        118 => Some(swash::text::Script::Chakma),
        120 => Some(swash::text::Script::Kaithi),
        121 => Some(swash::text::Script::Manichaean),
        122 => Some(swash::text::Script::InscriptionalPahlavi),
        123 => Some(swash::text::Script::PsalterPahlavi),
        125 => Some(swash::text::Script::InscriptionalParthian),
        126 => Some(swash::text::Script::Samaritan),
        127 => Some(swash::text::Script::TaiViet),
        130 => Some(swash::text::Script::Bamum),
        131 => Some(swash::text::Script::Lisu),
        133 => Some(swash::text::Script::OldSouthArabian),
        134 => Some(swash::text::Script::BassaVah),
        135 => Some(swash::text::Script::Duployan),
        136 => Some(swash::text::Script::Elbasan),
        137 => Some(swash::text::Script::Grantha),
        140 => Some(swash::text::Script::MendeKikakui),
        141 => Some(swash::text::Script::MeroiticCursive),
        142 => Some(swash::text::Script::OldNorthArabian),
        143 => Some(swash::text::Script::Nabataean),
        144 => Some(swash::text::Script::Palmyrene),
        145 => Some(swash::text::Script::Khudawadi),
        146 => Some(swash::text::Script::WarangCiti),
        149 => Some(swash::text::Script::Mro),
        150 => Some(swash::text::Script::Nushu),
        151 => Some(swash::text::Script::Sharada),
        152 => Some(swash::text::Script::SoraSompeng),
        153 => Some(swash::text::Script::Takri),
        154 => Some(swash::text::Script::Tangut),
        156 => Some(swash::text::Script::AnatolianHieroglyphs),
        157 => Some(swash::text::Script::Khojki),
        158 => Some(swash::text::Script::Tirhuta),
        159 => Some(swash::text::Script::CaucasianAlbanian),
        160 => Some(swash::text::Script::Mahajani),
        161 => Some(swash::text::Script::Ahom),
        162 => Some(swash::text::Script::Hatran),
        163 => Some(swash::text::Script::Modi),
        164 => Some(swash::text::Script::Multani),
        165 => Some(swash::text::Script::PauCinHau),
        166 => Some(swash::text::Script::Siddham),
        167 => Some(swash::text::Script::Adlam),
        168 => Some(swash::text::Script::Bhaiksuki),
        169 => Some(swash::text::Script::Marchen),
        170 => Some(swash::text::Script::Newa),
        171 => Some(swash::text::Script::Osage),
        175 => Some(swash::text::Script::MasaramGondi),
        176 => Some(swash::text::Script::Soyombo),
        177 => Some(swash::text::Script::ZanabazarSquare),
        178 => Some(swash::text::Script::Dogra),
        179 => Some(swash::text::Script::GunjalaGondi),
        180 => Some(swash::text::Script::Makasar),
        181 => Some(swash::text::Script::Medefaidrin),
        182 => Some(swash::text::Script::HanifiRohingya),
        183 => Some(swash::text::Script::Sogdian),
        184 => Some(swash::text::Script::OldSogdian),
        185 => Some(swash::text::Script::Elymaic),
        186 => Some(swash::text::Script::NyiakengPuachueHmong),
        187 => Some(swash::text::Script::Nandinagari),
        188 => Some(swash::text::Script::Wancho),
        189 => Some(swash::text::Script::Chorasmian),
        190 => Some(swash::text::Script::DivesAkuru),
        191 => Some(swash::text::Script::KhitanSmallScript),
        192 => Some(swash::text::Script::Yezidi),
        // 193 => Some(swash::text::Script::Cypro),
        // 194 => Some(swash::text::Script::OldUyghur),
        // 195 => Some(swash::text::Script::Tangsa),
        // 196 => Some(swash::text::Script::Toto),
        // 197 => Some(swash::text::Script::Vithkuqi),
        // 198 => Some(swash::text::Script::Kawi),
        // 199 => Some(swash::text::Script::NagMundari),
        // 200 => Some(swash::text::Script::Nastaliq),
        _ => None,
    }
}

mod tests {
    use fontique::FontWeight;
    use swash::text::WordBreakStrength;
    use crate::{FontContext, FontStack, LayoutContext, LineHeight, RangedBuilder, StyleProperty};

    #[derive(Default)]
    struct TestContext {
        pub layout_context: LayoutContext,
        pub font_context: FontContext,
    }

    // TODO(conor) - Rework/rename once Swash is fully removed
    fn verify_swash_icu_equivalence(text: &str, configure_builder: impl for<'a> FnOnce(&mut RangedBuilder<'a, [u8; 4]>))
    {
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
                icu_info.script
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
                icu_info.script,
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