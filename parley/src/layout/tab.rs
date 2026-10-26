// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tab stops, following [CSS Text 4][css-tab-stop].
//!
//! A tab shapes to no glyphs, with the advance of a space. Its advance depends on where it lands
//! on the line, so the line breaker sizes each tab as it places it, by overwriting the tab
//! cluster's advance with [`tab_advance`]. Everything downstream (hanging whitespace, alignment,
//! glyph positioning, hit testing) then sees the sized tab through the cluster advance like any
//! other cluster.
//!
//! [css-tab-stop]: https://www.w3.org/TR/css-text-4/#tab-stop

use crate::layout::spacing::{EffectiveSpacing, Justification};
use crate::layout::{Spacing, Style};
use crate::style::Brush;
use parley_engine::ShapedText;
use parley_engine::shape::Whitespace;

/// The advance of a tab that starts `x` past the line's start edge, with tab stops every `tab_size`.
///
/// `x` includes the line's indent, as tab stops are measured from the line's start edge. The tab
/// advances to the next stop, or to the one after it if the next is less than a sixteenth of the
/// tab size away, so that a tab is always visible. CSS phrases that threshold as half a space; the
/// tab size is a length here, so the threshold is taken relative to it instead, which agrees with
/// CSS at its default of eight spaces per tab stop. A tab size that isn't positive gives the tab
/// no advance.
pub(crate) fn tab_advance(x: f32, tab_size: f32) -> f32 {
    if tab_size.is_nan() || tab_size <= 0. {
        return 0.;
    }

    // The distance past the previous tab stop, in `0.0..tab_size`. The remainder takes the sign of
    // `x`, which can be negative under a negative indent.
    let mut past_stop = x % tab_size;
    if past_stop < 0. {
        past_stop += tab_size;
    }

    let mut distance = tab_size - past_stop;
    if distance < tab_size / 16. {
        distance += tab_size;
    }
    distance
}

/// Sizes the cluster at `cluster_idx` if it is a tab starting at `x` on the line.
///
/// `x` is measured from the line's start edge, i.e., it includes the line's indent.
pub(crate) fn size_tab<B: Brush>(
    shaped_text: &mut ShapedText,
    styles: &[Style<B>],
    cluster_idx: u32,
    x: f32,
) {
    let cluster = shaped_text.shaped_clusters()[cluster_idx as usize];
    if cluster.whitespace() != Whitespace::Tab {
        return;
    }
    let tab_size = styles[cluster.style_index as usize].tab_size;
    shaped_text.set_tab_advance(cluster_idx, tab_advance(x, tab_size));
}

/// Sizes the next tab of run `run_idx` at or after `cluster_idx`, if it is within reach, and
/// returns the cluster index up to which the breaker can now proceed: just past that tab's atom,
/// or the run's end if there is none.
///
/// The tab is sized as if the line breaker places every atom before it on the current line, from
/// `x`, the line's current advance including its indent. Should a line break fall before the tab
/// after all, the breaker returns and the next call sizes it again from its new position. The
/// scan runs through newlines, which have no advance: `break_next` returns at a newline and
/// re-sizes the tab on re-entry, while `break_next_with_length` keeps the line going past it.
///
/// The scan looks at most [`MAX_SCAN_ATOMS`] atoms ahead. Past that, it returns where it stopped
/// without sizing anything, and the breaker calls again once it has consumed those atoms. Without
/// the cap, a tab far into a run would be scanned towards again on every line before it, making
/// line breaking quadratic in the number of lines.
pub(crate) fn size_next_tab<B: Brush>(
    shaped_text: &mut ShapedText,
    styles: &[Style<B>],
    run_idx: u32,
    cluster_idx: u32,
    x: f32,
    spacing: Spacing,
) -> u32 {
    let effective_spacing = EffectiveSpacing::new(spacing, Justification::NONE);
    let slice = shaped_text.run_slice(run_idx);

    let mut x = x;
    let mut scanned_to = slice.shaped_clusters_range().end;
    let mut tab = None;
    for (count, atom) in slice.atoms_from(cluster_idx).enumerate() {
        let clusters = atom.shaped_clusters_range();
        if atom.shaped_clusters()[0].whitespace() == Whitespace::Tab {
            tab = Some(clusters);
            break;
        }
        x += effective_spacing.atom_advance(&atom);
        if count + 1 == MAX_SCAN_ATOMS {
            scanned_to = clusters.end;
            break;
        }
    }

    match tab {
        Some(clusters) => {
            size_tab(shaped_text, styles, clusters.start, x);
            clusters.end
        }
        None => scanned_to,
    }
}

/// The most atoms [`size_next_tab`] scans ahead before handing back to the breaker.
const MAX_SCAN_ATOMS: usize = 64;

#[cfg(test)]
mod tests {
    use super::tab_advance;

    #[test]
    fn tab_advances_to_the_next_stop() {
        assert_eq!(tab_advance(0., 80.), 80.);
        assert_eq!(tab_advance(80., 80.), 80.);
        assert_eq!(tab_advance(30., 80.), 50.);
        assert_eq!(tab_advance(170., 80.), 70.);
    }

    #[test]
    fn tab_skips_a_stop_less_than_a_sixteenth_away() {
        assert_eq!(tab_advance(76., 80.), 84.);
        // Exactly a sixteenth away is not "less than", so the next stop is kept.
        assert_eq!(tab_advance(75., 80.), 5.);
        assert_eq!(tab_advance(74., 80.), 6.);
    }

    #[test]
    fn tab_under_a_negative_indent_measures_from_the_previous_stop() {
        assert_eq!(tab_advance(-30., 80.), 30.);
        assert_eq!(tab_advance(-80., 80.), 80.);
    }

    #[test]
    fn tab_size_that_is_not_positive_gives_no_advance() {
        assert_eq!(tab_advance(10., 0.), 0.);
        assert_eq!(tab_advance(10., -1.), 0.);
        assert_eq!(tab_advance(10., f32::NAN), 0.);
    }
}
