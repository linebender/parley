<div align="center">

# Styled Text Parley

Parley lowering for compact styled_text style runs.

[![Linebender Zulip, #parley channel](https://img.shields.io/badge/Linebender-%23parley-blue?logo=Zulip)](https://xi.zulipchat.com/#narrow/channel/205635-parley)
[![dependency status](https://deps.rs/repo/github/linebender/parley/status.svg)](https://deps.rs/repo/github/linebender/parley)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
[![Build status](https://github.com/linebender/parley/workflows/CI/badge.svg)](https://github.com/linebender/parley/actions)
[![Crates.io](https://img.shields.io/crates/v/styled_text_parley.svg)](https://crates.io/crates/styled_text_parley)
[![Docs](https://docs.rs/styled_text_parley/badge.svg)](https://docs.rs/styled_text_parley)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=styled_text_parley --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs should be evaluated here.
See https://linebender.org/blog/doc-include/ for related discussion. -->
<!-- cargo-rdme start -->

Styled Text Parley adapts [`styled_text`] to Parley's low-level style-run
builder.
It lowers resolved styled-text segments into Parley's style table and range
runs, while reusing scratch storage across layout builds.

The crate also provides a Parley-shaped first style vocabulary.
[`ParleyLayoutStyle`] holds the fields that can affect shaping and line
layout.
[`ParleyPaintStyle`] holds paint-only fields such as brushes and decorations.
Interning those payloads separately means paint-only changes can share
layout identity when the styled text is lowered.

This adapter does not own document structure, inline boxes, cascading, or
renderer-specific style semantics.
Callers can use the provided Parley style payloads for a simple path, or keep
their own style types in `styled_text` and use the generic lowering
functions.

## Concepts

- [`ParleyStyledTextBuilder`] is a [`StyledTextBuilder`] configured with the
  default Parley style payloads and patch type.
- [`ParleyStyleChange`] is a partial style patch for common Parley fields,
  with public fields for less common changes.
- [`ParleyStyleRunWorkspace`] keeps the reusable segment workspace and the
  temporary [`StyleId`] to Parley style-index map.
- [`build_layout_from_parley_styled_text`] creates a Parley [`Layout`] from
  text built with the default Parley payloads.
- [`push_style_runs`] is the lower-level hook for callers that want to feed
  Parley style runs themselves.

## Building a Parley layout

```rust
use parley::{FontContext, FontWeight, LayoutContext};
use styled_text_parley::{
    ParleyLayoutStyle, ParleyPaintStyle, ParleyStyleChange, ParleyStyleRunWorkspace,
    ParleyStyledTextBuilder, build_layout_from_parley_styled_text,
};

let mut text = ParleyStyledTextBuilder::<()>::new(
    ParleyLayoutStyle::default(),
    ParleyPaintStyle::default(),
);
text.push("Hello ");
text.push_with(
    "styled text",
    ParleyStyleChange::default()
        .font_size(24.0)
        .font_weight(FontWeight::BOLD),
);
let styled = text.finish();

let mut font_cx = FontContext::new();
let mut layout_cx = LayoutContext::<()>::new();
let mut workspace = ParleyStyleRunWorkspace::new();
let mut layout = build_layout_from_parley_styled_text(
    &mut layout_cx,
    &mut font_cx,
    &styled,
    &mut workspace,
    1.0,
    true,
).unwrap();
layout.break_all_lines(Some(240.0));
```

## Features

- `std` (enabled by default): Enables `std` support in [`parley`] and
  [`styled_text`].
- `libm`: Enables the `libm` feature of [`parley`].

<!-- cargo-rdme end -->

## Minimum supported Rust Version (MSRV)

This version of Styled Text Parley has been verified to compile with **Rust 1.88** and later.

Future versions of Styled Text Parley might increase the Rust version requirement.
It will not be treated as a breaking change and as such can even happen with small patch releases.

<details>
<summary>Click here if compiling fails.</summary>

As time has passed, some of Styled Text Parley's dependencies could have released versions with a higher Rust requirement.
If you encounter a compilation issue due to a dependency and don't want to upgrade your Rust toolchain, then you could downgrade the dependency.

```sh
# Use the problematic dependency's name and version
cargo update -p package_name --precise 0.1.1
```

</details>

## Community

[![Linebender Zulip](https://img.shields.io/badge/Xi%20Zulip-%23parley-blue?logo=Zulip)](https://xi.zulipchat.com/#narrow/channel/205635-parley)

Discussion of Styled Text Parley development happens in the [Linebender Zulip](https://xi.zulipchat.com/), specifically the [#parley channel](https://xi.zulipchat.com/#narrow/channel/205635-parley).
All public content can be read without logging in.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Contributions are welcome by pull request. The [Rust code of conduct] applies.
Please feel free to add your name to the [AUTHORS] file in any substantive pull request.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be licensed as above, without any additional terms or conditions.

[Rust Code of Conduct]: https://www.rust-lang.org/policies/code-of-conduct
[AUTHORS]: ../AUTHORS
