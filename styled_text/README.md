<div align="center">

# Styled Text

Compact styled text spans built on attributed_text.

[![Linebender Zulip, #parley channel](https://img.shields.io/badge/Linebender-%23parley-blue?logo=Zulip)](https://xi.zulipchat.com/#narrow/channel/205635-parley)
[![dependency status](https://deps.rs/repo/github/linebender/parley/status.svg)](https://deps.rs/repo/github/linebender/parley)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
[![Build status](https://github.com/linebender/parley/workflows/CI/badge.svg)](https://github.com/linebender/parley/actions)
[![Crates.io](https://img.shields.io/crates/v/styled_text.svg)](https://crates.io/crates/styled_text)
[![Docs](https://docs.rs/styled_text/badge.svg)](https://docs.rs/styled_text)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=styled_text --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs should be evaluated here.
See https://linebender.org/blog/doc-include/ for related discussion. -->
<!-- cargo-rdme start -->

Styled Text stores text with compact full-style identifiers.
It builds on [`attributed_text`] for range storage and segment resolution,
then adds interned style payloads for callers that need compact, reusable
style data.

The core idea is that each resolved text segment points at a [`StyleId`].
The [`StyleSet`] behind that id stores complete style records, with
layout-affecting payloads and paint-only payloads interned separately.
That means a paint-only change can share layout style identity with the
surrounding text, which is the information a shaping or layout cache usually
wants.

This is deliberately not a document model.
It does not own shaping, font resolution, inline boxes, cascading, or
renderer-specific style semantics.
Callers choose their own layout and paint payload types, then adapt the
resolved segments to Parley or another layout system.

Unlike an API that sets individual style bits on ranges and leaves a later
stage to interpret those bits, `styled_text` resolves patches into complete
style records before lowering.
That keeps the core crate independent of any toolkit's style vocabulary
while giving downstream code a simple stream of text ranges and full styles.

## Concepts

- [`StyledText`] stores the text, the resolved [`StyleId`] spans, and the
  shared [`StyleSet`].
- [`StyleSet`] interns layout payloads, paint payloads, and the joined style
  records that point at them.
- [`StylePatch`] is the small trait callers implement to say how a partial
  style change updates their own full style types.
- [`StyledTextBuilder`] appends text and applies patches in the order they
  were applied to the builder.
- [`StyledSegmentsWorkspace`] is reusable scratch storage for iterating
  resolved styled segments without reallocating every time.

## Building styled text

```rust
use styled_text::{StylePatch, StyledSegmentsWorkspace, StyledTextBuilder};

#[derive(Clone, Debug, PartialEq, Default)]
struct LayoutStyle {
    font_size: f32,
}

#[derive(Clone, Debug, PartialEq, Default)]
struct PaintStyle {
    rgba: [u8; 4],
}

#[derive(Clone, Debug, Default)]
struct TextStyleChange {
    font_size: Option<f32>,
    rgba: Option<[u8; 4]>,
}

impl StylePatch<LayoutStyle, PaintStyle> for TextStyleChange {
    fn apply_to(&self, layout: &mut LayoutStyle, paint: &mut PaintStyle) {
        if let Some(font_size) = self.font_size {
            layout.font_size = font_size;
        }
        if let Some(rgba) = self.rgba {
            paint.rgba = rgba;
        }
    }
}

let mut text = StyledTextBuilder::new(
    LayoutStyle { font_size: 16.0 },
    PaintStyle { rgba: [0, 0, 0, 255] },
);
text.push("Hello ");
let styled_range = text.push_with(
    "styled",
    TextStyleChange {
        font_size: Some(28.0),
        ..TextStyleChange::default()
    },
);
text.apply(
    styled_range,
    TextStyleChange {
        rgba: Some([220, 40, 40, 255]),
        ..TextStyleChange::default()
    },
);
text.push(" text");
let styled = text.finish();

let mut workspace = StyledSegmentsWorkspace::new();
for segment in workspace.segments(&styled) {
    let style = styled.style_set().segment_style(segment.style());
    // Feed segment.range() and style into layout or painting code.
}
```

## Features

- `std` (enabled by default): Enables the `std` feature of
  [`attributed_text`].

<!-- cargo-rdme end -->

## Minimum supported Rust Version (MSRV)

This version of Styled Text has been verified to compile with **Rust 1.88** and later.

Future versions of Styled Text might increase the Rust version requirement.
It will not be treated as a breaking change and as such can even happen with small patch releases.

<details>
<summary>Click here if compiling fails.</summary>

As time has passed, some of Styled Text's dependencies could have released versions with a higher Rust requirement.
If you encounter a compilation issue due to a dependency and don't want to upgrade your Rust toolchain, then you could downgrade the dependency.

```sh
# Use the problematic dependency's name and version
cargo update -p package_name --precise 0.1.1
```

</details>

## Community

[![Linebender Zulip](https://img.shields.io/badge/Xi%20Zulip-%23parley-blue?logo=Zulip)](https://xi.zulipchat.com/#narrow/channel/205635-parley)

Discussion of Styled Text development happens in the [Linebender Zulip](https://xi.zulipchat.com/), specifically the [#parley channel](https://xi.zulipchat.com/#narrow/channel/205635-parley).
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
