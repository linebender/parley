// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The inspection page (`/inspect`) of the in-browser line-breaking harness.
//!
//! Given a single seed (`?seed=N`), this renders each font's case the way Chrome lays it out (the
//! visible line wrap *is* Chrome's break), and overlays a marker showing where Parley would break.
//! The marker is absolutely positioned, so it never affects the rendered layout.
//!
//! By default each font renders at the tightened width Chrome broke at; `?width=S` overrides this
//! with an explicit width (in subpixels) for both fonts, re-measuring Chrome's break there.

use parley_linebreaking_browser::{query_param, set_status};
use parley_linebreaking_browser_harness::{collect, first_line_chars, load_fonts, make_measurer};
use parley_linebreaking_cases::{
    Case, FONTS, LayoutContext, PROBE_SUBPIXELS, SUBPIXELS_PER_PX, font_context, parley_first_line,
};
use wasm_bindgen::prelude::*;
use web_sys::{Document, Element, HtmlElement, HtmlInputElement, Node, Range};

pub(crate) async fn run() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;

    let search = window.location().search().unwrap_or_default();
    let seed: u64 = query_param(&search, "seed")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let width_override: Option<i64> = query_param(&search, "width")
        .and_then(|value| value.trim().parse().ok())
        .filter(|&w| w > 0);

    setup_seed_form(&document, seed, width_override)?;

    set_status(&document, &format!("Inspecting seed {seed}…"));
    load_fonts(&window).await?;

    let case = Case::from_seed(seed);
    let measurer = make_measurer(&document)?;
    let output = document
        .get_element_by_id("inspect-output")
        .ok_or("no #inspect-output")?;
    output.set_inner_html("");

    let mut layout_cx = LayoutContext::<()>::new();
    for font in FONTS {
        measurer.style().set_property("font-family", font.family)?;

        // With an explicit width we render at it (re-measuring Chrome's break there); otherwise we
        // render at the width Chrome broke at (its `+1/64px` epsilon), so the visible wrap matches
        // Chrome's recorded break. Overflow cases (no interior break) use the 1px probe.
        let (render_width, overflow, chrome_chars) = if let Some(width) = width_override {
            (
                width,
                false,
                first_line_chars(&case, &measurer, &document, width),
            )
        } else {
            let record = collect(&case, &measurer, &document);
            let overflow = record.tightened_width_subpixels == 0;
            let render_width = if overflow {
                PROBE_SUBPIXELS
            } else {
                record.tightened_width_subpixels + 1
            };
            (render_width, overflow, record.first_line_chars)
        };

        let mut font_cx = font_context(font);
        let parley_chars =
            parley_first_line(&mut font_cx, &mut layout_cx, font.family, &case, render_width);

        render_font_block(RenderBlock {
            document: &document,
            output: &output,
            family: font.family,
            case: &case,
            render_width,
            overflow,
            chrome_chars,
            parley_chars,
        })?;
    }
    measurer.remove();

    set_status(
        &document,
        &format!(
            "Seed {seed} — {:?}, {} chars: {:?}",
            case.strategy,
            case.text.len(),
            case.text
        ),
    );
    Ok(())
}

/// Inputs for rendering one font's inspection block.
struct RenderBlock<'a> {
    document: &'a Document,
    output: &'a Element,
    family: &'a str,
    case: &'a Case,
    render_width: i64,
    overflow: bool,
    chrome_chars: usize,
    parley_chars: usize,
}

