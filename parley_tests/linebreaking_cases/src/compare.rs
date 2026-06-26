// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared Parley/Chrome line-breaking comparator.
//!
//! This is the comparison logic that decides, for a single [`Case`], whether Parley makes the
//! same first-line break decision as Chrome did when the expectation was recorded. It lives here
//! — behind the `compare` feature — so it can be shared by two consumers that can't depend on each
//! other:
//!
//! * the native regression test (`parley_tests/tests/linebreaking_matches_chrome.rs`), which
//!   pulls in heavy native-only dependencies and can't build for wasm; and
//! * the in-browser fuzzer (`parley_tests/linebreaking_browser`), which is wasm-only.
//!
//! Both build the [`FontContext`] the same way via [`font_context`] and classify each case with
//! [`compare_case`], so their decisions are guaranteed identical.

use std::sync::Arc;

use fontique::{Blob, Collection, CollectionOptions, SourceCache};
use parley::{CHROMIUM_LINE_BREAK_OVERRIDE, FontFamily, Layout, StyleProperty};
// Re-exported so consumers (e.g. the browser fuzzer) can name these without a direct `parley`
// dependency or feature-matching it themselves.
pub use parley::{FontContext, LayoutContext};

use crate::{Case, PROBE_SUBPIXELS, SUBPIXELS_PER_PX, SupportedFont};

/// If the matching with exactly Chrome's boundary fails, we try again with a very small
/// additional margin, to ensure that the cause is the accumulation unit mismatch (as
/// chrome works in 16.16 fixed point pixels).
///
/// The risk of this increases as text gets longer, but the absolute maximum drift
/// per glyph is `2^-16`, so even an unreasonable 1000 character line (~2^10 chars) would
/// still be expected to only have a drift of about `2^-6`px, i.e. 1/64px.
/// Therefore, an extra margin of 1/64px will correctly detect this for reasonable
/// texts.
///
/// In real-world code which needs to match Chrome, we expect that adding this margin
/// unconditionally would be safe. The chance of a real case falling exactly within
/// that 1/64px mismatch is negligible.
///
/// We validate the number of cases that fall within this boundary; for our dataset, we
/// expect it to be less than 2%, as we have lines of an average of ~30 characters, so
/// the average drift per line is 2^-17 * 30, so the likelihood of that falling
/// in the gap is around (30 * 2^-17) * 64 (~1.5%).
pub const RESIDUAL_SLACK_SUBPIXELS: i64 = 1;

/// How a single case's Parley break compares to Chrome's recorded break, ignoring the
/// independent [`Comparison::breaks_too_late`] check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Parley broke at exactly the same character as Chrome.
    Match,
    /// Parley matched only after allowing [`RESIDUAL_SLACK_SUBPIXELS`] of extra width; this is
    /// the known 16.16-vs-f32 advance-width drift, not a bug.
    Residual,
    /// Parley broke at a different character than Chrome.
    Mismatch {
        /// The number of characters Parley placed on the first line.
        parley_chars: usize,
    },
}

/// The result of comparing one case against Chrome's recorded break.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Comparison {
    /// Parley failed to break where Chrome did at the exact boundary
    /// (the "doesn't break when Chrome would" check). This is reported independently of
    /// [`Self::outcome`]; both can indicate a failure for the same case.
    pub breaks_too_late: bool,
    /// How Parley's break compares to Chrome's at the slightly-wider boundary.
    pub outcome: Outcome,
}

/// Build the [`FontContext`] used to lay out cases for `font`.
///
/// System fonts are disabled and only `font`'s bytes are registered, so both the native test and
/// the browser fuzzer resolve `font.family` to exactly these bytes.
pub fn font_context(font: &SupportedFont) -> FontContext {
    let mut collection = Collection::new(CollectionOptions {
        shared: false,
        system_fonts: false,
    });
    collection.register_fonts(Blob::new(Arc::new(font.bytes.to_vec())), None);
    FontContext {
        collection,
        source_cache: SourceCache::default(),
    }
}

