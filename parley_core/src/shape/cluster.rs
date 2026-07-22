// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![expect(missing_docs, reason = "Deferred")]
#![expect(missing_debug_implementations, reason = "Deferred")]

use core::cmp::Ordering;

use alloc::vec::Vec;
use icu_normalizer::properties::Decomposed;

use crate::{CharInfo, analysis::AnalysisDataSources};

#[derive(Debug, Default)]
pub struct CharCluster {
    chars: Vec<Char>,
    is_emoji: bool,
    map_len: u8,
    start: u32,
    end: u32,
    force_normalize: bool,
    comp: Form,
    decomp: Form,
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
        self.is_emoji
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

/// A font's coverage of a character cluster.
///
/// This is the fraction `covered / total` of characters covered by a font. Coverages are ordered by
/// the value of the fraction (with `0 / 0` considered to be completely covered). Use, e.g.,
/// [`Self::cmp`] to compare coverages.
///
/// In the unlikely event there are more characters than this in the character cluster, the counts
/// are saturated to [`u8::MAX`].
#[derive(Copy, Clone, Eq, Debug)]
pub struct Coverage {
    /// The number of characters covered by the font.
    pub covered: u8,

    /// The number of characters counting towards coverage.
    pub total: u8,
}

impl Coverage {
    /// Zero coverage.
    ///
    /// Useful, for example, as a starting "best coverage," in case you're searching for a
    /// best-covering font.
    pub const NONE: Self = Self {
        covered: 0,
        total: 1,
    };

    /// Complete coverage, meaning all characters contributing to shaping are covered.
    ///
    /// A coverage compares equal to `COMPLETE` exactly when it is complete, see also
    /// [`Self::is_complete`].
    pub const COMPLETE: Self = Self {
        covered: 1,
        total: 1,
    };

