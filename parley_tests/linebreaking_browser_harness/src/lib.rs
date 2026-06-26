// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared in-browser Chrome line-breaking measurement.
//!
//! Both harness pages in `parley_tests/linebreaking_browser` (the recorder and the fuzzer) need
//! to ask Chrome where it first breaks a given [`Case`]. That measurement — laying the text out in
//! an off-screen element and binary-searching the break position and the tightest width that
//! preserves it — lives here so it isn't duplicated.
//!
//! The entry points are [`load_fonts`] (register the embedded fonts), [`make_measurer`] (create
//! the off-screen element), and [`collect`] (measure one case into a [`Record`]).

use js_sys::Uint8Array;
use parley_linebreaking_cases::{Case, FONTS, PROBE_SUBPIXELS, SUBPIXELS_PER_PX};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, FontFace, HtmlElement, Node, Range, Window};

/// A fixed line height gives stable, well-separated per-line `y` positions,
/// which is what [`first_line_break`] reads to tell lines apart.
const LINE_HEIGHT: f64 = 50.0;

/// The measured first-line break decision for a single [`Case`].
#[derive(Clone, Copy, Debug)]
pub struct Record {
    /// The seed the measured [`Case`] was generated from.
    pub seed: u64,
    /// The tightest width that still keeps the first line intact, as an
    /// integer number of subpixels ([`SUBPIXELS_PER_PX`]).
    ///
    /// A value of `0` is a sentinel meaning the first line had no interior break
    /// opportunity. In that case, we just validate that Parley chooses to break
    /// at the same character. See [`PROBE_SUBPIXELS`] for more details.
    pub tightened_width_subpixels: i64,
    /// Characters on the first line. We use this to confirm
    /// that the same place in the line was used by Parley to break.
    pub first_line_chars: usize,
}

/// Collect the first-line break decision for a single case.
///
/// `measurer` must already have its `font-family` set to the family being measured (see
/// [`make_measurer`]); this sets the `font-size` and `width` itself.
pub fn collect(case: &Case, measurer: &HtmlElement, document: &Document) -> Record {
    let text = &case.text;
    // We floor here arbitrarily; we just need some mapping from the initial width to the subpixel width.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "Reasonable initial width values and floor means this will never truncate.."
    )]
    let initial_width_subpixels = (f64::from(case.initial_width) * SUBPIXELS_PER_PX).floor() as i64;

    measurer
        .style()
        .set_property("font-size", &format!("{}px", case.font_size))
        .unwrap();

    measurer.set_text_content(Some(text));
    let text_node = measurer.first_child().unwrap();
    assert_eq!(
        text_node.node_type(),
        Node::TEXT_NODE,
        "Measurer child node must be a text node for the Range API to work as we expect."
    );
    let range = document.create_range().unwrap();
    assert!(
        text.is_ascii(),
        "This code is not written to be robust against non-ASCII text, due to the DOM's use of UTF-16."
    );
    let len = text.len();

    // The index in the text at which the first line breaks, at the initial width.
    let break_at = first_line_break(measurer, &range, &text_node, len, initial_width_subpixels);
    // The index of the first break opportunity in the line.
    let first_break = first_line_break(measurer, &range, &text_node, len, PROBE_SUBPIXELS);

    debug_assert!(
        first_break <= break_at,
        "Making text narrower can't move break to later in the text."
    );
    let tightened_width_subpixels = if first_break == break_at {
        0
    } else {
        // Shrink the width to the minimum that still breaks here.
        tighten_width_subpixels(
            measurer,
            &range,
            &text_node,
            len,
            break_at,
            initial_width_subpixels,
        )
    };

    Record {
        seed: case.seed,
        tightened_width_subpixels,
        first_line_chars: break_at,
    }
}

