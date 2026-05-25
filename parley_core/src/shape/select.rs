// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Per-grapheme font selection.
//!
//! These are graphemes as in UAX #29's "extended grapheme clusters", i.e., multiple characters can
//! be a single unit. For example, "é" written as "e" + U+0301 is a grapheme.
//!
//! For each grapheme we ask the [`fontique::Query`] for candidate fonts and pick the one that
//! covers the cluster, keeping the best partial match as a fallback. Coverage is probed against
//! the font's `cmap`, trying canonical NFC/NFD forms of combining sequences so a font carrying
//! only the precomposed (or only the decomposed) form still counts as covering. The chosen *font*
//! is what matters here; actual glyph selection happens later when `harfrust` shapes the font run.

use alloc::vec::Vec;

use fontique::{Charmap, QueryFont};
use icu_normalizer::properties::Decomposed;

use crate::analysis::{AnalysisDataSources, CharInfo};

/// Nominal glyph identifier (0 is `.notdef` / unmapped).
type GlyphId = u32;

/// One character of a [`CharCluster`].
#[derive(Copy, Clone, Debug, Default)]
struct Char {
    /// The character.
    ch: char,
    /// Whether the character is a control character.
    is_control_character: bool,
    /// True if the character should be considered when mapping glyphs.
    contributes_to_shaping: bool,
}

/// A grapheme cluster being probed for font coverage.
#[derive(Debug, Default)]
pub(super) struct CharCluster {
    chars: Vec<Char>,
    map_len: u8,
    force_normalize: bool,
    comp: Form,
    decomp: Form,
    best_ratio: f32,
}

/// Iterative status of mapping a character cluster to nominal glyph identifiers.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Status {
    /// Mapping should be skipped.
    Discard,
    /// The best mapping so far.
    Keep,
    /// Complete mapping.
    Complete,
}

impl CharCluster {
    /// Resets the cluster, then fills it from `grapheme_text` and `run_char_infos`, advancing
    /// `char_cursor` by the number of characters in `grapheme_text`.
    pub(super) fn fill(
        &mut self,
        grapheme_text: &str,
        char_cursor: &mut usize,
        run_char_infos: &[CharInfo],
    ) {
        self.clear();
        for ch in grapheme_text.chars() {
            let info = run_char_infos[*char_cursor];
            *char_cursor += 1;
            self.force_normalize |= info.force_normalize();
            let contributes_to_shaping = info.contributes_to_shaping();
            if contributes_to_shaping {
                self.map_len += 1;
            }
            self.chars.push(Char {
                ch,
                is_control_character: info.is_control(),
                contributes_to_shaping,
            });
        }
    }

    fn clear(&mut self) {
        self.chars.clear();
        self.map_len = 0;
        self.force_normalize = false;
        self.comp.clear();
        self.decomp.clear();
        self.best_ratio = 0.;
    }

