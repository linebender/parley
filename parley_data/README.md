<div align="center">

# Parley Data

**Unicode data for text layout**

[![Latest published version.](https://img.shields.io/crates/v/parley_data.svg)](https://crates.io/crates/parley_data)
[![Documentation build status.](https://img.shields.io/docsrs/parley_data.svg)](https://docs.rs/parley_data)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
\
[![Linebender Zulip chat.](https://img.shields.io/badge/Linebender-%23parley-blue?logo=Zulip)](https://xi.zulipchat.com/#narrow/channel/205635-parley)
[![GitHub Actions CI status.](https://img.shields.io/github/actions/workflow/status/linebender/parley/ci.yml?logo=github&label=CI)](https://github.com/linebender/parley/actions)
[![Dependency staleness status.](https://deps.rs/crate/parley_data/latest/status.svg)](https://deps.rs/crate/parley_data)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=parley_data
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs should be evaluated here.
See https://linebender.org/blog/doc-include/ for related discussion. -->
[`Properties`]: https://docs.rs/parley_data/latest/parley_data/struct.Properties.html
[_alloc]: https://doc.rust-lang.org/stable/alloc/
<!-- cargo-rdme start -->

Parley Data packages the Unicode data that Parley's text analysis and shaping pipeline needs at runtime.
It exposes Unicode character [`Properties`][] backed by compact `PackTab` lookup tables, allowing the
engine to obtain all required character properties with a single lookup.

WARNING: This crate is currently designed only for use within Parley; if you have a use case for it,
please [reach out](https://xi.zulipchat.com/#narrow/channel/205635-parley).

## Features

The following crate [feature flags](https://doc.rust-lang.org/cargo/reference/features.html#dependency-features) are available:

- `std` (enabled by default): This is currently unused and is provided for forward compatibility.

Note that Parley Data requires that an allocator is available (i.e. it depends on [`alloc`][_alloc]).
Currently, Parley Data does not actually allocate, but the dependency is kept so starting to allocate in the future is not a breaking change.

## Regenerating the baked data

Run the following from the repository root to regenerate the baked data:

```sh
cargo run -p parley_data_gen -- ./parley_data/src/generated
cargo fmt -p parley_data
```

<!-- cargo-rdme end -->

## Minimum supported Rust Version (MSRV)

This version of Parley Data has been verified to compile with **Rust 1.88** and later.

Future versions of Parley Data might increase the Rust version requirement.
It will not be treated as a breaking change and as such can even happen with small patch releases.

## Community

Discussion of Parley Data development happens in the [Linebender Zulip](https://xi.zulipchat.com/), specifically the [#parley channel](https://xi.zulipchat.com/#narrow/channel/205635-parley).
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
