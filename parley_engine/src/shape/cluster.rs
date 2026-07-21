// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![expect(missing_docs, reason = "Deferred")]
#![expect(missing_debug_implementations, reason = "Deferred")]

use alloc::vec::Vec;
use icu_normalizer::properties::Decomposed;

use crate::{
    CharInfo,
    analysis::AnalysisDataSources,
    emoji::{EmojiDFA, EmojiPresentationStyle, EmojiSegmentationCategory},
};

#[derive(Debug, Default)]
pub struct CharCluster {
    chars: Vec<Char>,
    map_len: u8,
    start: u32,
    end: u32,
    force_normalize: bool,
    comp: Form,
    decomp: Form,
    best_ratio: f32,
    emoji_presentation_style: EmojiPresentationStyle,
}

impl CharCluster {
    #[inline]
    pub fn range(&self) -> SourceRange {
        SourceRange {
            start: self.start,
            end: self.end,
        }
    }

    #[inline(always)]
    pub fn chars(&self) -> &[Char] {
        &self.chars
    }

    #[inline(always)]
    pub fn is_emoji(&self) -> bool {
        self.emoji_presentation_style.is_emoji()
    }
}

/// Source range of a cluster in code units.
#[derive(Copy, Clone)]
pub struct SourceRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct Char {
    /// The character.
    pub ch: char,
    /// Whether the character
    pub is_control_character: bool,
    /// True if the character should be considered when mapping glyphs.
    pub contributes_to_shaping: bool,
    /// Indexes into the list of styles for the containing text run, to find the style applicable
    /// to this character.
    pub style_index: u16,
    /// True if the character is an emoji presentation selector (VS15 or VS16).
    pub is_emoji_presentation_selector: bool,
}

/// Whitespace content of a cluster.
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Whitespace {
    /// Not a space.
    None = 0,
    /// Standard space.
    Space = 1,
    /// Non-breaking space (U+00A0).
    NoBreakSpace = 2,
    /// Horizontal tab.
    Tab = 3,
    /// Newline (CR, LF, CRLF, LS, or PS).
    Newline = 4,
}

impl Whitespace {
    /// Returns true for space or no break space.
    #[inline]
    pub fn is_space_or_nbsp(self) -> bool {
        matches!(self, Self::Space | Self::NoBreakSpace)
    }
}

/// Iterative status of finding a font with the greatest coverage of a character cluster.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Status {
    /// Font covers less than previously considered fonts.
    Discard,
    /// The best font so far that isn't `Status::Complete`.
    Keep,
    /// Font with complete coverage.
    Complete,
}

impl CharCluster {
    #[inline]
    pub(crate) fn clear(&mut self) {
        self.chars.clear();
        self.map_len = 0;
        self.start = 0;
        self.end = 0;
        self.force_normalize = false;
        self.comp.clear();
        self.decomp.clear();
        self.best_ratio = 0.;
        self.emoji_presentation_style = EmojiPresentationStyle::Default;
    }

    #[inline(always)]
    fn len(&self) -> usize {
        self.chars.len()
    }

    /// Returns the primary style index for the cluster.
    #[inline(always)]
    pub fn style_index(&self) -> u16 {
        self.chars[0].style_index
    }

    #[inline(always)]
    fn contributes_to_shaping(ch: char, analysis_data_sources: &AnalysisDataSources) -> bool {
        let props = analysis_data_sources.properties(ch);
        crate::analysis::contributes_to_shaping(props.general_category(), props.script())
    }

    fn decomposed(&mut self, analysis_data_sources: &AnalysisDataSources) -> Option<&[Char]> {
        match self.decomp.state {
            FormState::Invalid => None,
            FormState::None => {
                self.decomp.state = FormState::Invalid;

                // Only attempt pairwise normalization (1 <-> 2 characters)
                if self.chars.len() != 1 {
                    return None;
                }

                let decomposer = analysis_data_sources.decomposing_normalizer();
                let decomp = decomposer.decompose(self.chars[0].ch);
                match decomp {
                    Decomposed::Default | Decomposed::Singleton(_) => {
                        return None;
                    }
                    Decomposed::Expansion(a, b) => {
                        let mut copy = self.chars[0];
                        copy.ch = a;
                        copy.contributes_to_shaping =
                            Self::contributes_to_shaping(a, analysis_data_sources);
                        self.decomp.chars[0] = copy;

                        copy.ch = b;
                        copy.contributes_to_shaping =
                            Self::contributes_to_shaping(b, analysis_data_sources);
                        self.decomp.chars[1] = copy;

                        self.decomp.len = 2;
                    }
                }

                self.decomp.state = FormState::Valid;
                self.decomp.setup();
                Some(self.decomp.chars())
            }
            FormState::Valid => Some(self.decomp.chars()),
        }
    }