    #[inline(always)]
    fn len(&self) -> usize {
        self.chars.len()
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
                let comp = composer.compose(self.chars[0].ch, self.chars[1].ch);
                match comp {
                    None => {}
                    Some(ch) => {
                        let mut copy = self.chars[0];
                        copy.ch = ch;
                        copy.contributes_to_shaping =
                            Self::contributes_to_shaping(ch, analysis_data_sources);
                        self.comp.chars[0] = copy;
                        self.comp.len = 1;
                    }
                }

                self.comp.state = FormState::Valid;
                self.comp.setup();
                Some(self.comp.chars())
            }
            FormState::Valid => Some(self.comp.chars()),
        }
    }

    /// Probes a font's coverage, where `f(ch)` reports the font's nominal glyph for `ch` (0 for
    /// `.notdef`/unmapped). Updates the running best ratio and returns how this font compares.
    fn map(
        &mut self,
        f: impl Fn(char) -> GlyphId,
        analysis_data_sources: &AnalysisDataSources,
    ) -> Status {
        let len = self.len();
        if len == 0 {
            return Status::Complete;
        }
        let prev_ratio = self.best_ratio;
        let mut ratio;
        if self.force_normalize && self.composed(analysis_data_sources).is_some() {
            ratio = self.comp.map(&f);
            if ratio > self.best_ratio {
                self.best_ratio = ratio;
                if ratio >= 1. {
                    return Status::Complete;
                }
            }
        }
        ratio = Mapper {
            chars: &self.chars[..len],
            map_len: self.map_len.max(1),
        }
        .map(&f);
        if ratio > self.best_ratio {
            self.best_ratio = ratio;
            if ratio >= 1. {
                return Status::Complete;
            }
        }
        if self.decomposed(analysis_data_sources).is_some() {
            ratio = self.decomp.map(&f);
            if ratio > self.best_ratio {
                self.best_ratio = ratio;
                if ratio >= 1. {
                    return Status::Complete;
                }
            }
            if !self.force_normalize && self.composed(analysis_data_sources).is_some() {
                ratio = self.comp.map(&f);
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
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum FormState {
    None,
    Valid,
    Invalid,
}

#[derive(Clone, Debug)]
struct Form {
    chars: [Char; 2],
    len: u8,
    map_len: u8,
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
            state: FormState::None,
        }
    }

    fn clear(&mut self) {
        self.chars = [Char::default(), Char::default()];
        self.len = 0;
        self.map_len = 0;
        self.state = FormState::None;
    }

    #[inline(always)]
    fn chars(&self) -> &[Char] {
        &self.chars[..self.len as usize]
    }

    #[inline(always)]
    fn setup(&mut self) {
        self.map_len = (self
            .chars()
            .iter()
            .filter(|c| !c.is_control_character)
            .count() as u8)
            .max(1);
    }

    #[inline(always)]
    fn map(&self, f: &impl Fn(char) -> GlyphId) -> f32 {
        Mapper {
            chars: &self.chars[..self.len as usize],
            map_len: self.map_len,
        }
        .map(f)
    }
}

struct Mapper<'a> {
    chars: &'a [Char],
    map_len: u8,
}

impl Mapper<'_> {
    fn map(&self, f: &impl Fn(char) -> GlyphId) -> f32 {
        if self.map_len == 0 {
            return 1.;
        }
        let mut mapped = 0;
        for c in self.chars.iter() {
            if !c.contributes_to_shaping {
                if self.map_len == 1 {
                    mapped += 1;
                }
            } else if f(c.ch) != 0 {
                mapped += 1;
            }
        }
        mapped as f32 / self.map_len as f32
    }
}

/// Selects the font that best covers `cluster` from the query's candidates.
///
/// `charmaps` is a cache of each candidate's character map, filled on first probe.
///
/// Returns the first fully-covering font, or the best partial match, or the first candidate if
/// none cover anything (so an unmappable cluster still gets a font and renders as `.notdef`).
pub(super) fn select_font<'c>(
    candidates: &'c [QueryFont],
    charmaps: &mut [Option<Charmap<'c>>],
    cluster: &mut CharCluster,
    analysis_data_sources: &AnalysisDataSources,
) -> Option<usize> {
    let mut selected = None;
    for index in 0..candidates.len() {
        if charmaps[index].is_none() {
            charmaps[index] = candidates[index].charmap();
        }
        // A font without a usable `cmap` covers nothing.
        let Some(charmap) = &charmaps[index] else {
            continue;
        };
        match cluster.map(|ch| charmap.map(ch).unwrap_or(0), analysis_data_sources) {
            Status::Complete => return Some(index),
            Status::Keep => selected = Some(index),
            Status::Discard => {
                if selected.is_none() {
                    selected = Some(index);
                }
            }
        }
    }
    selected
}

/// Two [`QueryFont`]s shape identically when they come from the same family entry with the same
/// synthesis, so font runs need not be split between them.
pub(super) fn same_font(a: &QueryFont, b: &QueryFont) -> bool {
    a.family == b.family && a.synthesis == b.synthesis
}
