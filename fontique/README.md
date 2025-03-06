<div align="center">

# Fontique

**Font enumeration and fallback**

[![Latest published fontique version.](https://img.shields.io/crates/v/fontique.svg)](https://crates.io/crates/fontique)
[![Documentation build status.](https://img.shields.io/docsrs/fontique.svg)](https://docs.rs/fontique)
[![Dependency staleness status.](https://deps.rs/crate/fontique/latest/status.svg)](https://deps.rs/crate/fontique)
[![Linebender Zulip chat.](https://img.shields.io/badge/Linebender-%23text-blue?logo=Zulip)](https://xi.zulipchat.com/#narrow/stream/205635-text)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)

</div>

Fontique provides font enumeration and fallback.

## Minimum supported Rust Version (MSRV)

This version of Fontique has been verified to compile with **Rust 1.85** and later.

Future versions of Fontique might increase the Rust version requirement.
It will not be treated as a breaking change and as such can even happen with small patch releases.

<details>
<summary>Click here if compiling fails.</summary>

As time has passed, some of Fontique's dependencies could have released versions with a higher Rust requirement.
If you encounter a compilation issue due to a dependency and don't want to upgrade your Rust toolchain, then you could downgrade the dependency.

```sh
# Use the problematic dependency's name and version
cargo update -p package_name --precise 0.1.1
```
</details>

## Community

Discussion of Fontique development happens in the [Linebender Zulip](https://xi.zulipchat.com/), specifically the [#text stream](https://xi.zulipchat.com/#narrow/stream/205635-text).
All public content can be read without logging in.

Contributions are welcome by pull request. The [Rust code of conduct] applies.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache 2.0 license, shall be licensed as noted in the [License](#license) section, without any additional terms or conditions.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

[Rust code of conduct]: https://www.rust-lang.org/policies/code-of-conduct
