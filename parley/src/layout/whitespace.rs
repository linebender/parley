// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Some whitespace-related utilities.

use parley_engine::shape::Whitespace;
use parley_engine::{Atom, ShapedSlice};

use crate::TextWrapMode;
use crate::layout::Style;
use crate::layout::spacing::EffectiveSpacing;
use crate::style::Brush;

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

/// The advance of the logically trailing clusters of `atom` that can hang past the line's end, and
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
                // Note whitespace only hangs with `TextWrapMode::Wrap`, following
                // CSS Text 4 § 4.3.2.
                whitespace_can_hang(character.info.whitespace())
                    && styles[character.style_index as usize].text_wrap_mode == TextWrapMode::Wrap
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
