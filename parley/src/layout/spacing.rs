// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Additional advance applied on top of shaped advance.

/// Additional gaps to apply around a shaped advance.
///
/// This spacing is in visual order.
///
/// [CSS Text 4 § 8][css-spacing] says to apply spacing around word separators and "typographic
/// characters units." Such a typographic character unit is an extended grapheme cluster, i.e.,
/// [`parley_engine::Grapheme`]. Word separators form their own graphemes. As ligatures should not
/// be broken up, you probably want to apply spacing at the boundaries of an
/// [`parley_engine::Atom`].
///
/// [css-spacing]: https://www.w3.org/TR/css-text-4/#spacing
#[derive(Copy, Clone, Debug)]
pub(crate) struct Gaps {
    /// For horizontal text, additional spacing visually to the left.
    ///
    /// For vertical text, this is to the top.
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
    pub(crate) const ZERO: Self = Self {
        word: 0.,
        letter: 0.,
    };

    #[inline(always)]
    pub(crate) const fn new(word: f32, letter: f32) -> Self {
        Self { word, letter }
    }

    #[inline(always)]
    pub(crate) const fn is_zero(self) -> bool {
        self.word == 0. && self.letter == 0.
    }
}

/// Justification to apply to a single line.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Justification {
    /// The amount of additional spacing to apply per justification opportunity.
    pub(crate) amount_per_opportunity: f32,

    /// The end of the line's logically last atom, i.e., the line's end, where justification won't
    /// be applied.
    pub(crate) line_end_char: u32,
}

