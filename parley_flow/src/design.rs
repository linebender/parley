// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Design notes, comparisons, and long-term considerations.
//!
//! Parley’s core (`parley`) is intentionally paragraph-focused and pure: it owns shaping,
//! line breaking, bidi, and alignment. This crate builds a higher-level layer for composing
//! many paragraphs/surfaces and for modeling multi-selection/navigation.
//!
//! ## Inspiration and Comparisons
//!
//! ### Apple TextKit 1/2
//!
//! - Layering: `NSTextStorage` (attributed storage) → `NSLayoutManager`/`NSTextLayoutManager`
//!   (shaping/line breaking) → `NSTextContainer` (geometry/regions). Storage can flow through
//!   multiple containers for columns/pages.
//! - Selection/navigation: First-class objects (`NSTextSelection`, `NSTextSelectionNavigation`)
//!   with anchor/focus, affinity, and granularity (character/word/line/paragraph).
//! - Attachments: `NSTextAttachment` uses object-replacement semantics, which maps to Parley
//!   inline boxes and helps serialization (U+FFFC placeholder).
//! - Takeaways for us:
//!   - Preserve a strict separation: storage vs. layout vs. region.
//!   - Model selection/navigation as reusable services that operate over regions/surfaces.
//!   - Treat inline objects as real selection units and serialization boundaries.
//!
//! ### Windows DirectWrite + TSF
//!
//! - DirectWrite: paragraph-centric layouts (`IDWriteTextLayout`) composed by host toolkits
//!   into documents. Justification, trimming, and typographic knobs exposed as explicit options.
//! - TSF (Text Services Framework): composition (IME) modeled as ranges; selection kept in sync;
//!   UTF‑16 code unit indexing as lingua franca.
//! - Takeaways:
//!   - Keep layout per-paragraph; composition/selection are ranges in storage.
//!   - Provide explicit UTF‑8↔UTF‑16 conversions at the edges for platform interoperability.
//!   - Don’t bake editing into layout; keep layout pure and reusable.
//!
//! ### Android (Spannable, MovementMethod, InputConnection)
//!
//! - Ranges: Spans with inclusive/exclusive flags survive edits; replacement spans for attachments.
//! - Navigation: MovementMethod separates navigation from widgets, enabling reuse across views.
//! - IME: InputConnection is an explicit bridge for composition, commit, and selection updates.
//! - Precompute: Precomputed/Measured text validates doing shaping/measurement off the UI thread
//!   and reusing results (like `LayoutContext`).
//! - Cautions: span proliferation and watchers can become hot paths; cross-widget selection is
//!   not first-class.
//! - Takeaways:
//!   - Favor compact range tables over callback-heavy span objects.
//!   - Keep navigation and IME bridges separate, explicit, and host-driven.
//!   - Make async/precomputed layout a supported pattern via caches + generations.
//!
//! ### WPF/Web (brief)
//!
//! - WPF: `TextContainer`/`TextPointer`/`TextSelection` abstractions, flow across regions.
//! - Web: `Range` across nodes, selection APIs that treat node boundaries as hard breaks; rich
//!   serialization policies matter.
//! - Takeaways:
//!   - Encapsulate positions as abstract pointers, not raw indices.
//!   - Provide serialization policies (logical/visual order, boundary separators).
//!
//! ## Key Choices for a 10+ Year Horizon
//!
//! - Keep editor/navigation policies out of `Layout`; implement them in this crate.
//! - Favor explicit types for locations/ranges across surfaces, with conversions to platform units.
//! - Treat surface boundaries as hard boundaries for movement/word/line granularity.
//! - Provide read-only aggregation (copy/search/AX) across surfaces by default; editing across
//!   multiple surfaces is opt-in and typically limited to a single active caret.
//!
//! ## Future Work
//!
//! ### Surface Flow and Ordering
//!
//! Parley surfaces are intentionally minimal. Explicit flow is modeled by `flow::TextFlow`:
//! - Each `FlowItem` encodes a `block_id`, a `rect` (for hit-testing/geometry), and a `join`
//!   policy for serialization.
//! - Order in the `TextFlow` defines cross-block navigation and concatenation semantics.
//! - This mirrors TextKit’s ordered container array.
//!
//! Guidance:
//! - Provide non-overlapping `rect`s in the flow for deterministic hit-testing.
//! - Use large widths/heights when you don’t care about precise bounds; the flow determines order.
//! - For multi-column/page or virtualized layouts, build the `TextFlow` with the intended reading
//!   order and rects for each visible block.
//! - Concatenation: use a uniform `join` via [`crate::flow::TextFlow::from_vertical_stack`] or assign
//!   per-item `join` policies (e.g., `Space` for inline, `Newline` for block boundaries).
//!
//! ### 1) TextLocation and TextRange (positions across surfaces)
//!
//! - Purpose: decouple pointer/range representations from raw indices; enable safe conversions to
//!   platform units and stable identity across edits.
//! - Shape:
//!   - `TextLocation { surface_id, utf8: usize }` with helpers: `to_utf16()`, `from_utf16()`.
//!   - `TextRange { start: TextLocation, end: TextLocation }`, normalized with methods for
//!     granularity expansion (to cluster/word/line/paragraph) using the surface’s layout.
//! - Invariants:
//!   - Always at character boundaries in UTF‑8; conversion to UTF‑16 is lossy only in units, not
//!     in meaning.
//!   - Stable `surface_id` and monotonic ordering within a surface.
//! - Interop:
//!   - Map to TSF/AppKit APIs that require UTF‑16 indices; provide zero‑allocation conversions.
//!   - Serialize as absolute (surface_id, byte_offset) to avoid ambiguity when text changes.
//! - Integration:
//!   - Backed by the same hit-testing code as current `Cursor`/`Selection`.
//!   - Acts as the wire type for accessibility and IME bridges.
//!
//! ### 2) SelectionNavigation (surface‑crossing caret movement)
//!
//! - Purpose: unify navigation semantics (move/extend by cluster/word/line/paragraph) across
//!   multiple surfaces in visual order, mirroring Apple’s `NSTextSelectionNavigation`.
//! - Current scaffold:
//!   - Implemented in `crate::selection_navigation` with `move_left`/`move_right` that cross
//!     surface boundaries by jumping to the end/start of adjacent surfaces.
//!   - Inside a surface, movement delegates to Parley’s `Cursor` logic.
//!   - Tests cover crossing from the end of one surface to the start of the next, and vice versa.
//! - Next steps:
//!   - Vertical movement preserving `h_pos`: `move_line_up`/`move_line_down`.
//!   - Word/paragraph granularity: `move_word_left/right`, `hard_line_start/end`.
//!   - Extend variants (Shift-modify): introduce an anchor caret in `SelectionSet` or pass a
//!     transient anchor so movement can grow/shrink the nearest segment or add cross-surface
//!     segments.
//!   - Surface ordering predicates: allow clients to supply ordering beyond y-offset (e.g., columns).
//! - Semantics to retain:
//!   - Affinity and bidi: respect `Cursor` affinity at line ends; treat surface edges as hard
//!     boundaries; never span a cluster across surfaces.
//!   - Vertical movement: maintain a sticky `h_pos` in global coordinates; when moving across
//!     surfaces, compute target y in the next surface via its `y_offset` and line metrics.
//!   - Word/line boundaries: use per-surface rules; when crossing a surface, land at the first/last
//!     selectable boundary inside the target surface.
//! - Testing:
//!   - Golden tests for bidi line ends, RTL/LTR boundaries, mixed metrics; property tests for
//!     inverse moves where appropriate.
//!
//! ### 3) Document Storage and Incremental Relayout
//!
//! - Purpose: provide a concrete “document” built from paragraphs with efficient editing,
//!   invalidation, and relayout, while still exposing each paragraph as a `TextBlock`.
//! - Storage:
//!   - Use a rope or gap buffer for large texts; maintain paragraph boundaries as a side table of
//!     byte ranges with stable paragraph IDs.
//!   - Inclusive/exclusive range flags for styles, composition, and annotations that survive edits.
//! - Invalidation:
//!   - When editing, detect impacted paragraphs (splits/merges on newlines) and only rebuild
//!     affected layouts; reuse shaped results when attributes allow.
//!   - Employ generations to coordinate caches (fonts, shaping, line breaking) and async rebuilds.
//! - Layout:
//!   - Build each paragraph’s `Layout` via `LayoutContext`; cache glyph/cluster data.
//!   - Support async layout: produce placeholder metrics/geometry, swap in final results when ready.
//! - Regions:
//!   - Flow paragraphs into regions (columns/pages) by assigning `y_offset`s; each paragraph is a
//!     `TextBlock` with its own layout and text slice.
//! - Attachments:
//!   - Represent as inline boxes using object‑replacement semantics (U+FFFC) for selection and
//!     serialization; geometry carried by the inline box.
//!
//! This module contains only documentation.
