<div align="center">

# Text Style Resolve

Specified→computed resolution for `text_style`.

[![Linebender Zulip, #parley channel](https://img.shields.io/badge/Linebender-%23parley-blue?logo=Zulip)](https://xi.zulipchat.com/#narrow/channel/205635-parley)
[![dependency status](https://deps.rs/repo/github/linebender/parley/status.svg)](https://deps.rs/repo/github/linebender/parley)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
[![Build status](https://github.com/linebender/parley/workflows/CI/badge.svg)](https://github.com/linebender/parley/actions)
[![Crates.io](https://img.shields.io/crates/v/text_style_resolve.svg)](https://crates.io/crates/text_style_resolve)
[![Docs](https://docs.rs/text_style_resolve/badge.svg)](https://docs.rs/text_style_resolve)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=text_style_resolve --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs should be evaluated here.
See https://linebender.org/blog/doc-include/ for related discussion. -->
<!-- cargo-rdme start -->

Specified→computed resolution for [`text_style`].

[`text_style`] intentionally focuses on a lightweight, shareable style vocabulary:
declarations, specified values, and common value types.

This crate provides the “engine” layer:
- Computed style types ([`ComputedInlineStyle`], [`ComputedParagraphStyle`])
- Resolution contexts ([`InlineResolveContext`], [`ParagraphResolveContext`])
- Specified→computed resolution (including parsing of raw OpenType settings sources)

It is `no_std` + `alloc` friendly.

## Example

```rust
use text_style::{BaseDirection, FontSize, InlineStyle, ParagraphStyle, Specified};
use text_style_resolve::{
    ComputedInlineStyle, ComputedParagraphStyle, InlineResolveContext, ParagraphResolveContext,
    ResolveStyleExt,
};

let base_inline = ComputedInlineStyle::default();
let base_paragraph = ComputedParagraphStyle::default();

// "font-size: 1.25em; text-decoration-line: underline"
let inline = InlineStyle::new()
    .font_size(Specified::Value(FontSize::Em(1.25)))
    .underline(Specified::Value(true));
let inline_ctx = InlineResolveContext::new(&base_inline, &base_inline, &base_inline);
let computed_inline = inline.resolve(inline_ctx).unwrap();
assert_eq!(computed_inline.font_size_px(), base_inline.font_size_px() * 1.25);
assert!(computed_inline.underline());

// "direction: rtl"
let paragraph = ParagraphStyle::new().base_direction(Specified::Value(BaseDirection::Rtl));
let paragraph_ctx =
    ParagraphResolveContext::new(&base_paragraph, &base_paragraph, &base_paragraph);
let computed_paragraph = paragraph.resolve(paragraph_ctx);
assert_eq!(computed_paragraph.base_direction(), BaseDirection::Rtl);
```

<!-- cargo-rdme end -->

## Minimum supported Rust Version (MSRV)

This version of Text Style Resolve has been verified to compile with **Rust 1.83** and later.

Future versions of Text Style Resolve might increase the Rust version requirement.
It will not be treated as a breaking change and as such can even happen with small patch releases.

## Community

[![Linebender Zulip](https://img.shields.io/badge/Xi%20Zulip-%23parley-blue?logo=Zulip)](https://xi.zulipchat.com/#narrow/channel/205635-parley)

Discussion of Parley development happens in the [Linebender Zulip](https://xi.zulipchat.com/), specifically the [#parley channel](https://xi.zulipchat.com/#narrow/channel/205635-parley).
All public content can be read without logging in.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

