# Plan: caller-provided line break opportunities in `parley_engine`

## Goal

`parley_engine` stops performing line and word segmentation. Callers supply
soft line break opportunities as input; word boundaries become something only
`parley` computes and stores. The engine keeps grapheme segmentation
(dictionary-free, needed internally for cluster formation) and mandatory-break
detection (a cheap Unicode property lookup, not policy).

Motivations, in order:

1. The segmentation dictionaries (`complex-scripts`: Thai, Khmer, Lao, Burmese)
   leave the engine's dependency tree. Hosts that load segmentation data
   separately, or use a platform segmenter such as the web's `Intl.Segmenter`,
   no longer link them twice. The dictionaries are shared between
   `LineSegmenter` and `WordSegmenter`, so both must leave the engine for this
   to land — moving only line segmentation achieves nothing.
2. Layering: the engine's contract becomes "break opportunities in, shaped text
   and line-break decisions out". Segmentation policy (word-break strength,
   browser-parity overrides) is a host concern. External engine users (Servo,
   Typst, Blink, ...) all have their own segmenters.

A future public extension point in `parley` for user-provided segmentation is a
separate task. This change only needs to leave an internal seam shaped so that
task can extract a trait from it, rather than design around it.

## Engine API changes (`parley_engine`)

- `AnalysisOptions` becomes `{ base_direction, line_break_opportunities: &[usize] }`.
  - `line_break_opportunities`: sorted UTF-8 byte offsets, each meaning "a soft
    break opportunity exists before the character starting at this offset".
    `usize` matches the existing byte-offset convention (`u32` is reserved for
    char indices in the shape API).
  - A slice, not an iterator: producers (ICU, `Intl.Segmenter`) materialize
    eagerly anyway, and the current code already collects into a `Vec`.
  - The `word_break` ranges field and `line_break_override` field are deleted.
- Validation: panic on unsorted offsets, offsets not on `char` boundaries, and
  offsets that fall mid-grapheme-cluster. The grapheme check is one branch in
  the existing merge loop (`is_grapheme_start` is computed there). No silent
  repair.
- `Boundary` shrinks to `{ None, Line, Mandatory }`. It continues to flow
  through `CharInfo` and `ClusterInfo` unchanged otherwise.
- Moved (not deleted) out of the engine, into `parley`: the `break_overrides`
  module (`LineBreakContext`, `AsciiLineBreakTable`,
  `CHROMIUM_LINE_BREAK_OVERRIDE`), the `DenseWordBreaks`/`WordBreakSegmentIter`
  substring plumbing in `analysis.rs`, the `LineSegmenter` and `WordSegmenter`
  construction, and the single-word-break-style fast path (a bookkeeping bypass
  of the substring machinery for the common no-`word-break`-spans case — it
  belongs wherever ICU runs, so it migrates verbatim).
- Kept: `GraphemeClusterSegmenter` (a slim, `const`, dictionary-free
  `icu_segmenter` dependency remains) and `Properties::is_mandatory_linebreak`.
- The `complex-scripts` feature moves from `parley_engine`'s Cargo.toml to
  `parley`'s.
- New/changed public items get documentation which does not refer to the prior state.

## `parley` changes

All evicted machinery lands in `parley` as `pub(crate)` code (suggested module:
`parley/src/segmentation.rs`). Public API and observable behavior are unchanged
in this task; the existing knobs (`word_break` style ranges,
`line_break_override`, the Chromium table re-export if any) keep their surface.

At layout-build time, before calling `Analyzer::analyze`:

1. Line breaks: split text into contiguous word-break-strength substrings
   (migrated `DenseWordBreaks`/`WordBreakSegmentIter`, including the
   single-word-break-style fast path), run the ICU `LineSegmenter` per
   substring, apply the `line_break_override` callback as a post-pass over
   adjacent char pairs, and collect the offsets `Vec` passed to the engine.
   Keep this behind one internal function with the shape
   `(text, word-break config) -> offsets` — the future extraction seam.

   Allocation note: the engine previously applied the override just-in-time
   inside its merge loop, materializing only raw ICU offsets. Post-override
   offsets must now be materialized instead, but it is still exactly one
   `Vec`, held as reusable scratch on `LayoutContext`, and of roughly the same
   length (the Chromium table mostly suppresses; its forced break after a
   space targets positions ICU already emits). The real cost is one extra
   traversal of the text for the override pair-walk; the override closure is
   called the same number of times as before. The bench comparison sizes this.
