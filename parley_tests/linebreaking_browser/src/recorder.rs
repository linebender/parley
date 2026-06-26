// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The recorder page (`/recorder`) of the in-browser line-breaking harness.
//!
//! For each test case (see [`parley_linebreaking_cases`]), we find where the browser first breaks,
//! given a random width. We then find and record the minimum width which also causes the same break.
//! A future extension will be to evaluate with a more reasonable bound across multiple lines of text;
//! this test is designed to catch the edge cases for that test.
//!
//! The data is persisted manually, by copying it into the relevant files, using the relevant
//! `Copy <font> Data` buttons. These should go into the appropriately named files in
//! `parley_tests/linebreaking_browser/data`.

use parley_linebreaking_browser::{add_copy_button, chrome_version, set_status, yield_now};
use parley_linebreaking_browser_harness::{Record, collect, load_fonts, make_measurer};
use parley_linebreaking_cases::{Case, FONTS};
use std::fmt::Write as _;
use wasm_bindgen::prelude::*;

/// Number of seeds to collect by default (overridable with `?count=N`).
const DEFAULT_SEED_COUNT: u64 = 1024;

/// Run the recorder page. The DOM (`#status`, `#results`) is provided by `index.html`.
pub(crate) async fn run() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;

    set_status(&document, "Loading fonts…");
    load_fonts(&window).await?;

    let measurer = make_measurer(&document)?;
    let results = document.get_element_by_id("results").ok_or("no #results")?;

    let version = chrome_version(&window);

    for font in FONTS {
        measurer.style().set_property("font-family", font.family)?;

        let mut records = Vec::new();
        for seed in 0..DEFAULT_SEED_COUNT {
            if seed % 512 == 0 {
                set_status(
                    &document,
                    &format!(
                        "Collecting {} - {}/{}",
                        font.family, seed, DEFAULT_SEED_COUNT
                    ),
                );
                yield_now().await;
            }
            records.push(collect(&Case::from_seed(seed), &measurer, &document));
        }

        let csv = output_csv(font.family, &version, &records);
        add_copy_button(
            &document,
            &results,
            &format!("Copy {} Data", font.family),
            format!("Copied {} ✓", font.family),
            csv,
        )?;
    }

    measurer.remove();
    set_status(
        &document,
        &format!(
            "Done — {} font(s); use the per-font buttons to get the data. This can then be copied into the relevant data files.",
            FONTS.len()
        ),
    );
    Ok(())
}

/// Serialise the collected data as CSV.
///
/// We also store some metadata as frontmatter in comments indicated by '#' - that isn't standard, but
/// we make sure that our parser ignores it.
fn output_csv(font_family: &str, chrome_version: &str, records: &[Record]) -> String {
    let mut out = format!(
        "# font_family: {font_family}
# chrome_version: {chrome_version}
seed,width_subpixels,first_line_chars\n"
    );
    for r in records {
        writeln!(
            out,
            "{},{},{}",
            r.seed, r.tightened_width_subpixels, r.first_line_chars
        )
        .unwrap();
    }
    out
}
