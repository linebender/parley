// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Additional advance applied on top of shaped advance.

use alloc::vec::Vec;
use core::ops::Range;
use parley_engine::shape::Whitespace;
use parley_engine::{Atom, ShapedSlice};

/// Whether the whitespace is a word separator.
///
/// This mostly follows [CSS Text 3 § 7.1][css-word-separator], in that fixed-width spaces are not
/// considered word separators (and do not get stretched). Unicode's segmentation does consider such
/// fixed-width spaces to be word boundaries.
///
/// Note that the spec includes, e.g., the Ethiopic word space (U+1361) as a word separator (which
/// generally has a glyph), but we currently do not.
///
/// [css-word-separator]: https://www.w3.org/TR/css-text-3/#word-separator
pub(crate) fn is_word_separator(whitespace: Whitespace) -> bool {
    whitespace.is_space_or_nbsp()
}

/// Additional gaps to apply around a shaped advance.
///
/// This spacing is in visual order.
///
/// [CSS Text 4 § 8][css-spacing] says to apply spacing around word separators and "typographic
/// character units." Such a typographic character unit is an extended grapheme cluster, i.e.,
/// [`parley_engine::Grapheme`]. Word separators form their own graphemes. As ligatures should not
/// be broken up and we cannot tell `HarfRust` we want spacing between graphemes, you probably want
/// to apply spacing at the boundaries of an [`parley_engine::Atom`] (and in case of letter spacing,
/// disable optional ligation when shaping).
///
/// [css-spacing]: https://www.w3.org/TR/css-text-4/#spacing
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct Gaps {
    /// For horizontal text, additional spacing visually to the left.
    ///
    /// For vertical text, this is to the top.
    ///
    /// Note: this field is wired up, but it's currently always zero as we follow Blink in assigning
    /// word and letter spacing to be visually after (see also docs on [`Spacing`]). However, e.g.,
    /// `text-justify: inter-character` would distribute spacing around graphemes, at which point
    /// `before` becomes used.
    pub(crate) before: f32,

    /// For horizontal text, additional spacing visually to the right.
    ///
    /// For vertical text, this is to the bottom.
    pub(crate) after: f32,
}

impl Gaps {
    pub(crate) const ZERO: Self = Self {
        before: 0.,
        after: 0.,
    };

    /// The total gap.
    #[inline(always)]
    pub(crate) const fn total(self) -> f32 {
        self.before + self.after
    }
}

/// Additional spacing to apply.
///
/// Word and letter spacing are applied visually after atoms. Note Blink and Gecko behave
/// differently: Blink applies spacing visually after, but Gecko applies it logically after. There
/// is [some discussion on whether the CSS specs should improve][css-reconsider-spacing].
///
/// [css-reconsider-spacing]: https://github.com/w3c/csswg-drafts/issues/10193#issue-2234554156
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct Spacing {
    /// Additional spacing inserted after every word.
    ///
    /// See [CSS Text 4 § 8.1][css-word-spacing].
    ///
    /// [css-word-spacing]: https://www.w3.org/TR/css-text-4/#word-spacing-property
    pub(crate) word: f32,
    /// Additional spacing inserted after every atom.
    ///
    /// See [CSS Text 4 § 8.2][css-letter-spacing].
    ///
    /// [css-letter-spacing]: https://www.w3.org/TR/css-text-4/#letter-spacing-property
    pub(crate) letter: f32,
}

impl Spacing {
    #[inline(always)]
    pub(crate) const fn new(word: f32, letter: f32) -> Self {
        Self { word, letter }
    }

    #[inline(always)]
    pub(crate) const fn is_zero(self) -> bool {
        self.word == 0. && self.letter == 0.
    }

    /// The gaps around an atom whose first character:
    /// - has the given whitespace class, and
    /// - has the given value for its [`CharacterFlags::is_default_ignorable`].
    ///
    /// This is [`EffectiveSpacing::gaps`] under [`Justification::NONE`].
    ///
    /// In CSS [letter-spacing][] ignores "invisible zero-width formatting characters", but
    /// doesn't specify an exact list. Blink chooses to ignore clusters which start with a default
    /// ignorable character, so we do the same. Blink does additionally hardcode skipping letter-spacing
    /// for U+000C FORM FEED, but we choose not to, as the WPTs suggest it should have ink.
    ///
    /// Only checking the first character does sometimes cause undesirable results, primarily due to
    /// U+115F HANGUL CHOSEONG FILLER. Blink has the same bug, but fixing this would be welcome.
    ///
    /// [`CharacterFlags::is_default_ignorable`]: parley_engine::shape::CharacterFlags::is_default_ignorable
    /// [letter-spacing]: https://www.w3.org/TR/css-text-4/#letter-spacing-property
    #[inline(always)]
    pub(crate) fn gaps(self, whitespace: Whitespace, is_default_ignorable: bool) -> Gaps {
        if whitespace == Whitespace::Newline {
            return Gaps::ZERO;
        }

        let mut gaps = Gaps {
            before: 0.,
            after: if is_default_ignorable {
                0.
            } else {
                self.letter
            },
        };

        if is_word_separator(whitespace) {
            gaps.after += self.word;
        }

        gaps
    }
}