    fn composed(&mut self, analysis_data_sources: &AnalysisDataSources) -> Option<&[Char]> {
        match self.comp.state {
            FormState::Invalid => None,
            FormState::None => {
                self.comp.state = FormState::Invalid;

                // Only attempt pairwise normalization (1 <-> 2 characters)
                if self.chars.len() != 2 {
                    return None;
                }

                let composer = analysis_data_sources.composing_normalizer();
                let ch = composer.compose(self.chars[0].ch, self.chars[1].ch)?;

                let mut copy = self.chars[0];
                copy.ch = ch;
                copy.contributes_to_shaping =
                    Self::contributes_to_shaping(ch, analysis_data_sources);
                self.comp.chars[0] = copy;
                self.comp.len = 1;

                self.comp.state = FormState::Valid;
                self.comp.setup();
                Some(self.comp.chars())
            }
            FormState::Valid => Some(self.comp.chars()),
        }
    }

    /// Decide whether a candidate font is complete, should be kept, or should be discarded.
    ///
    /// The callback `covers` should return whether the character is covered by the font under
    /// consideration. If the font covers the full character cluster or the characters of its
    /// normalized forms (see <https://www.unicode.org/reports/tr15/>), it's considered complete.
    /// Otherwise, the font is kept if its coverage is greater than fonts considered previously,
    /// else it's discarded.
    pub fn map(
        &mut self,
        covers: impl Fn(char) -> bool,
        analysis_data_sources: &AnalysisDataSources,
    ) -> Status {
        let len = self.len();
        if len == 0 {
            return Status::Complete;
        }
        let prev_ratio = self.best_ratio;
        let mut ratio;
        if self.force_normalize && self.composed(analysis_data_sources).is_some() {
            ratio = self.comp.coverage(&covers);
            if ratio > self.best_ratio {
                self.best_ratio = ratio;
                if ratio >= 1. {
                    return Status::Complete;
                }
            }
        }
        ratio = Mapper {
            chars: &mut self.chars[..len],
            map_len: self.map_len.max(1),
            has_contributing: self.map_len > 0,
        }
        .coverage(&covers);
        if ratio > self.best_ratio {
            self.best_ratio = ratio;
            if ratio >= 1. {
                return Status::Complete;
            }
        }
        if self.decomposed(analysis_data_sources).is_some() {
            ratio = self.decomp.coverage(&covers);
            if ratio > self.best_ratio {
                self.best_ratio = ratio;
                if ratio >= 1. {
                    return Status::Complete;
                }
            }
            if !self.force_normalize && self.composed(analysis_data_sources).is_some() {
                ratio = self.comp.coverage(&covers);
                if ratio > self.best_ratio {
                    self.best_ratio = ratio;
                    if ratio >= 1. {
                        return Status::Complete;
                    }
                }
            }
        }
        if self.best_ratio > prev_ratio {
            Status::Keep
        } else {
            Status::Discard
        }
    }