    /// Returns `true` iff all characters are covered.
    #[inline(always)]
    #[must_use]
    pub fn is_complete(self) -> bool {
        self.covered >= self.total
    }
}

impl PartialEq for Coverage {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl PartialOrd for Coverage {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Coverage {
    /// Coverages are ordered by the value of the fraction `covered / total`.
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        // We check `is_complete` explicitly, such that fractions of `x / 0`, which are considered
        // complete, are ordered identically to other complete fractions. Cases other than those
        // with a denominator of 0 would have already been correctly ordered by value through the
        // cross-multiplication in the `(false, false)` arm, but note we now order all complete
        // fractions as `Ordering::Equal`.
        match (self.is_complete(), other.is_complete()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => (u16::from(self.covered) * u16::from(other.total))
                .cmp(&(u16::from(other.covered) * u16::from(self.total))),
        }
    }
}

impl CharCluster {
    #[inline]
    pub(crate) fn clear(&mut self) {
        self.chars.clear();
        self.is_emoji = false;
        self.map_len = 0;
        self.start = 0;
        self.end = 0;
        self.force_normalize = false;
        self.comp.clear();
        self.decomp.clear();
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

    /// Calculate a candidate font's coverage of this character cluster.
    ///
    /// The callback `covers` should return whether the character is covered by the font under
    /// consideration. The coverage returned is the best coverage of the character cluster's
    /// characters, or the characters of its normalized forms (see
    /// <https://www.unicode.org/reports/tr15/>).
    ///
    /// Coverages are ordered by how much of the cluster they cover, see [`Coverage`]. Note
    /// [`Coverage::total`] may be different from the count of [`Self::chars`], and may also differ
    /// between subsequent calls due to returning the best coverage of either the original
    /// characters or one of the normalized forms.
    pub fn calculate_coverage(
        &mut self,
        covers: impl Fn(char) -> bool,
        analysis_data_sources: &AnalysisDataSources,
    ) -> Coverage {
        let len = self.len();
        let mut best_coverage = Mapper {
            chars: &self.chars[..len],
            map_len: self.map_len,
        }
        .coverage(&covers);
        if best_coverage.is_complete() {
            return best_coverage;
        }
        if self.force_normalize && self.composed(analysis_data_sources).is_some() {
            let coverage = self.comp.coverage(&covers);
            if coverage > best_coverage {
                best_coverage = coverage;
                if coverage.is_complete() {
                    return best_coverage;
                }
            }
        }
        if self.decomposed(analysis_data_sources).is_some() {
            let coverage = self.decomp.coverage(&covers);
            if coverage > best_coverage {
                best_coverage = coverage;
                if coverage.is_complete() {
                    return best_coverage;
                }
            }
            if !self.force_normalize && self.composed(analysis_data_sources).is_some() {
                let coverage = self.comp.coverage(&covers);
                if coverage > best_coverage {
                    best_coverage = coverage;
                    if coverage.is_complete() {
                        return best_coverage;
                    }
                }
            }
        }

        best_coverage
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
        let mut is_emoji_or_pictograph = false;
        let mut map_len: u8 = 0;
        let start = *code_unit_offset_in_string as u32;

        for ((_, ch), (info, style_index)) in
            segment_text.char_indices().zip(item_infos_iter.by_ref())
        {
            force_normalize |= info.force_normalize();
            // TODO - make emoji detection more complete, as per (except using composite Trie tables as
            //  much as possible:
            //  https://github.com/conor-93/parley/blob/4637d826732a1a82bbb3c904c7f47a16a21cceec/parley/src/shape/mod.rs#L221-L269
            is_emoji_or_pictograph |= info.is_emoji_or_pictograph();
            *code_unit_offset_in_string += ch.len_utf8();

            // TODO: Explore ignoring other modifiers in determining `contributes_to_shaping`:
            //  regional indicators, subdivision flag tag sequences, skin tone modifiers
            //  See also: https://github.com/google/emoji-segmenter

            // If the color emoji has a non-printing variation selector, ignore the variation selector.
            // Its presentation depends on the platform and font.
            //
            // e.g.
            //  - `U+270C + U+FE0E`: `✌`, force text presentation
            //  - `U+270C + U+FE0F`: `✌️`, force emoji presentation
            //
            // See <https://www.unicode.org/reports/tr51/#Emoji_Variation_Sequences> for emoji
            // variation sequences and
            // <https://www.unicode.org/versions/Unicode17.0.0/core-spec/chapter-23/#G19053> for
            // variation selectors more generally.
            let is_emoji_with_non_printing_variation_selector =
                is_emoji_or_pictograph && info.is_variation_selector();

            let contributes_to_shaping =
                info.contributes_to_shaping() && !is_emoji_with_non_printing_variation_selector;
            if contributes_to_shaping {
                map_len = map_len.saturating_add(1);
            }

            self.chars.push(Char {
                ch,
                contributes_to_shaping,
                style_index,
                is_control_character: info.is_control(),
            });
        }

        // Finalize cluster metadata
        let end = *code_unit_offset_in_string as u32;
        self.is_emoji = is_emoji_or_pictograph;
        self.map_len = map_len;
        self.start = start;
        self.end = end;
        self.force_normalize = force_normalize;
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
    #[expect(clippy::cast_possible_truncation, reason = "Deferred")]
    fn setup(&mut self) {
        self.map_len = self
            .chars()
            .iter()
            .filter(|c| !c.is_control_character)
            .count() as u8;
    }

    #[inline(always)]
    fn coverage(&self, covers: &impl Fn(char) -> bool) -> Coverage {
        Mapper {
            chars: &self.chars[..self.len as usize],
            map_len: self.map_len,
        }
        .coverage(covers)
    }
}

struct Mapper<'a> {
    chars: &'a [Char],
    map_len: u8,
}

impl<'a> Mapper<'a> {
    /// Returns the coverage of characters contributing to shaping.
    fn coverage(&self, covers: &impl Fn(char) -> bool) -> Coverage {
        let mut mapped: u8 = 0;
        for c in self.chars.iter() {
            if c.contributes_to_shaping && covers(c.ch) {
                mapped = mapped.saturating_add(1);
            }
        }

        // If this assert is hit, some bookkeeping is wrong somewhere, but the returned coverage is
        // then simply considered to be complete.
        debug_assert!(
            mapped <= self.map_len,
            "Counted more covered characters than the number of characters we consider to be contributing."
        );

        Coverage {
            covered: mapped,
            total: self.map_len,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Coverage;

    #[test]
    fn ordering() {
        assert!(Coverage::NONE < Coverage::COMPLETE);

        assert_eq!(
            Coverage {
                covered: 0,
                total: 0,
            },
            Coverage::COMPLETE
        );
        assert_eq!(
            Coverage {
                covered: 5,
                total: 5,
            },
            Coverage::COMPLETE
        );
        assert_eq!(
            Coverage {
                // `covered` is greater than `total`, which shouldn't be produced by us, but is
                // possible because `Coverage`'s fields are `pub`.
                covered: 3,
                total: 2,
            },
            Coverage::COMPLETE
        );
        assert_eq!(
            Coverage {
                covered: 255,
                total: 255,
            },
            Coverage::COMPLETE
        );

        assert!(
            Coverage {
                covered: 4,
                total: 5,
            } > Coverage::NONE
        );
        assert!(
            Coverage {
                covered: 4,
                total: 5,
            } < Coverage::COMPLETE
        );

        assert!(
            Coverage {
                covered: 254,
                total: 255,
            } > Coverage::NONE
        );
        assert!(
            Coverage {
                covered: 254,
                total: 255,
            } < Coverage::COMPLETE
        );
    }
}
