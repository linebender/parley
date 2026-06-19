// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Seeded random generation of line-breaking test cases.
//!
//! We use these to ensure that our Line breaking decision exactly matches Chromium (or can be
//! made to do so). This is validated in the `chrome_linebreaking.rs` test.
//!
//! The text is intentionally restricted for now: ASCII letters (`a`-`z`, `A`-`Z`)
//! grouped into space-separated words, with no punctuation and no
//! leading/trailing whitespace. This keeps line-break opportunities to spaces
//! only. Our line breaking doesn't currently match Chrome around punctuation,
//! hence this restriction.
//!
//! The boundary cases are calculated by running `parley_tests/linebreaking_browser` in the browser,
//! then saved to `parley_tests/linebreaking_browser/data`, with a file per family.

use rand::{RngExt, SeedableRng, seq::IndexedRandom};
use rand_chacha::ChaCha8Rng;

// --- Shared, non-case types between runners ---

/// A font the harness collects data for.
#[derive(Clone, Copy, Debug)]
pub struct SupportedFont {
    /// The family name for the font, also used as the key for the data files.
    pub family: &'static str,
    /// The raw font file bytes, embedded so the browser harness needs no
    /// network or filesystem access.
    pub bytes: &'static [u8],
}

/// The fonts which we compare Chrome's line breaking against.
pub const FONTS: &[SupportedFont] = &[
    SupportedFont {
        family: "Roboto",
        bytes: include_bytes!("../../../parley_dev/assets/fonts/roboto_fonts/Roboto-Regular.ttf"),
    },
    SupportedFont {
        family: "Arimo",
        bytes: include_bytes!(
            "../../../parley_dev/assets/fonts/arimo_fonts/Arimo-VariableFont_wght.ttf"
        ),
    },
];

/// We perform all measurements as an integer number of 64th of a pixel subpixels.
///
/// This is particularly useful for binary search, to avoid floating point weirdness.
/// We don't need to go finer, as we know that Chrome's box layout is performed at this granularity.
pub const SUBPIXELS_PER_PX: f64 = 64.0;

/// The width we ask the browser to break at in cases where the first line
/// cannot be broken within. This can happen in two cases:
/// 1) Where the initial width is inside the first "word".
/// 2) Where the initial width is inside the second "word"; in that
///    case, the only thing left on the first line would be the first word.
///
/// We haven't reasoned about whether starting from zero would work here.
pub const PROBE_SUBPIXELS: i64 = 64; // 1px

// --- Case generation ---

const MIN_FONT_SIZE: f32 = 10.0;
const MAX_FONT_SIZE: f32 = 30.0;
const FONT_SIZE_STEP: f32 = 0.02;
#[expect(
    clippy::cast_possible_truncation,
    reason = "We know this doesn't overflow."
)]
const FONT_SIZE_STEPS: u32 = ((MAX_FONT_SIZE - MIN_FONT_SIZE) / FONT_SIZE_STEP) as u32;

/// Inclusive bounds on the container width as a multiplier of the font size.
const MIN_EM_FACTOR: f32 = 7.0;
const MAX_EM_FACTOR: f32 = 32.0;

/// Inclusive bounds on the number of words generated per case.
const MIN_WORDS: usize = 50;
const MAX_WORDS: usize = 100;

/// Inclusive bounds on the length (in characters) of each generated word.
const MIN_WORD_LEN: usize = 2;
const MAX_WORD_LEN: usize = 10;

/// A single seed-derived line-breaking test case.
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    /// The seed this case was generated from.
    pub seed: u64,
    /// Space-separated ASCII words.
    pub text: String,
    /// Font size in CSS pixels.
    pub font_size: f32,
    /// Initial container width in CSS pixels (`font_size * em_factor`). The
    /// Chromium harness uses this to decide where the first line breaks before tightening
    /// the width down to the minimum that preserves that break. This shall not be used by
    /// the Parley test case validator.
    pub initial_width: f32,
}

// The subset of letters which can be in each word. We probably will want a more advanced generation
// algorithm at some point, especially to handle punctuation's soft-wrap ability.
const VALID_CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

impl Case {
    /// Generate the [`Case`] for a given seed.
    pub fn from_seed(seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        // For some font sizes, Chrome has an order-dependent cache collision issue (for example,
        // font size 16.79 and 16.80 get the same key, so the first to be used defines the
        // size of the other).
        // No reasonable app will have font sizes that close to each other, so we
        // sample in such a way to miss these collisions.
        let font_size =
            MIN_FONT_SIZE + rng.random_range(0..=FONT_SIZE_STEPS) as f32 * FONT_SIZE_STEP;
        // However, in real world conditions, we do want to make sure that we handle font sizes
        // which don't neatly fall on that grid, so add an offset which still maintains the non-collision.
        let offset = rng.random_range(0_f32..0.0095_f32);
        let font_size = font_size + offset;
        let em_factor = rng.random_range(MIN_EM_FACTOR..=MAX_EM_FACTOR);
        let initial_width = font_size * em_factor;
        let word_count = rng.random_range(MIN_WORDS..=MAX_WORDS);

        let mut text = String::new();
        // TODO: Ideally, this generation would be more free-form, to e.g. test line breaking with punctuation.
        for word in 0..word_count {
            if word > 0 {
                text.push(' ');
            }
            let word_len = rng.random_range(MIN_WORD_LEN..=MAX_WORD_LEN);
            for _ in 0..word_len {
                let letter = *VALID_CHARS.choose(&mut rng).unwrap();
                text.push(char::from(letter));
            }
        }

        Self {
            seed,
            text,
            font_size,
            initial_width,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn well_formed() {
        for seed in 0..64 {
            let case = Case::from_seed(seed);
            assert!(case.text.is_ascii());
            assert!(!case.text.starts_with(' ') && !case.text.ends_with(' '));
            assert!(case.text.split(' ').all(|word| !word.is_empty()));
            assert!(
                case.text
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_uppercase() || c == ' '),
                "unexpected character in {:?}",
                case.text
            );
            let regenerated = Case::from_seed(seed);
            // Generation must be deterministic
            assert_eq!(case, regenerated);
        }
    }
}