/// Justification to apply to a single line.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct Justification {
    /// The amount of additional spacing to apply per justification opportunity.
    pub(crate) amount_per_opportunity: f32,

    /// One past the line's logically last shaped cluster eligible for justification. Shaped
    /// clusters at or beyond this index are not justified, as they hang past the line's end, or are
    /// part of the last atom that doesn't fully hang (e.g., an NBSP doesn't hang, but if it closes
    /// the line, it shouldn't get justified).
    pub(crate) justification_end_cluster: u32,
}

impl Justification {
    /// No justification.
    pub(crate) const NONE: Self = Self {
        amount_per_opportunity: 0.,
        justification_end_cluster: u32::MAX,
    };
}

impl Default for Justification {
    fn default() -> Self {
        Self::NONE
    }
}

/// Additional spacing to apply, including justification (if any).
#[derive(Copy, Clone, Debug)]
pub(crate) struct EffectiveSpacing {
    spacing: Spacing,
    justification: Justification,
}

impl EffectiveSpacing {
    /// The combined [`Spacing`] and [`Justification`] effective for a run.
    ///
    /// To measure text for purposes like line breaking, pass [`Justification::NONE`].
    #[inline(always)]
    pub(crate) const fn new(spacing: Spacing, justification: Justification) -> Self {
        Self {
            spacing,
            justification,
        }
    }

    #[inline(always)]
    pub(crate) const fn is_zero(self) -> bool {
        self.spacing.is_zero() && self.justification.amount_per_opportunity == 0.
    }

    /// The gaps around `atom`.
    #[inline(always)]
    pub(crate) fn gaps(self, atom: &Atom<'_>) -> Gaps {
        let first_cluster = atom.shaped_clusters()[0];
        let whitespace = first_cluster.whitespace();
        let mut gaps = self
            .spacing
            .gaps(whitespace, first_cluster.is_default_ignorable());

        if is_word_separator(whitespace)
            && atom.shaped_clusters_range().end <= self.justification.justification_end_cluster
        {
            gaps.after += self.justification.amount_per_opportunity;
        }

        gaps
    }

    /// The total advance of `atom`, i.e., the sum of the shaped advance and gaps.
    #[inline(always)]
    pub(crate) fn atom_advance(self, atom: &Atom<'_>) -> f32 {
        atom.advance() + self.gaps(atom).total()
    }
}

/// Write the [`Gaps`] for the shaped clusters in `slice` into `cluster_gaps`,
/// given that `spacing` applies.
pub(crate) fn push_cluster_gaps(
    cluster_gaps: &mut Vec<Gaps>,
    slice: ShapedSlice<'_>,
    spacing: EffectiveSpacing,
    is_rtl: bool,
) {
    let start = cluster_gaps.len();
    cluster_gaps.resize(start + slice.shaped_clusters().len(), Gaps::ZERO);
    let entries = &mut cluster_gaps[start..];
    let base = slice.shaped_clusters_range().start;
    for atom in slice.atoms_start() {
        let gaps = spacing.gaps(&atom);
        if gaps.before == 0. && gaps.after == 0. {
            continue;
        }
        let clusters = atom.shaped_clusters();
        let (first, last) = if is_rtl {
            (clusters.len() - 1, 0)
        } else {
            (0, clusters.len() - 1)
        };
        let offset = (atom.shaped_clusters_range().start - base) as usize;
        entries[offset + first].before += gaps.before;
        entries[offset + last].after += gaps.after;
    }
}

/// A slice of the [`Gaps`] for the shaped clusters with global indices
/// `run_cluster_start..run_cluster_start + entries.len()`
#[derive(Copy, Clone, Debug)]
pub(crate) struct RunGaps<'a> {
    entries: &'a [Gaps],
    /// The global index of the first shaped cluster whose gap is in `entries`.
    /// (That is, to get the index of a shaped cluster's gaps in `entries`, we subtract
    /// `run_cluster_start` from its global index).
    run_cluster_start: u32,
}

impl<'a> RunGaps<'a> {
    #[inline]
    pub(crate) fn new(entries: &'a [Gaps], run_cluster_start: u32) -> Self {
        Self {
            entries,
            run_cluster_start,
        }
    }

    #[inline]
    pub(crate) fn for_cluster_range(self, range: Range<u32>) -> &'a [Gaps] {
        &self.entries[(range.start - self.run_cluster_start) as usize
            ..(range.end - self.run_cluster_start) as usize]
    }

    pub(crate) fn for_atom(self, atom: &Atom<'_>) -> Gaps {
        let mut gaps = Gaps::ZERO;
        for entry in self.for_cluster_range(atom.shaped_clusters_range()) {
            gaps.before += entry.before;
            gaps.after += entry.after;
        }
        gaps
    }
}
