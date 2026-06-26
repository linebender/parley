// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The fuzzer page (`/fuzzer`) of the in-browser line-breaking harness.
//!
//! Unlike the recorder (which stores every seed's result to a CSV corpus), the fuzzer runs the
//! comparison *live*: for each seed it measures Chrome's break (via the shared harness) and lays
//! the same case out in Parley (via `compare_case`), then **discards the seed immediately when
//! they match**. Only failing seeds are persisted (to `localStorage`), so a single long-running
//! tab can sweep huge seed ranges cheaply.
//!
//! Query parameters:
//! * `?start=S` — begin at seed `S` (otherwise resume from the stored cursor, else 0).
//! * `?count=N` — process `N` seeds then stop (otherwise run until stopped/closed).
//! * `?reset=1` — clear stored failures and cursor before starting.
//!
//! Failing seeds are the deliverable: use **Copy seeds** and paste them into a native repro.

use std::cell::Cell;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::rc::Rc;

use parley_linebreaking_browser::{query_param, set_status, yield_now};
use parley_linebreaking_browser_harness::{collect, load_fonts, make_measurer};
use parley_linebreaking_cases::{
    Case, FONTS, FontContext, LayoutContext, Outcome, PROBE_SUBPIXELS, compare_case, font_context,
    parley_first_line_advance_subpixels,
};
use wasm_bindgen::prelude::*;
use web_sys::{Document, Element, HtmlElement, Storage};

/// Seeds processed between yields to the browser / cursor writes.
const CHUNK: u64 = 64;
/// `localStorage` key holding the persisted failing seeds (one CSV-ish line each).
const FAILURES_KEY: &str = "parley-linebreak-fuzz/failures";
/// `localStorage` key holding the next seed to process, so a reload resumes the sweep.
const CURSOR_KEY: &str = "parley-linebreak-fuzz/cursor";

/// A single failing (seed, font) pair.
#[derive(Clone, Copy)]
struct Failure {
    seed: u64,
    font: &'static str,
    /// Characters Chrome placed on the first line.
    chrome_chars: usize,
    /// Characters Parley placed on the first line (equal to `chrome_chars` for a pure
    /// breaks-too-late failure, where the mismatch is only visible at a narrower width).
    parley_chars: usize,
    /// Whether Parley kept Chrome's full first line at a strictly narrower width.
    breaks_too_late: bool,
    /// Chrome's tightened width in subpixels (the `0` overflow sentinel for cases with no
    /// interior break opportunity). This is `Record::tightened_width_subpixels`.
    chrome_width_subpixels: i64,
    /// The trimmed width of Parley's first line in subpixels, measured at the same probe width the
    /// comparison used. Together with `chrome_width_subpixels` this shows the sub-pixel gap behind
    /// the failure.
    parley_advance_subpixels: f64,
}

