// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Whitespace-related utilities, for implementing e.g. whitespace collapse and whitespace hanging.
//!
//! # Conditional vs. unconditional whitespace hanging
//!
//! Under CSS Text 4 § 4.3.2, some whitespace hangs, and some may hang *conditionally*.
//! Conditionally hanging whitespace only hangs if it overflows the line. A sequence of
//! `white-space-collapse: preserve; text-wrap-mode: wrap;` whitespace hangs conditionally if it's
//! directly followed by a forced line break; otherwise, it hangs unconditionally. The end of the
//! layout counts as a forced line break (CSS Text 4 § 5). In particular, if the `preserve` sequence
//! is followed by a sequence of `collapse` whitespace (and not a break), then it hangs
//! unconditionally. Effectively, the two sequences can just be treated as one that's
//! unconditionally hanging.
//!
//! Under CSS Text 4 § 9.2, unconditionally hanging whitespace followed by conditionally hanging
//! whitespace, hangs in full, but only if the conditionally-hanging whitespace hangs in full. See
//! <https://github.com/linebender/parley/pull/831> for an HTML sample.
//!
//! So, in effect, the only behaviorally observable case of conditional vs. unconditional hanging
//! can be represented as a suffix of conditionally hanging whitespace, preceded by unconditionally
//! hanging whitespace.
//!
//! Further note that normally, under `white-space-collapse: collapse`, most but not all whitespace
//! is collapsed; e.g., the Ideographic Space (U+3000) does not collapse. It generally takes
//! deliberate effort to write a sample that exercises these edge cases.

use parley_engine::Atom;
use parley_engine::shape::Whitespace;

use crate::layout::Style;
use crate::style::Brush;
use crate::{TextWrapMode, WhiteSpaceCollapse};

impl WhiteSpaceCollapse {
    /// Whether `c` is whitespace that this mode collapses.
    pub(crate) fn is_collapsible(self, c: char) -> bool {
        match self {
            Self::Collapse => c.is_ascii_whitespace(),
            Self::Preserve | Self::BreakSpaces => false,
            Self::PreserveBreaks => matches!(c, ' ' | '\t'),
        }
    }
}

/// Whether whitespace with the given style hangs past the line's end edge.
///
/// Following [CSS Text 4 § 4.3.2][css-hanging], non-breaking spaces don't hang, preserved
/// whitespace only hangs when wrapping is enabled, and whitespace in a collapsing mode always
/// hangs.
///
/// [css-hanging]: https://www.w3.org/TR/css-text-4/#white-space-phase-2
///
// Note: CSS Text 4 effectively includes tab, newline, plus all of the "space separators" here
// (Unicode category `Zs`), except for non-breaking space. However, we only include space and
// ideographic space from the "space separators". See
// https://github.com/linebender/parley/pull/762#discussion_r3923722770.
#[inline(always)]
pub(crate) fn whitespace_hangs<B: Brush>(whitespace: Whitespace, style: &Style<B>) -> bool {
    match whitespace {
        Whitespace::Newline => true,
        Whitespace::Space | Whitespace::IdeographicSpace | Whitespace::Tab => {
            match style.white_space_collapse {
                WhiteSpaceCollapse::BreakSpaces => false,
                WhiteSpaceCollapse::Preserve => style.text_wrap_mode == TextWrapMode::Wrap,
                _ => true,
            }
        }
        _ => false,
    }
}

/// Whether `atom` hangs past the line's end.
///
/// Atoms are the unit of hanging whitespace in Parley. There are some cases where an atom could
/// have a suffix which nominally would hang (primarily a prefix grapheme + a space). However, we don't
/// expect this to actually happen in non-adversarial text, and supporting it would make a number of other
/// code much more complicated (especially for bidi reordering).
#[inline(always)]
pub(crate) fn atom_hangs<B: Brush>(atom: &Atom<'_>, styles: &[Style<B>]) -> bool {
    atom.characters().iter().all(|character| {
        whitespace_hangs(
            character.whitespace,
            &styles[character.style_index as usize],
        )
    })
}