/// Render the case for one font and overlay the Parley break marker.
fn render_font_block(block: RenderBlock<'_>) -> Result<(), JsValue> {
    let RenderBlock {
        document,
        output,
        family,
        case,
        render_width,
        overflow,
        chrome_chars,
        parley_chars,
    } = block;
    let width_px = render_width as f64 / SUBPIXELS_PER_PX;

    let container: HtmlElement = document.create_element("div")?.dyn_into()?;
    container.style().set_property("margin", "1rem 0")?;

    let heading: HtmlElement = document.create_element("h2")?.dyn_into()?;
    heading.set_text_content(Some(&format!(
        "{family} — {:.2}px, width {width_px:.2}px ({render_width} sp){}",
        case.font_size,
        if overflow { " (overflow, 1px probe)" } else { "" }
    )));
    container.append_child(&heading)?;

    let caption: HtmlElement = document.create_element("div")?.dyn_into()?;
    let matches = parley_chars == chrome_chars;
    caption.set_text_content(Some(&format!(
        "Chrome: {chrome_chars} chars (visible wrap) · Parley: {parley_chars} chars (red line) · {}",
        if matches { "match" } else { "MISMATCH" }
    )));
    let caption_style = caption.style();
    caption_style.set_property("color", if matches { "#2a8a2a" } else { "#c00" })?;
    caption_style.set_property("font", "13px ui-monospace, monospace")?;
    caption_style.set_property("margin", "0.2rem 0")?;
    container.append_child(&caption)?;

    // The visible render element. We use the same line-breaking properties and font size as the
    // measurer, so the wrap matches Chrome's recorded break. `position: relative` anchors the
    // absolutely-positioned marker; we use `outline` (not `border`) so the box origin still lines
    // up with the Range coordinates.
    let render: HtmlElement = document.create_element("div")?.dyn_into()?;
    let render_style = render.style();
    render_style.set_property("font-family", family)?;
    render_style.set_property("font-size", &format!("{}px", case.font_size))?;
    render_style.set_property("width", &format!("{width_px}px"))?;
    render_style.set_property("white-space", "normal")?;
    render_style.set_property("word-break", "normal")?;
    render_style.set_property("overflow-wrap", "normal")?;
    render_style.set_property("line-height", "1.9")?;
    render_style.set_property("position", "relative")?;
    render_style.set_property("outline", "1px solid #ccc")?;
    render_style.set_property("background", "#fafafa")?;
    render.set_text_content(Some(&case.text));
    container.append_child(&render)?;
    output.append_child(&container)?;

    // Now that the text is laid out, place the marker at Parley's break boundary.
    let text_node = render.first_child().ok_or("render element has no text node")?;
    let range = document.create_range()?;
    if let Some((left, top, height)) =
        break_boundary(&range, &text_node, parley_chars, case.text.len())
    {
        let render_rect = render.get_bounding_client_rect();
        let marker: HtmlElement = document.create_element("div")?.dyn_into()?;
        let marker_style = marker.style();
        marker_style.set_property("position", "absolute")?;
        marker_style.set_property("left", &format!("{}px", left - render_rect.left()))?;
        marker_style.set_property("top", &format!("{}px", top - render_rect.top()))?;
        marker_style.set_property("width", "2px")?;
        marker_style.set_property("height", &format!("{height}px"))?;
        marker_style.set_property("background", "#c00")?;
        marker_style.set_property("pointer-events", "none")?;
        render.append_child(&marker)?;
    }
    Ok(())
}

/// Viewport `(left, top, height)` of the boundary after the first `parley_chars` characters — i.e.
/// where Parley's first line ends. `None` for empty text.
#[expect(
    clippy::cast_possible_truncation,
    reason = "text lengths are tiny, far inside u32"
)]
fn break_boundary(range: &Range, node: &Node, parley_chars: usize, len: usize) -> Option<(f64, f64, f64)> {
    if len == 0 {
        return None;
    }
    if parley_chars == 0 {
        // Before the first character: use its left edge.
        range.set_start(node, 0).ok()?;
        range.set_end(node, 1).ok()?;
        let rect = range.get_bounding_client_rect();
        return Some((rect.left(), rect.top(), rect.height()));
    }
    // After the last character Parley keeps: use its right edge.
    let index = parley_chars.min(len) as u32;
    range.set_start(node, index - 1).ok()?;
    range.set_end(node, index).ok()?;
    let rect = range.get_bounding_client_rect();
    Some((rect.right(), rect.top(), rect.height()))
}

/// Populate the seed/width inputs and wire the "Go" button to reload at the chosen seed (and
/// optional explicit width).
fn setup_seed_form(
    document: &Document,
    seed: u64,
    width_override: Option<i64>,
) -> Result<(), JsValue> {
    if let Some(input) = document.get_element_by_id("inspect-seed") {
        input
            .dyn_into::<HtmlInputElement>()?
            .set_value(&seed.to_string());
    }
    if let Some(input) = document.get_element_by_id("inspect-width") {
        input
            .dyn_into::<HtmlInputElement>()?
            .set_value(&width_override.map(|w| w.to_string()).unwrap_or_default());
    }
    if let Some(go) = document.get_element_by_id("inspect-go") {
        let go: HtmlElement = go.dyn_into()?;
        let on_click = Closure::<dyn FnMut()>::new(move || {
            if let Some(window) = web_sys::window() {
                if let Some(document) = window.document() {
                    let seed = input_value(&document, "inspect-seed");
                    let width = input_value(&document, "inspect-width");
                    let mut query = format!("?seed={}", seed.trim());
                    if !width.trim().is_empty() {
                        query.push_str(&format!("&width={}", width.trim()));
                    }
                    let _ = window.location().set_search(&query);
                }
            }
        });
        go.add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref())?;
        on_click.forget();
    }
    Ok(())
}

/// The current value of the input with the given id, or empty if it isn't an input.
fn input_value(document: &Document, id: &str) -> String {
    document
        .get_element_by_id(id)
        .and_then(|element| element.dyn_into::<HtmlInputElement>().ok())
        .map(|input| input.value())
        .unwrap_or_default()
}
