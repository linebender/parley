<div align="center">

# Parley

**Rich text layout**

[![Latest published version.](https://img.shields.io/crates/v/parley.svg)](https://crates.io/crates/parley)
[![Documentation build status.](https://img.shields.io/docsrs/parley.svg)](https://docs.rs/parley)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
\
[![Linebender Zulip chat.](https://img.shields.io/badge/Linebender-%23parley-blue?logo=Zulip)](https://xi.zulipchat.com/#narrow/channel/205635-parley)
[![GitHub Actions CI status.](https://img.shields.io/github/actions/workflow/status/linebender/parley/ci.yml?logo=github&label=CI)](https://github.com/linebender/parley/actions)
[![Dependency staleness status.](https://deps.rs/crate/parley/latest/status.svg)](https://deps.rs/crate/parley)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=parley
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs should be evaluated here.
See https://linebender.org/blog/doc-include/ for related discussion. -->
[`FontContext`]: https://docs.rs/parley/latest/parley/struct.FontContext.html
[`LayoutContext`]: https://docs.rs/parley/latest/parley/struct.LayoutContext.html
[`Layout`]: https://docs.rs/parley/latest/parley/struct.Layout.html
[`RangedBuilder`]: https://docs.rs/parley/latest/parley/struct.RangedBuilder.html
[`TreeBuilder`]: https://docs.rs/parley/latest/parley/struct.TreeBuilder.html
[`StyleRunBuilder`]: https://docs.rs/parley/latest/parley/struct.StyleRunBuilder.html
[`LayoutContext::ranged_builder`]: https://docs.rs/parley/latest/parley/struct.LayoutContext.html#method.ranged_builder
[`LayoutContext::tree_builder`]: https://docs.rs/parley/latest/parley/struct.LayoutContext.html#method.tree_builder
[`LayoutContext::style_run_builder`]: https://docs.rs/parley/latest/parley/struct.LayoutContext.html#method.style_run_builder
[`alloc`]: https://doc.rust-lang.org/stable/alloc/
[`bytemuck`]: https://docs.rs/bytemuck/latest/bytemuck
[`DictionaryProvider`]: https://docs.rs/parley/latest/parley/trait.DictionaryProvider.html
<!-- cargo-rdme start -->

Parley is a library for rich text layout.
It is backed by [HarfRust](https://github.com/harfbuzz/harfrust) for text shaping.

Some key types are:

- [`FontContext`][] and [`LayoutContext`][] are resources which should be shared globally (or at coarse-grained boundaries).
  - [`FontContext`][] is database of fonts.
  - [`LayoutContext`][] is scratch space that allows for reuse of allocations between layouts.
- Builders for creating a [`Layout`][]:
  - [`RangedBuilder`][]: styles specified as a flat `Vec` of property spans
  - [`TreeBuilder`][]: styles specified as a tree of spans
  - [`StyleRunBuilder`][]: styles specified as a shared style table plus non-overlapping indexed runs

  They are constructed using [`LayoutContext::ranged_builder`][], [`LayoutContext::tree_builder`][], and [`LayoutContext::style_run_builder`][].
- [`Layout`][] which represents styled paragraph(s) of text and can perform shaping, line-breaking, bidi-reordering, and alignment of that text.

  `Layout` supports re-linebreaking and re-aligning many times (in case the width at which wrapping should occur changes).
  But if the text content or the styles applied to that content change then a new `Layout` must be created using a
  new `RangedBuilder`, `TreeBuilder`, or `StyleRunBuilder`.

## Usage Example

See the [examples](https://github.com/linebender/parley/tree/main/examples) directory for more complete usage examples that include rendering.

```rust
use parley::{
   Alignment, AlignmentOptions, FontContext, FontWeight, InlineBox, InlineBoxKind, Layout,
   LayoutContext, LineHeight, PositionedLayoutItem, StyleProperty, VerticalAlign,
};

// Create a FontContext (font database) and LayoutContext (scratch space).
// These are both intended to be constructed rarely (perhaps even once per app):
let mut font_cx = FontContext::new();
let mut layout_cx = LayoutContext::new();

// Create a `RangedBuilder` or a `TreeBuilder`, which are used to construct a `Layout`.
const DISPLAY_SCALE : f32 = 1.0;
const TEXT : &str = "Lorem\u{FFFC} Ipsum...";
let mut builder = layout_cx.ranged_builder(&mut font_cx, &TEXT, DISPLAY_SCALE, true);

// Set default styles that apply to the entire layout
builder.push_default(StyleProperty::FontSize(16.0));

// Set a style that applies to the first 4 characters
builder.push(StyleProperty::FontWeight(FontWeight::new(600.0)), 0..4);

// Add a box to be laid out inline with the text, in place of the U+FFFC OBJECT REPLACEMENT
// CHARACTER
builder.push_inline_box(InlineBox { id: 0, kind: InlineBoxKind::InFlow, index: 5, width: 50.0, height: 50.0, baseline: None, vertical_align: VerticalAlign::BASELINE });

// Build the builder into a Layout
let mut layout: Layout<()> = builder.build(&TEXT);

// Run line-breaking and alignment on the Layout
const MAX_WIDTH : Option<f32> = Some(100.0);
layout.break_all_lines(MAX_WIDTH);
layout.align(Alignment::Start, AlignmentOptions::default());

// Inspect computed layout (see examples for more details)
let width = layout.width();
let height = layout.height();
for line in layout.lines() {
    for item in line.items() {
        match item {
            PositionedLayoutItem::GlyphRun(glyph_run) => {
                // Render the glyph run
            }
            PositionedLayoutItem::InlineBox(inline_box) => {
                // Render the inline box
            }
        };
    }
}
```

## Features

The following crate [feature flags](https://doc.rust-lang.org/cargo/reference/features.html#dependency-features) are available:

- `std` (enabled by default): Get floating point functions from the standard library
  (likely using your target's libc).
- `libm`: Use floating point implementations from [libm][].
  This is useful for `no_std` environments.
  However, note that the `libm` crate is not as efficient as the standard library.
- `bytemuck`: Forwarded to [`parlance`][] which then implements traits from `bytemuck` on its types.
- `system` (enabled by default): Enable support for system font backends.
  This also enables `std`.
- `complex-scripts`: Enable dictionary-based line and word breaking for complex scripts (CJK, Thai, Khmer, Lao, Myanmar).
  When disabled, a lightweight segmenter is used that falls back to character-level breaks for those scripts.
  See [`DictionaryProvider`][] for full details.

At least one of `std` and `libm` is required; `std` overrides `libm`.
Note that Parley requires that an allocator is available (i.e. it uses [`alloc`][]).

[libm]: https://crates.io/crates/libm

<!-- cargo-rdme end -->

## Minimum supported Rust Version (MSRV)

This version of Parley has been verified to compile with **Rust 1.88** and later.

Future versions of Parley might increase the Rust version requirement.
It will not be treated as a breaking change and as such can even happen with small patch releases.

<details>
<summary>Click here if compiling fails.</summary>

As time has passed, some of Parley's dependencies could have released versions with a higher Rust requirement.
If you encounter a compilation issue due to a dependency and don't want to upgrade your Rust toolchain, then you could downgrade the dependency.

```sh
# Use the problematic dependency's name and version
cargo update -p package_name --precise 0.1.1
```
</details>

## Community

Discussion of Parley development happens in the [Linebender Zulip](https://xi.zulipchat.com/), specifically the [#parley channel](https://xi.zulipchat.com/#narrow/channel/205635-parley).
All public content can be read without logging in.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Contributions are welcome by pull request.
The [Rust code of conduct] applies.
Please feel free to add your name to the [AUTHORS] file in any substantive pull request.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be licensed as above, without any additional terms or conditions.

[Rust code of conduct]: https://www.rust-lang.org/policies/code-of-conduct
[AUTHORS]: ../AUTHORS
