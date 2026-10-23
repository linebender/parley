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

use crate::TabSize;
use crate::layout::spacing::{EffectiveSpacing, Justification};
use crate::layout::{Spacing, Style};
use crate::style::Brush;
use parley_engine::ShapedText;
use parley_engine::shape::Whitespace;

/// The advance of a tab whose start edge is at `x`, measured from the line's start edge.
///
/// `space_advance` is the advance of the space glyph of the tab's font, and `spacing` the word and
/// letter spacing of the tab's run, which together make up the unit of [`TabSize::Spaces`].
///
/// The tab shifts to the next tab stop, or to the one after it if the next is less than half a
/// space away so that a tab is always visible. CSS phrases that threshold as `0.5ch`; like Blink,
/// we use half the space advance. A tab size that isn't positive gives the tab no advance of its
/// own (spacing gaps still apply).
pub(crate) fn tab_advance(x: f32, tab_size: TabSize, space_advance: f32, spacing: Spacing) -> f32 {
    let base = match tab_size {
        TabSize::Spaces(spaces) => spaces * (space_advance + spacing.letter + spacing.word),
        TabSize::Length(length) => length,
    };
    if base <= 0. || base.is_nan() {
        return 0.;
    }

    // The distance from the previous tab stop, in `0.0..base`. The remainder takes the sign of
    // `x`, which can be negative under a negative indent.
    let mut past_stop = x % base;
    if past_stop < 0. {
        past_stop += base;
    }

    let mut distance = base - past_stop;
    if distance < space_advance / 2. {
        distance += base;
    }
    distance
}

/// Sizes the cluster at `cluster_idx` if it is a tab starting at `x` on the line.
///
/// `space_advance` and `spacing` are those of the cluster's run; `x` is measured from the line's
/// start edge, i.e., it includes the line's indent.
pub(crate) fn size_tab<B: Brush>(
    shaped_text: &mut ShapedText,
    styles: &[Style<B>],
    cluster_idx: u32,
    x: f32,
    space_advance: f32,
    spacing: Spacing,
) {
    let cluster = shaped_text.shaped_clusters()[cluster_idx as usize];
    if cluster.whitespace() != Whitespace::Tab {
        return;
    }
    let tab_size = styles[cluster.style_index as usize].tab_size;
    shaped_text.set_tab_advance(
        cluster_idx,
        tab_advance(x, tab_size, space_advance, spacing),
    );
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
    space_advance: f32,
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
            size_tab(
                shaped_text,
                styles,
                clusters.start,
                x,
                space_advance,
                spacing,
            );
            clusters.end
        }
        None => scanned_to,
    }
}

/// The most atoms [`size_next_tab`] scans ahead before handing back to the breaker.
const MAX_SCAN_ATOMS: usize = 64;

#[cfg(test)]
mod tests {
    use super::*;

    const SPACE: f32 = 10.;
    const NO_SPACING: Spacing = Spacing::new(0., 0.);

    fn eight_spaces(x: f32) -> f32 {
        tab_advance(x, TabSize::default(), SPACE, NO_SPACING)
    }

    #[test]
    fn default_is_eight_spaces() {
        assert_eq!(eight_spaces(0.), 80.);
        assert_eq!(eight_spaces(80.), 80.);
        assert_eq!(eight_spaces(30.), 50.);
        assert_eq!(eight_spaces(170.), 70.);
    }

    #[test]
    fn skips_a_stop_that_is_less_than_half_a_space_away() {
        assert_eq!(eight_spaces(76.), 84.);
        // Exactly half a space away is not "less than", so the next stop is kept.
        assert_eq!(eight_spaces(75.), 5.);
        assert_eq!(eight_spaces(74.), 6.);
    }

    #[test]
    fn negative_x_measures_from_the_previous_stop() {
        assert_eq!(eight_spaces(-30.), 30.);
        assert_eq!(eight_spaces(-80.), 80.);
    }

    #[test]
    fn spaces_include_letter_and_word_spacing() {
        let advance = tab_advance(0., TabSize::Spaces(4.), SPACE, Spacing::new(2., 1.));
        assert_eq!(advance, 4. * (10. + 2. + 1.));
    }

    #[test]
    fn length_is_absolute() {
        assert_eq!(
            tab_advance(10., TabSize::Length(25.), SPACE, Spacing::new(2., 1.)),
            15.
        );
    }

    #[test]
    fn non_positive_size_renders_nothing() {
        assert_eq!(tab_advance(10., TabSize::Spaces(0.), SPACE, NO_SPACING), 0.);
        assert_eq!(
            tab_advance(10., TabSize::Length(-1.), SPACE, NO_SPACING),
            0.
        );
        assert_eq!(tab_advance(10., TabSize::Spaces(8.), 0., NO_SPACING), 0.);
    }
}
