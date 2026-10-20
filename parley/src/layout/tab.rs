// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tab stops, following [CSS Text 4 § 4.1.3][css-tab-stop].
//!
//! A tab shapes to no glyphs and the advance of a space. Its advance
//! depends on where it lands on the line, so the line breaker sizes each tab as it places it, by
//! overwriting the tab cluster's advance with [`tab_advance`]. Everything downstream (hanging
//! whitespace, alignment, glyph positioning, hit testing) then sees the sized tab through the
//! cluster advance like any other cluster.
//!
//! [css-tab-stop]: https://www.w3.org/TR/css-text-4/#tab-stop

use crate::TabSize;
use crate::layout::Spacing;

/// The advance of a tab whose start edge is at `x`, measured from the line's start edge.
///
/// `space_advance` is the advance of the space glyph of the tab's font, and `spacing` the word and
/// letter spacing of the tab's run, which together make up the unit of [`TabSize::Spaces`].
///
/// The tab shifts to the next tab stop, or to the one after it if the next is less than half a
/// space away so that a tab is always visible. CSS phrases that threshold as `0.5ch`; like Blink,
/// we use half the space advance. A tab size that isn't positive renders the tab as nothing.
#[allow(dead_code)]
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
