<div align="center">

# Fontique

**Font enumeration and fallback**

[![Latest published version.](https://img.shields.io/crates/v/fontique.svg)](https://crates.io/crates/fontique)
[![Documentation build status.](https://img.shields.io/docsrs/fontique.svg)](https://docs.rs/fontique)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
\
[![Linebender Zulip chat.](https://img.shields.io/badge/Linebender-%23parley-blue?logo=Zulip)](https://xi.zulipchat.com/#narrow/channel/205635-parley)
[![GitHub Actions CI status.](https://img.shields.io/github/actions/workflow/status/linebender/parley/ci.yml?logo=github&label=CI)](https://github.com/linebender/parley/actions)
[![Dependency staleness status.](https://deps.rs/crate/fontique/latest/status.svg)](https://deps.rs/crate/fontique)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=fontique
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs should be evaluated here.
See https://linebender.org/blog/doc-include/ for related discussion. -->
[`parlance`]: https://docs.rs/parlance/latest/parlance/
[`alloc`]: https://doc.rust-lang.org/stable/alloc/
<!-- cargo-rdme start -->

Font enumeration and fallback.

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
- `fontconfig-dlopen`: Use dlopen to load the fontconfig library.
  This allows Fontique to compile even if the system does not have fontconfig installed,
  although font fallback to system fonts will not work in this case.

At least one of `std` and `libm` is required; `std` overrides `libm`.
Note that Fontique requires that an allocator is available (i.e. it uses [`alloc`][]).

[libm]: https://crates.io/crates/libm

<!-- cargo-rdme end -->

## Minimum supported Rust Version (MSRV)

This version of Fontique has been verified to compile with **Rust 1.88** and later.

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

Discussion of Fontique development happens in the [Linebender Zulip](https://xi.zulipchat.com/), specifically the [#parley channel](https://xi.zulipchat.com/#narrow/channel/205635-parley).
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
