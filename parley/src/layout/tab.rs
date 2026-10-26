// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tab stops, following [CSS `tab-size`][tab-size].
//!
//! A tab shapes to no glyphs and no advance (see [`Whitespace::Tab`]). Its advance is the
//! distance to the next tab stop from where it lands, which depends on the line breaking, so the
//! line breaker computes it with [`tab_advance`] as it places the tab, and records it for the line
//! (see [`TabAdvance`]). Everything that reads the line back adds it to the tab's shaped advance
//! through [`EffectiveSpacing`], like justification.
//!
//! [tab-size]: https://www.w3.org/TR/css-text-4/#tab-size-property
//! [`Whitespace::Tab`]: parley_engine::shape::Whitespace::Tab
//! [`TabAdvance`]: crate::layout::data::TabAdvance
//! [`EffectiveSpacing`]: crate::layout::spacing::EffectiveSpacing

/// The advance of a tab that starts `x` past the line's start edge, with tab stops every
/// `tab_size`.
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
