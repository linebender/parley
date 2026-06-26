// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! In-browser Parley/Chrome line-breaking harness.
//!
//! This is a single wasm app serving several pages, dispatched by URL path:
//! * `/recorder` — record every seed's Chrome break decision to a CSV corpus.
//! * `/fuzzer` — sweep seeds, comparing Parley against Chrome live, persisting only failures.
//! * `/inspect` — render one seed's Chrome layout and overlay where Parley breaks.
//! * anything else — a landing page linking to them.
//!
//! There is one `index.html`; Trunk's dev server serves it for every path (SPA fallback), and the
//! page reveals the relevant section and runs the matching driver based on `location.pathname`.
//!
//! Build/run with [Trunk](https://trunkrs.dev):
//!
//! ```sh
//! rustup target add wasm32-unknown-unknown
//! cargo install --locked trunk
//! cd parley_tests/linebreaking_browser
//! trunk serve
//! ```
//!
//! Then open `/recorder` or `/fuzzer` in Chrome (or another Chromium based browser).

mod fuzzer;
mod inspect;
mod recorder;

use wasm_bindgen::prelude::*;
use web_sys::{Document, HtmlElement};

fn main() {
    console_error_panic_hook::set_once();
    wasm_bindgen_futures::spawn_local(async {
        if let Err(err) = dispatch().await {
            web_sys::console::error_1(&err);
        }
    });
}

/// Reveal the page matching the URL path and run its driver.
async fn dispatch() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;

    let path = window.location().pathname().unwrap_or_default();
    // The final path segment, with any trailing slash and `.html` suffix removed, so `/recorder`,
    // `/recorder/` and `/recorder.html` all route to the recorder.
    let route = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    let route = route.strip_suffix(".html").unwrap_or(route);

    match route {
        "recorder" => {
            reveal(&document, "recorder-page")?;
            recorder::run().await
        }
        "fuzzer" => {
            reveal(&document, "fuzzer-page")?;
            fuzzer::run().await
        }
        "inspect" => {
            reveal(&document, "inspect-page")?;
            inspect::run().await
        }
        _ => {
            reveal(&document, "landing-page")?;
            Ok(())
        }
    }
}

/// Make the section with the given id visible (sections start hidden).
fn reveal(document: &Document, id: &str) -> Result<(), JsValue> {
    if let Some(element) = document.get_element_by_id(id) {
        let element: HtmlElement = element.dyn_into()?;
        element.style().set_property("display", "block")?;
    }
    Ok(())
}