2. Word boundaries: run the ICU `WordSegmenter` over the full text and store its
   output as-is: `LayoutData::word_boundary_bytes: Vec<usize>`, sorted byte
   offsets of UAX-29 word-segment boundaries. `parley` has no existing per-cluster
   arrays (`Cluster::info()` reads engine-owned data), so this is a new side
   array; `Cluster::is_word_boundary()` binary-searches it via the cluster's
   text range. Word boundaries are sparse and the query sites are
   editing/selection/accessibility paths, so `O(log n)` is fine. Whether
   `parley` later packs this with line boundary data into its own
   `Boundary`-like (e.g. bitvec) representation is deliberately deferred.

`Cluster::is_word_boundary()` currently means "any boundary" (`boundary !=
None`), which is wrong (not all line opportunities are word boundaries) but is the
behavior selection/cursor/editing/accessibility ship on. This task keeps it
bug-compatible: the read site computes `word_boundary || line || mandatory`,
marked `// HACK(follow-up):` so review sees the storage is correct and only the
merge is legacy. Fixing the semantics is a hyper-local follow-up.

## Suggested implementation order

Single PR, reviewed commit-by-commit (the workspace must compile at every
commit, so engine and `parley` changes cannot be split into separate PRs):

1. Add `line_break_opportunities` to `AnalysisOptions` alongside the existing
   fields; engine prefers it when non-empty. Wire `parley` to compute and pass
   it (machinery still duplicated at this point). Validate: snapshot tests
   byte-identical.
2. Add `parley`-side word-boundary storage; switch `is_word_boundary()` to the
   marked hack expression. Validate: snapshot tests byte-identical.
3. Delete the engine-side segmentation, `word_break`/`line_break_override`
   options, `break_overrides` module; shrink `Boundary`; add the panics; move
   the `complex-scripts` feature. Port tests.
4. Changelog entries for both crates; bench comparison.

## Test and validation strategy

- The full snapshot/golden suites must pass byte-identical — this refactor is
  behavior-preserving by construction, which is the main correctness gate.
- `parley/src/tests/test_analysis.rs`: boundary-list tests that exercise
  word-break strength and overrides become tests of `parley`'s segmentation
  stage. Engine-level analysis tests supply hand-written offset slices.
- New engine tests: panic cases (unsorted, non-char-boundary, mid-grapheme
  offsets), empty-offsets input (no soft wrapping — a valid configuration).
- `parley_bench` before/after: analysis loses its fused fast path and `parley`
  gains passes; the word/letter-spacing and justification benches cover the
  affected paths.

## Consequences

- Breaking change for direct `parley_engine` users: they must bring a
  segmenter. The offsets contract is deliberately simple (sorted byte offsets)
  so any UAX-14 implementation or platform API can feed it. Note that
  `Intl.Segmenter` reports UTF-16 code-unit indices; converting to UTF-8 byte
  offsets is the caller's job and must be prominent in the docs.
- Engine and provider can disagree about grapheme boundaries across Unicode
  versions; the mid-grapheme panic surfaces that skew rather than hiding it.
- `parlance::WordBreak` stays in `parlance`; only `parley` consumes it now.
- With no offsets supplied, text simply never soft-wraps (mandatory breaks
  still apply). This is valid, not an error.

## Future work (separate tasks)

- Public `Segmenter` extension point in `parley`, extracted from the internal
  `(text, word-break config) -> offsets` seam, with the ICU default behind a
  feature so hosts can exclude the dictionaries entirely.
- Fix `Cluster::is_word_boundary()` to exclude line-only and mandatory-only
  boundaries (removing the marked hack); a documented behavior change for
  selection/cursor/double-click around hyphens, CJK, etc.
