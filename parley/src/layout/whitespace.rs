// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Some whitespace-related utilities.

use parley_engine::shape::{Character, Whitespace};
use parley_engine::{Atom, Boundary, ShapedSlice};

use crate::layout::Style;
use crate::layout::spacing::EffectiveSpacing;
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

fn is_break_space<B: Brush>(character: Character, styles: &[Style<B>]) -> bool {
    styles[character.style_index as usize].white_space_collapse == WhiteSpaceCollapse::BreakSpaces
        && matches!(
            character.info.whitespace(),
            Whitespace::Space | Whitespace::Tab | Whitespace::IdeographicSpace
        )
}

/// Whether a soft line break opportunity exists before the character at `index`.
///
/// Following [CSS Text 4 § 5.1][css-break-spaces], `white-space-collapse: break-spaces` permits
/// wrapping after each preserved space or tab, but not before the first one. Other Unicode
/// separators retain their UAX #14 opportunities.
///
/// [css-break-spaces]: https://www.w3.org/TR/css-text-4/#white-space-collapsing
pub(crate) fn soft_line_break<B: Brush>(
    characters: &[Character],
    index: usize,
    styles: &[Style<B>],
) -> bool {
    if index > 0 && is_break_space(characters[index - 1], styles) {
        return true;
    }
    let character = characters[index];
    character.info.boundary() == Boundary::Line && !is_break_space(character, styles)
}

/// The advance of the logically trailing clusters of `atom` that hang past the line's end, and
/// whether that is the atom in its entirety.
///
/// An atom can hang partially: e.g., a prepend character followed by a space is a single atom (as
/// it's a grapheme), but may consist of multiple shaped clusters, of which the spaces can hang.
/// The atom's spacing at its logical end hangs along with the clusters, and if the atom hangs in
/// its entirety does its spacing at its logical start hang as well.
#[inline(always)]
pub(crate) fn atom_hanging_advance<B: Brush>(
    slice: ShapedSlice<'_>,
    atom: &Atom<'_>,
    styles: &[Style<B>],
    spacing: EffectiveSpacing,
    is_rtl: bool,
) -> (f32, bool) {
    let gaps = spacing.gaps(atom);
    let (gap_start, gap_end) = if is_rtl {
        (gaps.after, gaps.before)
    } else {
        (gaps.before, gaps.after)
    };
    let mut last_cluster = true;
    let mut all_hang = true;
    let mut hanging = 0.;
    for cluster in atom.shaped_clusters().iter().rev() {
        let cluster_hangs = slice
            .characters_in(cluster.chars_range())
            .iter()
            .all(|character| {
                whitespace_hangs(
                    character.info.whitespace(),
                    &styles[character.style_index as usize],
                )
            });
        if !cluster_hangs {
            all_hang = false;
            break;
        }
        if last_cluster {
            hanging += gap_end;
            last_cluster = false;
        }
        hanging += cluster.advance;
    }

    let hanging = if all_hang {
        hanging + gap_start
    } else {
        hanging
    };

    (hanging, all_hang)
}
