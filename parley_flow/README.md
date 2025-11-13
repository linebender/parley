# parley_flow (TextBlock + TextFlow)

A small, flow-first layer on top of Parley that introduces:

- TextBlock: a minimal trait for blocks of laid-out text (paragraphs, labels)
- LayoutBlock: adapter for a `parley::layout::Layout` + `&str`
- TextFlow: explicit ordered containers (rect + join policy) for deterministic hit-testing,
  cross-block navigation, and text concatenation
- Flow-based helpers: `hit_test`, `selection_geometry`, `copy_text`

Status: experimental. Names and APIs may change as this evolves.

See `src/design.rs` for comparisons to TextKit/DirectWrite/Android and long-term goals.
