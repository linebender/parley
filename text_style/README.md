<div align="center">

# Text Style

CSS-inspired text style vocabulary.

[![Linebender Zulip, #parley channel](https://img.shields.io/badge/Linebender-%23parley-blue?logo=Zulip)](https://xi.zulipchat.com/#narrow/channel/205635-parley)
[![dependency status](https://deps.rs/repo/github/linebender/parley/status.svg)](https://deps.rs/repo/github/linebender/parley)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
[![Build status](https://github.com/linebender/parley/workflows/CI/badge.svg)](https://github.com/linebender/parley/actions)
[![Crates.io](https://img.shields.io/crates/v/text_style.svg)](https://crates.io/crates/text_style)
[![Docs](https://docs.rs/text_style/badge.svg)](https://docs.rs/text_style)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=text_style --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs should be evaluated here.
See https://linebender.org/blog/doc-include/ for related discussion. -->
<!-- cargo-rdme start -->

CSS-inspired text style vocabulary.

This crate defines:
- A closed set of inline and paragraph style properties (the vocabulary)
- CSS-like reset semantics via [`Specified`]

It is `no_std` + `alloc` friendly and is intentionally independent of any shaping/layout engine.

Specified→computed resolution (including parsing of raw OpenType settings sources) lives in the
companion crate `text_style_resolve`.

## Scope

This crate focuses on portable, engine-agnostic style semantics (fonts, metrics, bidi, and
OpenType settings). It intentionally does **not** model paint/brush types (color/gradients),
and it currently does not expose detailed decoration geometry (underline/strikethrough
thickness/offset) or decoration paints. Those are expected to live in wrapper attributes or in
an engine-lowering layer.

The primary integration pattern is:
- Author spans using [`InlineStyle`] declarations
- Resolve specified styles to computed runs (via `text_style_resolve` or a higher-level crate)
- Lower computed runs to a layout engine such as Parley

## Model

This crate is structured similarly to CSS:

- **Specified values** are expressed as declaration lists ([`InlineStyle`], [`ParagraphStyle`]).
- Declarations store values wrapped in [`Specified`], enabling `inherit`/`initial` behavior.
- **Computed values** are absolute and engine-ready, produced by an engine layer such as
  `text_style_resolve`.

## OpenType Settings

OpenType feature and variation settings are represented as [`Settings`]. For convenience, they
can be authored either as a parsed list ([`Settings::List`]) or as a CSS-like source string
([`Settings::Source`]). When using `Source`, parsing is performed by an engine layer (for
example `text_style_resolve`).

Resolution is performed relative to three computed styles (`parent`, `initial`, and `root`) by
an engine layer (for example `text_style_resolve`).

## Conflict Handling

Styles are lists of declarations rather than “one field per property”. When multiple
declarations of the same property are present, the **last** declaration in the list wins.
When multiple overlapping spans apply to the same text, the higher-level layer is expected to
define an application order (commonly: span application order, last writer wins).

## Relative Values

Some specified values are relative (for example [`FontSize::Em`], [`Spacing::Em`],
[`LineHeight::Em`], and their root-relative forms like `rem`). These are resolved against
*computed* context:

- `font-size: Em(x)` is resolved against the **parent** computed font size.
- `font-size: Rem(x)` is resolved against the **root** computed font size.
- properties like `letter-spacing` and `word-spacing` are resolved against the **computed**
  font size for the same resolved style.

This gives deterministic results for overlapping declarations and matches common CSS-like
systems.

## References

This crate is inspired by (but not identical to) these specifications:

- CSS Fonts: <https://www.w3.org/TR/css-fonts-4/>
- CSS Text: <https://www.w3.org/TR/css-text-3/> and <https://www.w3.org/TR/css-text-4/>
- Unicode Bidirectional Algorithm (UAX #9): <https://www.unicode.org/reports/tr9/>

## Example

```rust
use text_style::{BaseDirection, FontSize, InlineStyle, ParagraphStyle, Specified};

// "font-size: 1.25em; text-decoration-line: underline"
let inline = InlineStyle::new()
    .font_size(Specified::Value(FontSize::Em(1.25)))
    .underline(Specified::Value(true));
assert_eq!(inline.declarations().len(), 2);

// "direction: rtl"
let paragraph = ParagraphStyle::new().base_direction(Specified::Value(BaseDirection::Rtl));
assert_eq!(paragraph.declarations().len(), 1);
```

<!-- cargo-rdme end -->

## Minimum supported Rust Version (MSRV)

This version of Text Style has been verified to compile with **Rust 1.83** and later.

Future versions of Text Style might increase the Rust version requirement.
It will not be treated as a breaking change and as such can even happen with small patch releases.

<details>
<summary>Click here if compiling fails.</summary>

As time has passed, some of Text Style's dependencies could have released versions with a higher Rust requirement.
If you encounter a compilation issue due to a dependency and don't want to upgrade your Rust toolchain, then you could downgrade the dependency.

```sh
# Use the problematic dependency's name and version
cargo update -p package_name --precise 0.1.1
```

</details>

## Community

[![Linebender Zulip](https://img.shields.io/badge/Xi%20Zulip-%23parley-blue?logo=Zulip)](https://xi.zulipchat.com/#narrow/channel/205635-parley)

Discussion of Text Style development happens in the [Linebender Zulip](https://xi.zulipchat.com/), specifically the [#parley channel](https://xi.zulipchat.com/#narrow/channel/205635-parley).
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
