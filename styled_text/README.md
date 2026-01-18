<div align="center">

# Styled Text

Attributed text + CSS-inspired span styles + a lightweight document block model.

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

Span styling and document structure built on [`attributed_text`].

- [`style`] defines a closed style vocabulary.
- [`resolve`] resolves specified styles to computed styles.
- [`attributed_text`] stores generic attributes on byte ranges.
- `styled_text` combines them:
  - [`StyledText`]: a single layout block (maps cleanly to one Parley `Layout`)
  - [`StyledDocument`]: a flat sequence of blocks with semantic kinds (headings, list items…)

## Scope

This crate provides span application, inline style run resolution, and a lightweight block
model. It does not itself lower styles to Parley APIs, and it does not define paint/brush types
(those are expected to live in wrapper attributes and an engine-lowering layer).

## Design Intent

`StyledText` is intended to be a durable attributed-text model:
- it can be parsed from markup and retained as an application’s in-memory representation
- it can be used transiently as a “layout input packet” if you already have your own model
- it aims to be a reasonable interchange format for rich text (for example copy/paste)

The mutation/editing story is still evolving. Short-term APIs focus on span application and
layout-facing iteration; richer mutation patterns (inserts/deletes with span adjustment, etc.)
are expected to be added over time.

## Indices

All ranges are expressed as **byte indices** into UTF-8 text, and must be on UTF-8 character
boundaries (as required by [`attributed_text`]).

## Overlaps

When spans overlap, inline style resolution applies spans in the order they were added (last
writer wins). Higher-level semantic attributes can be carried by wrapper types via
[`HasInlineStyle`].

## Example: Styled spans

```rust
use styled_text::StyledText;
use styled_text::style::{FontSize, InlineStyle, Specified};
use styled_text::resolve::{ComputedInlineStyle, ComputedParagraphStyle};

let base_inline = ComputedInlineStyle::default();
let base_paragraph = ComputedParagraphStyle::default();
let mut text = StyledText::new("Hello world!", base_inline, base_paragraph);

// Make "world!" 1.5x larger.
let world = 6..12;
let style = InlineStyle::new().font_size(Specified::Value(FontSize::Em(1.5)));
text.apply_span(text.range(world).unwrap(), style);

let runs: Vec<_> = text
    .resolved_inline_runs_coalesced()
    .collect();
assert_eq!(runs.len(), 2);
assert_eq!(runs[1].range, 6..12);
```

## Example: Wrapper attributes for semantics

```rust
use alloc::sync::Arc;
use styled_text::{HasInlineStyle, StyledText};
use styled_text::style::InlineStyle;
use styled_text::resolve::{ComputedInlineStyle, ComputedParagraphStyle};

#[derive(Debug, Clone)]
struct Attr {
    style: InlineStyle,
    href: Option<Arc<str>>,
}

impl HasInlineStyle for Attr {
    fn inline_style(&self) -> &InlineStyle {
        &self.style
    }
}

let base_inline = ComputedInlineStyle::default();
let base_paragraph = ComputedParagraphStyle::default();
let mut text = StyledText::new("Click me", base_inline, base_paragraph);
text.apply_span(
    text.range(0..8).unwrap(),
    Attr {
        style: InlineStyle::new(),
        href: Some(Arc::from("https://example.invalid")),
    },
);
```

<!-- cargo-rdme end -->

## Minimum supported Rust Version (MSRV)

This version of Styled Text has been verified to compile with **Rust 1.83** and later.

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
[AUTHORS]: ./AUTHORS