/// Run the fuzzer page. The DOM (`#status`, `#controls` buttons, `#failures`) is provided by
/// `index.html`.
pub(crate) async fn run() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let storage = window.local_storage()?.ok_or("no localStorage")?;

    let search = window.location().search().unwrap_or_default();
    if query_param(&search, "reset").is_some() {
        storage.remove_item(FAILURES_KEY)?;
        storage.remove_item(CURSOR_KEY)?;
    }
    let start_param: Option<u64> = query_param(&search, "start").and_then(|s| s.parse().ok());
    let count: Option<u64> = query_param(&search, "count").and_then(|s| s.parse().ok());

    let stored_cursor: Option<u64> = storage.get_item(CURSOR_KEY)?.and_then(|s| s.parse().ok());
    let mut seed = start_param.or(stored_cursor).unwrap_or(0);

    let mut failures = load_failures(&storage)?;

    let running = Rc::new(Cell::new(true));
    setup_buttons(&document, &running)?;

    let failures_list = document
        .get_element_by_id("failures")
        .ok_or("no #failures")?;
    for failure in &failures {
        append_failure_row(&document, &failures_list, failure)?;
    }

    set_status(&document, "Loading fonts…");
    load_fonts(&window).await?;
    let measurer = make_measurer(&document)?;

    let mut layout_cx = LayoutContext::<()>::new();
    let mut font_contexts: HashMap<&'static str, FontContext> = HashMap::new();
    for font in FONTS {
        font_contexts.insert(font.family, font_context(font));
    }

    let mut processed = 0_u64;
    let mut residuals = 0_u64;

    loop {
        if count.is_some_and(|c| processed >= c) || !running.get() {
            break;
        }

        let case = Case::from_seed(seed);
        for font in FONTS {
            measurer.style().set_property("font-family", font.family)?;
            let record = collect(&case, &measurer, &document);

            let font_cx = font_contexts.get_mut(font.family).expect("font context");
            let comparison = compare_case(
                font_cx,
                &mut layout_cx,
                font.family,
                &case,
                record.tightened_width_subpixels,
                record.first_line_chars,
            );

            if matches!(comparison.outcome, Outcome::Residual) {
                residuals += 1;
            }
            let mismatch = matches!(comparison.outcome, Outcome::Mismatch { .. });
            if comparison.breaks_too_late || mismatch {
                let parley_chars = match comparison.outcome {
                    Outcome::Mismatch { parley_chars } => parley_chars,
                    Outcome::Match | Outcome::Residual => record.first_line_chars,
                };
                // Measure Parley's first-line width at the same probe the comparison used: the
                // `+1` epsilon width for the normal break path, or the overflow probe otherwise.
                let probe_width = if record.tightened_width_subpixels == 0 {
                    PROBE_SUBPIXELS
                } else {
                    record.tightened_width_subpixels + 1
                };
                let parley_advance_subpixels = parley_first_line_advance_subpixels(
                    font_cx,
                    &mut layout_cx,
                    font.family,
                    &case,
                    probe_width,
                );
                let failure = Failure {
                    seed,
                    font: font.family,
                    chrome_chars: record.first_line_chars,
                    parley_chars,
                    breaks_too_late: comparison.breaks_too_late,
                    chrome_width_subpixels: record.tightened_width_subpixels,
                    parley_advance_subpixels,
                };
                failures.push(failure);
                persist_failures(&storage, &failures)?;
                append_failure_row(&document, &failures_list, &failure)?;
            }
        }

        seed += 1;
        processed += 1;

        if processed.is_multiple_of(CHUNK) {
            storage.set_item(CURSOR_KEY, &seed.to_string())?;
            set_status(
                &document,
                &status_text(processed, failures.len(), seed, residuals, count),
            );
            yield_now().await;
        }
    }

    storage.set_item(CURSOR_KEY, &seed.to_string())?;
    measurer.remove();
    let prefix = if running.get() { "Done" } else { "Stopped" };
    let resume = if running.get() {
        ""
    } else {
        " Reload to resume."
    };
    set_status(
        &document,
        &format!(
            "{prefix}. {}{resume}",
            status_text(processed, failures.len(), seed, residuals, count)
        ),
    );
    Ok(())
}

/// Build the `#status` line text.
///
/// The residual proportion is taken over all `(seed, font)` comparisons (one per font per
/// processed seed), since residuals are counted per comparison.
#[expect(
    clippy::cast_precision_loss,
    reason = "counts are far too small to lose precision as f64"
)]
fn status_text(
    processed: u64,
    failures: usize,
    next_seed: u64,
    residuals: u64,
    count: Option<u64>,
) -> String {
    let limit = count.map(|c| format!("/{c}")).unwrap_or_default();
    let comparisons = processed.saturating_mul(FONTS.len() as u64);
    let residual_pct = if comparisons == 0 {
        0.0
    } else {
        residuals as f64 / comparisons as f64 * 100.0
    };
    format!(
        "Processed {processed}{limit} seeds (next seed {next_seed}); {failures} failures, {residuals} residuals ({residual_pct:.1}%)."
    )
}