/// The number of characters Chrome places on the first line of `case` at the given width.
///
/// Like [`collect`], `measurer` must already have its `font-family` set (see [`make_measurer`]);
/// this sets the `font-size`, text, and `width` itself. Useful for inspecting a chosen width
/// rather than the tightened one [`collect`] finds.
pub fn first_line_chars(
    case: &Case,
    measurer: &HtmlElement,
    document: &Document,
    width_subpixels: i64,
) -> usize {
    measurer
        .style()
        .set_property("font-size", &format!("{}px", case.font_size))
        .unwrap();
    measurer.set_text_content(Some(&case.text));
    let text_node = measurer.first_child().unwrap();
    assert_eq!(
        text_node.node_type(),
        Node::TEXT_NODE,
        "Measurer child node must be a text node for the Range API to work as we expect."
    );
    assert!(
        case.text.is_ascii(),
        "This code is not written to be robust against non-ASCII text, due to the DOM's use of UTF-16."
    );
    let range = document.create_range().unwrap();
    first_line_break(measurer, &range, &text_node, case.text.len(), width_subpixels)
}

/// Use `range` to find the y-position of the character at the given UTF-16 `index` within the `node`.
///
/// `node` must be a text node.
#[expect(
    clippy::cast_possible_truncation,
    reason = "text lengths are tiny, far inside u32"
)]
fn char_top(range: &Range, node: &Node, index: usize) -> f64 {
    range.set_start(node, index as u32).unwrap();
    range.set_end(node, index as u32 + 1).unwrap();
    range.get_bounding_client_rect().top()
}

/// The length of the text on the first line. A return value of `len` means that all the text was on the first line.
fn first_line_break(
    measurer: &HtmlElement,
    range: &Range,
    node: &Node,
    len: usize,
    width_subpixels: i64,
) -> usize {
    measurer
        .style()
        .set_property("width", &format!("{}px", width_px(width_subpixels)))
        .unwrap();

    let first_top = char_top(range, node, 0);
    if char_top(range, node, len - 1) <= first_top {
        return len;
    }
    // Invariant: the char at `lo` is on line one, the char at `hi` is later.
    let mut lo = 0;
    let mut hi = len - 1;
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if char_top(range, node, mid) > first_top {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi
}

/// Find the smallest width, in integer subpixels, where the first line still breaks
/// at `target_break`.
///
/// We know that there will be such a value because shrinking the width won't allow more
/// content onto the line with the greedy algorithm we're trying to match.
fn tighten_width_subpixels(
    measurer: &HtmlElement,
    range: &Range,
    node: &Node,
    len: usize,
    target_break: usize,
    hi_subpixels: i64,
) -> i64 {
    // Invariant: the break is before `target_break` at `lo`, at it at `hi`.
    let mut lo = PROBE_SUBPIXELS;
    let mut hi = hi_subpixels;
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        let preserved = first_line_break(measurer, range, node, len, mid) >= target_break;
        if preserved {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi
}

/// Convert a width in integer subpixels back to CSS pixels.
const fn width_px(width_subpixels: i64) -> f64 {
    width_subpixels as f64 / SUBPIXELS_PER_PX
}

/// Register and load every supported font, awaiting their readiness.
pub async fn load_fonts(window: &Window) -> Result<(), JsValue> {
    let fonts = window.document().ok_or("no document")?.fonts();
    for font in FONTS {
        let bytes = Uint8Array::from(font.bytes);
        let face = FontFace::new_with_array_buffer(font.family, &bytes.buffer())?;
        // We could parallelise this, but with only two fonts it doesn't have much advantage.
        let loaded = JsFuture::from(face.load()?).await?;
        let face: FontFace = loaded.dyn_into()?;
        fonts.add(&face)?;
    }
    Ok(())
}

/// Create the off-screen element we measure text in.
pub fn make_measurer(document: &Document) -> Result<HtmlElement, JsValue> {
    let el: HtmlElement = document.create_element("div")?.dyn_into()?;
    let style = el.style();
    // Avoid rendering the element, to speed up processing.
    style.set_property("position", "absolute")?;
    style.set_property("left", "-99999px")?;
    style.set_property("top", "0")?;
    style.set_property("visibility", "hidden")?;
    // Ensure that our the values we read are only due to the text.
    style.set_property("margin", "0")?;
    style.set_property("padding", "0")?;
    style.set_property("border", "0")?;
    // Match Parley's line breaking properties.
    style.set_property("white-space", "normal")?;
    style.set_property("word-break", "normal")?;
    style.set_property("overflow-wrap", "normal")?;
    style.set_property("line-height", &format!("{LINE_HEIGHT}px"))?;
    document.body().ok_or("no body")?.append_child(&el)?;
    Ok(el)
}
