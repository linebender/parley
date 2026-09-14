// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Some whitespace-related utilities.

use core::ops::Range;

use parley_engine::shape::Whitespace;
use parley_engine::{Atom, ShapedSlice};

use crate::inline_box::InlineBoxKind;
use crate::layout::Style;
use crate::layout::data::{LayoutData, LayoutItem, LayoutItemKind};
use crate::layout::spacing::{EffectiveSpacing, Justification, is_word_separator};
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
/// Following [CSS Text 4 § 4.3.2][css-hanging], non-breaking spaces don't hang. See also
/// [`whitespace_hangs`].
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

/// Whether whitespace with the given style hangs past the line's end edge.
///
/// Following [CSS Text 4 § 4.3.2][css-hanging], preserved whitespace only hangs when wrapping is
/// enabled, whereas whitespace in a collapsing mode always hangs.
///
/// [css-hanging]: https://www.w3.org/TR/css-text-4/#white-space-phase-2
#[inline(always)]
pub(crate) fn whitespace_hangs<B: Brush>(whitespace: Whitespace, style: &Style<B>) -> bool {
    whitespace_can_hang(whitespace)
        && (style.white_space_collapse != WhiteSpaceCollapse::Preserve
            || style.text_wrap_mode == TextWrapMode::Wrap)
}

/// The advance of whitespace hanging past a line's end edge, split by whether it's hanging
/// conditionally or unconditionally.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub(crate) struct HangingAdvance {
    /// The advance of whitespace that hangs unconditionally.
    pub(crate) unconditional: f32,

    /// The advance of whitespace that hangs conditionally when followed by a forced line break.
    ///
    /// If followed by a forced line break, only the part that does not fit on a line hangs. If not
    /// followed by a forced line break, this part also hangs unconditionally.
    pub(crate) conditional: f32,
}

impl HangingAdvance {
    /// Add `advance` to the conditionally or unconditionally hanging advance.
    #[inline(always)]
    pub(crate) fn add(&mut self, conditional: bool, advance: f32) {
        if conditional {
            self.conditional += advance;
        } else {
            self.unconditional += advance;
        }
    }

    /// The advance that hangs when the line is not followed by a forced line break.
    #[inline(always)]
    pub(crate) fn total(self) -> f32 {
        self.unconditional + self.conditional
    }
}

impl core::ops::AddAssign for HangingAdvance {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        self.unconditional += rhs.unconditional;
        self.conditional += rhs.conditional;
    }
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
) -> (HangingAdvance, bool) {
    let gaps = spacing.gaps(atom);
    let (gap_start, gap_end) = if is_rtl {
        (gaps.after, gaps.before)
    } else {
        (gaps.before, gaps.after)
    };
    let mut last_cluster = true;
    let mut all_hang = true;
    let mut hanging = HangingAdvance::default();
    // Whether the last visited shaped cluster hangs conditionally.
    let mut conditional = false;
    for cluster in atom.shaped_clusters().iter().rev() {
        // Note, as a somewhat esoteric edge case, an atom's shaped whitespace clusters may
        // flip-flop between hanging conditionally and unconditionally when the `WhiteSpaceCollapse`
        // style changes inside that atom.
        //
        // CSS Text 4 § 4.3.2 doesn't say anything about this explicitly; it just says the trailing
        // sequence hangs conditionally or unconditionally, and § 1.4 leaves style changes inside a
        // grapheme as being undefined. We choose here to follow what § 4.3.1 does for collapsing,
        // by classifying each character by its own mode.
        conditional = false;
        let cluster_hangs = slice
            .characters_in(cluster.chars_range())
            .iter()
            .all(|character| {
                let style = &styles[character.style_index as usize];
                // Following CSS Text 4 § 4.3.2, only preserved whitespace can hang conditionally.
                conditional |= style.white_space_collapse == WhiteSpaceCollapse::Preserve;
                whitespace_hangs(character.info.whitespace(), style)
            });
        if !cluster_hangs {
            all_hang = false;
            break;
        }
        if last_cluster {
            hanging.add(conditional, gap_end);
            last_cluster = false;
        }
        hanging.add(conditional, cluster.advance);
    }

    if all_hang {
        hanging.add(conditional, gap_start);
    }

    (hanging, all_hang)
}

/// The whitespace hanging past the end of a line.
pub(crate) struct HangingWhitespace {
    /// The advance of the hanging whitespace.
    pub(crate) advance: HangingAdvance,
    /// The index one past the line's logically last shaped cluster that's eligible for
    /// justification (see [`Justification::justification_end_cluster`]).
    pub(crate) justification_end_cluster: u32,
    /// The number of justification opportunities that are no longer eligible. These are at or
    /// beyond `justification_end_cluster`.
    pub(crate) hanging_justification_opportunities: u32,
}

/// Returns the advance of the whitespace hanging past the end of the line made up of `items`,
/// the index one past its logically last shaped cluster that's eligible for justification (see
/// [`Justification::justification_end_cluster`]), and the number of justification opportunities
/// that are no longer eligible, as they are at or beyond that cluster.
///
/// Each item in `items` is paired with its range of the run's shaped clusters.
pub(crate) fn hanging_whitespace<B: Brush>(
    layout: &LayoutData<B>,
    items: impl Iterator<Item = (LayoutItem, Range<u32>)>,
) -> HangingWhitespace {
    let mut trailing = HangingWhitespace {
        advance: HangingAdvance::default(),
        // Atoms with shaped clusters before this index may be stretched by justification.
        justification_end_cluster: u32::MAX,
        hanging_justification_opportunities: 0,
    };

    for (item, cluster_range) in items {
        match item.kind {
            LayoutItemKind::InlineBox => {
                // Inline boxes don't hang.
                if layout.inline_boxes[item.index].kind == InlineBoxKind::InFlow {
                    break;
                }
            }
            LayoutItemKind::TextRun => {
                // Trailing whitespace can only hang if it ends up at the line's end edge after bidi
                // reordering. We don't currently apply UAX #9 L1 (resetting trailing whitespace to
                // paragraph level), so only logically-last items that match the paragraph level are
                // guaranteed to be at that edge.
                if item.bidi_level != layout.base_level {
                    break;
                }

                let effective_spacing =
                    EffectiveSpacing::new(layout.runs[item.index].spacing, Justification::NONE);
                let slice = layout
                    .shaped_text
                    .run_slice(item.index as u32)
                    .narrow(cluster_range);

                for atom in slice.atoms_end().rev() {
                    let (hanging, all_hang) = atom_hanging_advance(
                        slice,
                        &atom,
                        &layout.styles,
                        effective_spacing,
                        item.bidi_level.is_rtl(),
                    );
                    trailing.advance += hanging;
                    // Justification can't stretch within an atom, so it stops at the start of the
                    // last atom that hangs in its entirety or only partially.
                    trailing.justification_end_cluster = atom.shaped_clusters_range().start;
                    if is_word_separator(atom.characters()[0].info.whitespace()) {
                        trailing.hanging_justification_opportunities += 1;
                    }
                    if !all_hang {
                        return trailing;
                    }
                }
            }
        }
    }

    trailing
}