/// Parse the persisted failures from `localStorage`.
fn load_failures(storage: &Storage) -> Result<Vec<Failure>, JsValue> {
    let raw = storage.get_item(FAILURES_KEY)?.unwrap_or_default();
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() != 7 {
            continue;
        }
        // Map the family name back to its `&'static str` from `FONTS`; skip unknown families.
        let Some(font) = FONTS.iter().find(|f| f.family == cols[1]) else {
            continue;
        };
        out.push(Failure {
            seed: cols[0].parse().unwrap_or(0),
            font: font.family,
            chrome_chars: cols[2].parse().unwrap_or(0),
            parley_chars: cols[3].parse().unwrap_or(0),
            breaks_too_late: cols[4] == "1",
            chrome_width_subpixels: cols[5].parse().unwrap_or(0),
            parley_advance_subpixels: cols[6].parse().unwrap_or(0.0),
        });
    }
    Ok(out)
}

/// Write the full failures list back to `localStorage`.
fn persist_failures(storage: &Storage, failures: &[Failure]) -> Result<(), JsValue> {
    let mut serialized = String::new();
    for f in failures {
        writeln!(
            serialized,
            "{},{},{},{},{},{},{:.4}",
            f.seed,
            f.font,
            f.chrome_chars,
            f.parley_chars,
            u8::from(f.breaks_too_late),
            f.chrome_width_subpixels,
            f.parley_advance_subpixels,
        )
        .unwrap();
    }
    storage.set_item(FAILURES_KEY, &serialized)
}

/// Append a human-readable row for `failure` to the `#failures` list.
fn append_failure_row(
    document: &Document,
    list: &Element,
    failure: &Failure,
) -> Result<(), JsValue> {
    let row = document.create_element("div")?;
    let kind = if failure.breaks_too_late {
        "breaks too late"
    } else {
        "mismatch"
    };
    // Chrome's `0` tightened width is the overflow sentinel (no interior break opportunity).
    let widths = if failure.chrome_width_subpixels == 0 {
        format!(
            "Chrome overflow, Parley {:.2}sp",
            failure.parley_advance_subpixels
        )
    } else {
        format!(
            "Chrome {}sp, Parley {:.2}sp (Δ {:+.2}sp)",
            failure.chrome_width_subpixels,
            failure.parley_advance_subpixels,
            failure.parley_advance_subpixels - failure.chrome_width_subpixels as f64,
        )
    };
    row.set_text_content(Some(&format!(
        "seed {} [{}] {} — Chrome {} chars, Parley {} chars · {widths}",
        failure.seed, failure.font, kind, failure.chrome_chars, failure.parley_chars
    )));
    list.append_child(&row)?;
    Ok(())
}

/// Wire up the Stop / Clear / Copy seeds buttons.
fn setup_buttons(document: &Document, running: &Rc<Cell<bool>>) -> Result<(), JsValue> {
    if let Some(stop) = document.get_element_by_id("stop") {
        let stop: HtmlElement = stop.dyn_into()?;
        let running = running.clone();
        let on_click = Closure::<dyn FnMut()>::new(move || running.set(false));
        stop.add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref())?;
        on_click.forget();
    }

    if let Some(clear) = document.get_element_by_id("clear") {
        let clear: HtmlElement = clear.dyn_into()?;
        let on_click = Closure::<dyn FnMut()>::new(move || {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.remove_item(FAILURES_KEY);
                    let _ = storage.remove_item(CURSOR_KEY);
                }
                // Reload so the (now empty) state is re-read from scratch.
                let _ = window.location().reload();
            }
        });
        clear.add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref())?;
        on_click.forget();
    }

    if let Some(copy) = document.get_element_by_id("copy-seeds") {
        let copy: HtmlElement = copy.dyn_into()?;
        let on_click = Closure::<dyn FnMut()>::new(move || {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let raw = storage
                        .get_item(FAILURES_KEY)
                        .ok()
                        .flatten()
                        .unwrap_or_default();
                    let _ = window
                        .navigator()
                        .clipboard()
                        .write_text(&unique_seeds(&raw));
                }
            }
        });
        copy.add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref())?;
        on_click.forget();
    }
    Ok(())
}

/// Extract the unique seeds (in first-seen order) from the persisted failures, space-separated.
fn unique_seeds(raw: &str) -> String {
    let mut seeds: Vec<&str> = Vec::new();
    for line in raw.lines() {
        if let Some(seed) = line.split(',').next() {
            let seed = seed.trim();
            if !seed.is_empty() && !seeds.contains(&seed) {
                seeds.push(seed);
            }
        }
    }
    seeds.join(" ")
}
