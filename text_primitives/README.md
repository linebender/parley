<div align="center">

# Text Primitives

Fundamental text property types.

[![Linebender Zulip, #parley channel](https://img.shields.io/badge/Linebender-%23parley-blue?logo=Zulip)](https://xi.zulipchat.com/#narrow/channel/205635-parley)
[![dependency status](https://deps.rs/repo/github/linebender/parley/status.svg)](https://deps.rs/repo/github/linebender/parley)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
[![Build status](https://github.com/linebender/parley/workflows/CI/badge.svg)](https://github.com/linebender/parley/actions)
[![Crates.io](https://img.shields.io/crates/v/text_primitives.svg)](https://crates.io/crates/text_primitives)
[![Docs](https://docs.rs/text_primitives/badge.svg)](https://docs.rs/text_primitives)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=text_primitives --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs should be evaluated here.
See https://linebender.org/blog/doc-include/ for related discussion. -->
<!-- cargo-rdme start -->

Fundamental text property types.

This crate is intended as a lightweight, `no_std`-friendly vocabulary layer that can be shared
across style systems, text layout engines, and font tooling. It focuses on small, typed
representations of common “leaf” concepts (weights, widths, OpenType tags, language tags, etc).

## Features

- `std` (enabled by default): This is currently unused and is provided for forward compatibility.

## Example

```rust
use text_primitives::{Language, Tag};

let tag = Tag::parse("wght").unwrap();
assert_eq!(tag.to_bytes(), *b"wght");

let lang = Language::parse("zh-Hans-CN").unwrap();
assert_eq!(lang.as_str(), "zh-Hans-CN");
assert_eq!(lang.language(), "zh");
assert_eq!(lang.script(), Some("Hans"));
assert_eq!(lang.region(), Some("CN"));
```

<!-- cargo-rdme end -->

## Minimum supported Rust Version (MSRV)

This version of Text Primitives has been verified to compile with **Rust 1.83** and later.

Future versions of Text Primitives might increase the Rust version requirement.
It will not be treated as a breaking change and as such can even happen with small patch releases.

<details>
<summary>Click here if compiling fails.</summary>

As time has passed, some of Text Primitives's dependencies could have released versions with a higher Rust requirement.
If you encounter a compilation issue due to a dependency and don't want to upgrade your Rust toolchain, then you could downgrade the dependency.

```sh
# Use the problematic dependency's name and version
cargo update -p package_name --precise 0.1.1
```

</details>

## Community

[![Linebender Zulip](https://img.shields.io/badge/Xi%20Zulip-%23parley-blue?logo=Zulip)](https://xi.zulipchat.com/#narrow/channel/205635-parley)

Discussion of Text Primitives development happens in the [Linebender Zulip](https://xi.zulipchat.com/), specifically the [#parley channel](https://xi.zulipchat.com/#narrow/channel/205635-parley).
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

