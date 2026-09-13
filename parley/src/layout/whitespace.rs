// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Some whitespace-related utilities.

use parley_engine::shape::Whitespace;
use parley_engine::{Atom, ShapedSlice};

use crate::layout::Style;
use crate::layout::spacing::EffectiveSpacing;
use crate::style::Brush;
use crate::{TextWrapMode, WhiteSpaceCollapse};

impl WhiteSpaceCollapse {
    /// Whether `c` is whitespace that this mode collapses.
    pub(crate) fn is_collapsible(self, c: char) -> bool {
        match self {
            Self::Collapse => c.is_ascii_whitespace(),
            Self::Preserve => false,
            Self::PreserveBreaks => matches!(c, ' ' | '\t'),
        }
    }
}

/// Whether this is whitespace that is allowed to hang past the line.
///
/// Following [CSS Text 4 § 4.3.2][css-hanging], non-breaking spaces don't hang.
///
/// [css-hanging]: https://www.w3.org/TR/css-text-4/#white-space-phase-2
///
// Note: CSS Text 4 effectively includes tab, newline, plus all of the "space separators" here
// (Unicode category `Zs`), except for non-breaking space. However, we only include space and
// ideographic space from the "space separators". See
// https://github.com/linebender/parley/pull/762#discussion_r3923722770.
#[inline(always)]
pub(crate) const fn whitespace_can_hang(whitespace: Whitespace) -> bool {
    matches!(
        whitespace,
        Whitespace::Space | Whitespace::IdeographicSpace | Whitespace::Tab | Whitespace::Newline
    )
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Hanging {
    Never,
    Conditional,
    Always,
}

impl Hanging {
    pub(crate) fn for_character<B: Brush>(whitespace: Whitespace, style: &Style<B>) -> Self {
        if whitespace == Whitespace::Newline {
            return Self::Always;
        }
        if !whitespace_can_hang(whitespace) {
            return Self::Never;
        }
        match (style.white_space_collapse, style.text_wrap_mode) {
            (WhiteSpaceCollapse::Collapse | WhiteSpaceCollapse::PreserveBreaks, _) => Self::Always,
            (WhiteSpaceCollapse::Preserve, TextWrapMode::Wrap) => Self::Conditional,
            (WhiteSpaceCollapse::Preserve, TextWrapMode::NoWrap) => Self::Never,
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct HangingAdvance {
    pub(crate) unconditional: f32,
    pub(crate) total: f32,
}

impl HangingAdvance {
    pub(crate) fn add(&mut self, policy: Hanging, advance: f32) {
        match policy {
            Hanging::Always => self.unconditional = self.total + advance,
            Hanging::Conditional => self.unconditional = 0.,
            Hanging::Never => {}
        }
        self.total += advance;
    }
}

/// The advance of the logically trailing clusters of `atom` that can hang past the line's end, and
/// whether that is the atom in its entirety.
///
/// An atom can hang partially: e.g., a prepend character followed by a space is a single atom (as
/// it's a grapheme), but may consist of multiple shaped clusters, of which the spaces can hang.
/// The atom's spacing at its logical end hangs along with the clusters, and if the atom hangs in
/// its entirety does its spacing at its logical start hang as well.
///
/// `overflow` tracks remaining conditional hanging at forced ends. `None` allows hanging in full
/// at soft ends or before an unconditionally hanging glyph.
#[inline(always)]
pub(crate) fn atom_hanging_advance<B: Brush>(
    slice: ShapedSlice<'_>,
    atom: &Atom<'_>,
    styles: &[Style<B>],
    spacing: EffectiveSpacing,
    is_rtl: bool,
    overflow: &mut Option<f32>,
) -> (f32, bool) {
    let gaps = spacing.gaps(atom);
    let (gap_start, gap_end) = if is_rtl {
        (gaps.after, gaps.before)
    } else {
        (gaps.before, gaps.after)
    };
    let mut hanging = 0.;
    let clusters = atom.shaped_clusters();
    for (index, cluster) in clusters.iter().enumerate().rev() {
        let characters = slice.characters_in(cluster.chars_range());
        let policy = characters
            .iter()
            .map(|character| {
                Hanging::for_character(
                    character.info.whitespace(),
                    &styles[character.style_index as usize],
                )
            })
            .min()
            .unwrap_or(Hanging::Never);

        let mut advance = cluster.advance;
        if index == clusters.len() - 1 {
            advance += gap_end;
        }
        if index == 0 {
            advance += gap_start;
        }

        match policy {
            Hanging::Never => return (hanging, false),
            Hanging::Always => {
                // Whitespace before an unconditionally hanging glyph hangs in full. Newlines
                // don't count: they have no advance and are not glyphs at the line's end.
                if characters
                    .iter()
                    .any(|character| character.info.whitespace() != Whitespace::Newline)
                {
                    *overflow = None;
                }
                hanging += advance;
            }
            Hanging::Conditional => {
                let Some(remaining) = overflow.as_mut() else {
                    hanging += advance;
                    continue;
                };
                // Only the part that doesn't fit hangs. A cluster that fits (including one
                // with a non-positive advance) doesn't hang, and so ends the hanging sequence.
                let allowed = remaining.max(0.).min(advance);
                if allowed <= 0. {
                    return (hanging, false);
                }
                hanging += allowed;
                *remaining -= allowed;
                if allowed < advance {
                    return (hanging, false);
                }
            }
        }
    }

    (hanging, true)
}
