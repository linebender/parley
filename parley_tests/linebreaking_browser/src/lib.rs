// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared UI helpers for the in-browser line-breaking harness pages.
//!
//! The recorder (`src/recorder.rs`) and fuzzer (`src/fuzzer.rs`) pages are dispatched from the
//! single binary (`src/main.rs`) by URL path. The Chrome measurement they share lives in
//! `parley_linebreaking_browser_harness`; the small DOM/UI helpers they share live here.

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, Element, HtmlElement, Window};

/// Look up a query-string parameter (e.g. `seed` in `?seed=5&count=10`).
pub fn query_param(search: &str, key: &str) -> Option<String> {
    let query = search.strip_prefix('?').unwrap_or(search);
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if parts.next() == Some(key) {
            return Some(parts.next().unwrap_or("").to_owned());
        }
    }
    None
}

/// Set the text of the `#status` element, if present.
pub fn set_status(document: &Document, message: &str) {
    if let Some(status) = document.get_element_by_id("status") {
        status.set_text_content(Some(message));
    }
}

/// The Chrome/Chromium version from the user-agent string (e.g. `"138.0.0.0"`),
/// falling back to the full user-agent if it is not Chrome-shaped.
pub fn chrome_version(window: &Window) -> String {
    let user_agent = window.navigator().user_agent().unwrap_or_default();
    match user_agent.split_once("Chrome/") {
        Some((_, rest)) => rest.split(' ').next().unwrap_or("").to_owned(),
        None => user_agent,
    }
}

/// Append a button to `parent` that copies `text` to the clipboard when clicked, flipping its
/// label to `copied_label` as feedback.
pub fn add_copy_button(
    document: &Document,
    parent: &Element,
    label: &str,
    copied_label: String,
    text: String,
) -> Result<(), JsValue> {
    let button: HtmlElement = document.create_element("button")?.dyn_into()?;
    button.set_text_content(Some(label));
    parent.append_child(&button)?;

    let button_for_handler = button.clone();
    let on_click = Closure::<dyn FnMut()>::new(move || {
        if let Some(window) = web_sys::window() {
            // The returned promise is fire-and-forget; we don't await it.
            let _ = window.navigator().clipboard().write_text(&text);
        }
        button_for_handler.set_text_content(Some(&copied_label));
    });
    button.add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref())?;
    // Keep the closure alive for the lifetime of the page.
    on_click.forget();
    Ok(())
}

/// Yield to other tasks in the browser. In particular, this allows painting/redrawing to happen, so that progress is visible.
/// Adapted from <https://github.com/wasm-bindgen/wasm-bindgen/discussions/3476#discussion-5283084>
pub async fn yield_now() {
    let mut cb = |resolve: js_sys::Function, _reject: js_sys::Function| {
        web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 0)
            .expect("Failed to call set_timeout");
    };
    let p = js_sys::Promise::new(&mut cb);
    JsFuture::from(p).await.unwrap();
}
