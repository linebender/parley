<div align="center">

# Styled Text Parley

Parley backend for `styled_text`.

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

Parley backend for [`styled_text`].

This crate lowers `styled_text`’s resolved computed style runs into Parley builder calls,
producing a [`parley::Layout`].

## Scope

This crate focuses on mapping `styled_text` computed styles into Parley
[`parley::StyleProperty`] values.

It intentionally does not handle:
- paint/brush resolution (callers provide a default brush and may extend this crate later)
- inline bidi controls / forced base direction (not currently modeled by Parley style properties)
- inline boxes / attachments (use `parley::InlineBox` directly when you add an attachment layer)

## Example

```rust
use parley::{FontContext, Layout, LayoutContext};
use styled_text::StyledText;
use styled_text_parley::build_layout_from_styled_text;
use styled_text::{ComputedInlineStyle, ComputedParagraphStyle, FontSize, InlineStyle, Specified};

let mut font_cx = FontContext::new();
let mut layout_cx = LayoutContext::new();
let base_inline = ComputedInlineStyle::default();
let base_paragraph = ComputedParagraphStyle::default();
let mut text = StyledText::new("Hello world!", base_inline, base_paragraph);
text.apply_span(
    text.range(6..12).unwrap(),
    InlineStyle::new().font_size(Specified::Value(FontSize::Em(1.5))),
);

let layout: Layout<()> =
    build_layout_from_styled_text(&mut layout_cx, &mut font_cx, &text, 1.0, true, ());
```

<!-- cargo-rdme end -->

## Minimum supported Rust Version (MSRV)

This version of Styled Text Parley has been verified to compile with **Rust 1.83** and later.
