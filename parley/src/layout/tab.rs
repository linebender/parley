// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tab stops, following [CSS `tab-size`][tab-size]. See [`TabAdvance`] for how a tab's advance
//! flows through layout.
//!
//! [tab-size]: https://www.w3.org/TR/css-text-4/#tab-size-property
//! [`TabAdvance`]: crate::layout::data::TabAdvance

/// The advance of a tab that starts `x` past the line's start edge, with tab stops every
/// `tab_size`.
///
/// `x` includes the line's indent, as tab stops are measured from the line's start edge. The tab
/// advances to the next stop, or to the one after it if the next is closer than `skip_threshold`
/// (see [`TabSize::skip_threshold`](crate::TabSize::skip_threshold)). Only that one stop is
/// skipped, so with stops closer together than the threshold the advance can still be less than
/// it. A tab size that isn't positive gives the tab no advance.
pub(crate) fn tab_advance(x: f32, tab_size: f32, skip_threshold: f32) -> f32 {
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
    if distance < skip_threshold {
        distance += tab_size;
    }
    distance
}

#[cfg(test)]
mod tests {
    use super::tab_advance;

    #[test]
    fn tab_advances_to_the_next_stop() {
        assert_eq!(tab_advance(0., 80., 5.), 80.);
        assert_eq!(tab_advance(80., 80., 5.), 80.);
        assert_eq!(tab_advance(30., 80., 5.), 50.);
        assert_eq!(tab_advance(170., 80., 5.), 70.);
    }

    #[test]
    fn tab_skips_a_stop_closer_than_the_skip_threshold() {
        assert_eq!(tab_advance(76., 80., 5.), 84.);
        // Exactly the threshold away is not "closer than", so the next stop is kept.
        assert_eq!(tab_advance(75., 80., 5.), 5.);
        assert_eq!(tab_advance(74., 80., 5.), 6.);
        // With a zero threshold, no stop is skipped.
        assert_eq!(tab_advance(79., 80., 0.), 1.);
        // Only one stop is skipped: with stops closer together than the threshold, the advance
        // ends up below the threshold.
        assert_eq!(tab_advance(3., 4., 8.), 5.);
    }

    #[test]
    fn tab_under_a_negative_indent_measures_from_the_previous_stop() {
        assert_eq!(tab_advance(-30., 80., 5.), 30.);
        assert_eq!(tab_advance(-80., 80., 5.), 80.);
    }

    #[test]
    fn tab_size_that_is_not_positive_gives_no_advance() {
        assert_eq!(tab_advance(10., 0., 5.), 0.);
        assert_eq!(tab_advance(10., -1., 5.), 0.);
        assert_eq!(tab_advance(10., f32::NAN, 5.), 0.);
    }
}