/// Compare Parley's first-line break for `case` against Chrome's recorded result.
///
/// `width_subpixels` is the tightened width Chrome broke at (a sentinel `0` means the first line
/// had no interior break opportunity — the overflow case), and `first_line_chars` is the number
/// of characters Chrome placed on the first line.
///
/// This mirrors the three-step probe the native test historically performed inline: an overflow
/// path at [`PROBE_SUBPIXELS`]; otherwise a `too_small`/`normal`/`slack` probe at `width`,
/// `width + 1`, and `width + 1 + RESIDUAL_SLACK_SUBPIXELS`.
pub fn compare_case(
    font_cx: &mut FontContext,
    layout_cx: &mut LayoutContext<()>,
    font_family: &str,
    case: &Case,
    width_subpixels: i64,
    first_line_chars: usize,
) -> Comparison {
    let mut layout = build_layout(font_cx, layout_cx, font_family, case);

    if width_subpixels == 0 {
        // Overflow case: Chrome couldn't break the first line within the initial width, so we
        // just check Parley breaks at the same character with a probe width.
        let chars = parley_first_line_chars(&mut layout, PROBE_SUBPIXELS);
        return Comparison {
            breaks_too_late: false,
            outcome: if chars == first_line_chars {
                Outcome::Match
            } else {
                Outcome::Mismatch {
                    parley_chars: chars,
                }
            },
        };
    }

    // We first try breaking with a *too small* width. We know that Chrome would break here,
    // but we want to check that Parley does too. If Parley still fits Chrome's full first line in
    // a strictly narrower width, it breaks too late.
    let too_small_chars = parley_first_line_chars(&mut layout, width_subpixels);
    let breaks_too_late = too_small_chars == first_line_chars;

    // Blink's line breaker fits each line against the LayoutUnit available width plus one
    // LayoutUnit epsilon of 1/64px.
    //
    // See `AvailableWidthToFit`
    // <https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/line_breaker.h;l=306-308;drc=e619a461ad0bb6c32c92dc22f6faf5f2ea42cb75>
    let normal_chars = parley_first_line_chars(&mut layout, width_subpixels + 1);
    let outcome = if normal_chars == first_line_chars {
        Outcome::Match
    } else {
        // This is (almost certainly) a mismatch due to Chromium's use of 16.16 advances, whereas
        // we use f32. See docs on [`RESIDUAL_SLACK_SUBPIXELS`] for more.
        let slack_chars =
            parley_first_line_chars(&mut layout, width_subpixels + 1 + RESIDUAL_SLACK_SUBPIXELS);
        if slack_chars == first_line_chars {
            Outcome::Residual
        } else {
            Outcome::Mismatch {
                parley_chars: normal_chars,
            }
        }
    };

    Comparison {
        breaks_too_late,
        outcome,
    }
}

/// Build the Parley [`Layout`] for `case` in `font_family`, matching how Chrome was configured
/// (Chromium line-break override and quantized font size).
fn build_layout(
    font_cx: &mut FontContext,
    layout_cx: &mut LayoutContext<()>,
    font_family: &str,
    case: &Case,
) -> Layout<()> {
    let mut builder = layout_cx.ranged_builder(font_cx, &case.text, 1.0, false);
    builder.set_line_break_override(Some(CHROMIUM_LINE_BREAK_OVERRIDE));
    builder.push_default(FontFamily::named(font_family));
    builder.push_default(StyleProperty::FontSize(chromium_quantized_font_size(
        case.font_size,
    )));
    builder.build(&case.text)
}

/// The number of characters Parley places on the first line of `case` (in `font_family`) when
/// broken at `width_subpixels`.
///
/// This is the raw measurement behind [`compare_case`], exposed for tooling (e.g. the inspection
/// page) that needs Parley's actual break position rather than a pass/fail classification.
pub fn parley_first_line(
    font_cx: &mut FontContext,
    layout_cx: &mut LayoutContext<()>,
    font_family: &str,
    case: &Case,
    width_subpixels: i64,
) -> usize {
    let mut layout = build_layout(font_cx, layout_cx, font_family, case);
    parley_first_line_chars(&mut layout, width_subpixels)
}

/// The trimmed width of Parley's first line for `case` (in `font_family`), in subpixels, when
/// broken at `width_subpixels`.
///
/// This is the line advance with trailing whitespace removed — i.e. the width Parley measures the
/// first line at and compares against the break constraint. It's the counterpart to Chrome's
/// recorded tightened width, so recording both lets a failing case be diagnosed (e.g. "Parley is
/// 1/64px narrower than Chrome") without re-deriving the measurement by hand.
pub fn parley_first_line_advance_subpixels(
    font_cx: &mut FontContext,
    layout_cx: &mut LayoutContext<()>,
    font_family: &str,
    case: &Case,
    width_subpixels: i64,
) -> f64 {
    let mut layout = build_layout(font_cx, layout_cx, font_family, case);
    let max_advance = width_subpixels as f64 / SUBPIXELS_PER_PX;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "width values are small; truncating the advance to f32 is acceptable for layout"
    )]
    layout.break_all_lines(Some(max_advance as f32));
    let first_line = layout.get(0).expect("layout has at least one line");
    let metrics = first_line.metrics();
    f64::from(metrics.advance - metrics.trailing_whitespace) * SUBPIXELS_PER_PX
}

/// The number of characters of the first line when `layout` is broken at `width_subpixels` (in subpixels).
#[expect(
    clippy::cast_possible_truncation,
    reason = "width values are small; truncating the advance to f32 is acceptable for layout"
)]
fn parley_first_line_chars(layout: &mut Layout<()>, width_subpixels: i64) -> usize {
    let max_advance = width_subpixels as f64 / SUBPIXELS_PER_PX;
    layout.break_all_lines(Some(max_advance as f32));

    let first_line = layout.get(0).expect("layout has at least one line");
    first_line.text_range().len()
}

/// Quantizes a font size the way Chromium does (LSR-804).
///
/// Blink's font cache is keyed on the font size truncated to 1/100px
/// (see `kFontSizePrecisionMultiplier` in Chromium). Matching this
/// truncation avoids our lines being wider than in the browser.
///
/// See <https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/font_description.cc;l=270-282;drc=e619a461ad0bb6c32c92dc22f6faf5f2ea42cb75>.
pub fn chromium_quantized_font_size(font_size: f32) -> f32 {
    (font_size * 100.0).floor() / 100.0
}