    /// Rebuilds `self` in-place using the existing allocation for the given grapheme
    /// `segment_text` and consuming items from `item_infos_iter`.
    ///
    /// The iterator must yield one item for each character in `segment_text`.
    ///
    /// `code_unit_offset_in_string` must be the byte offset of the start of `segment_text` in the
    /// source string. When this method returns, its value is the byte offset just past the end of
    /// `segment_text` in the source string.
    #[expect(clippy::cast_possible_truncation, reason = "Deferred")]
    #[inline]
    pub(crate) fn fill(
        &mut self,
        segment_text: &str,
        item_infos_iter: &mut impl Iterator<Item = (CharInfo, u16)>,
        code_unit_offset_in_string: &mut usize,
    ) {
        // Reset cluster but keep allocation
        self.clear();

        let mut force_normalize = false;
        let mut is_emoji = false;
        let mut map_len: u8 = 0;
        let mut emoji_dfa = EmojiDFA::new();
        let start = *code_unit_offset_in_string as u32;

        for ((_, ch), (info, style_index)) in
            segment_text.char_indices().zip(item_infos_iter.by_ref())
        {
            *code_unit_offset_in_string += ch.len_utf8();
            force_normalize |= info.force_normalize();

            // TODO - make emoji detection more complete, as per (except using composite Trie tables as
            //  much as possible:
            //  https://github.com/conor-93/parley/blob/4637d826732a1a82bbb3c904c7f47a16a21cceec/parley/src/shape/mod.rs#L221-L269
            is_emoji |= info.is_emoji_or_pictograph();

            let mut is_emoji_presentation_selector = false;

            if is_emoji {
                let category =
                    EmojiSegmentationCategory::from_codepoint(ch as u32, info.emoji_properties);

                is_emoji_presentation_selector = category.is_presentation_selector();

                emoji_dfa.step_record(category);
            }

            let contributes_to_shaping = info.contributes_to_shaping();
            if contributes_to_shaping {
                map_len += 1;
            }

            self.chars.push(Char {
                ch,
                contributes_to_shaping,
                style_index,
                is_control_character: info.is_control(),
                is_emoji_presentation_selector,
            });
        }

        // Finalize cluster metadata
        let end = *code_unit_offset_in_string as u32;
        self.map_len = map_len;
        self.start = start;
        self.end = end;
        self.force_normalize = force_normalize;

        if is_emoji {
            self.emoji_presentation_style = emoji_dfa.presentation_style();
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum FormState {
    None,
    Valid,
    Invalid,
}

#[derive(Clone, Debug)]
pub(crate) struct Form {
    chars: [Char; 2],
    len: u8,
    map_len: u8,
    has_contributing: bool,
    state: FormState,
}

impl Default for Form {
    fn default() -> Self {
        Self::new()
    }
}

impl Form {
    fn new() -> Self {
        Self {
            chars: [Char::default(), Char::default()],
            len: 0,
            map_len: 0,
            has_contributing: false,
            state: FormState::None,
        }
    }

    fn clear(&mut self) {
        self.chars = [Char::default(), Char::default()];
        self.len = 0;
        self.map_len = 0;
        self.has_contributing = false;
        self.state = FormState::None;
    }

    #[inline(always)]
    fn chars(&self) -> &[Char] {
        &self.chars[..self.len as usize]
    }

    #[inline(always)]
    #[expect(clippy::cast_possible_truncation, reason = "Deferred")]
    fn setup(&mut self) {
        self.map_len = (self
            .chars()
            .iter()
            .filter(|c| !c.is_control_character)
            .count() as u8)
            .max(1);
        self.has_contributing = self.chars().iter().any(|c| c.contributes_to_shaping);
    }

    #[inline(always)]
    fn coverage(&self, covers: &impl Fn(char) -> bool) -> f32 {
        Mapper {
            chars: &self.chars[..self.len as usize],
            map_len: self.map_len,
            has_contributing: self.has_contributing,
        }
        .coverage(covers)
    }
}

struct Mapper<'a> {
    chars: &'a [Char],
    map_len: u8,
    has_contributing: bool,
}

impl<'a> Mapper<'a> {
    /// Returns the ratio of characters contributing to shaping that are covered.
    fn coverage(&self, covers: &impl Fn(char) -> bool) -> f32 {
        if self.map_len == 0 {
            return 1.;
        }
        let mut mapped = 0;
        for c in self.chars.iter() {
            // If the color emoji has a presentation style, ignore the variation selector.
            if c.is_emoji_presentation_selector {
                mapped += 1;
                continue;
            }

            if !c.contributes_to_shaping {
                if !self.has_contributing {
                    mapped += 1;
                }
            } else {
                if covers(c.ch) {
                    mapped += 1;
                }
            }

            if self.map_len == 1 {
                mapped += 1;
            }
        }
        mapped as f32 / self.map_len as f32
    }
}
