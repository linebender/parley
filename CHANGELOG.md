<!-- Instructions

This changelog follows the patterns described here: <https://keepachangelog.com/en/>.

Subheadings to categorize changes are `added, changed, deprecated, removed, fixed, security`.

-->

# Changelog

The latest published Parley release is [0.2.0](#020---2024-10-10) which was released on 2024-10-10.
You can find its changes [documented below](#020---2024-10-10).

## [Unreleased]

This release has an [MSRV] of 1.75.

### Added

#### Fontique

- `FontStretch`, `FontStyle`, and `FontWeight` get helper functions `from_fontconfig` ([#212] by [@waywardmonkeys][])

#### Parley

- `Generation` on `PlainEditor` to help implement lazy drawing. ([#143] by [@xorgy])
- Support for preedit for input methods in `PlainEditor` ([#192][], [#198][] by [@tomcur][])
- `PlainEditor` method to get a cursor area for use by the platform's input method ([#224][] by [@tomcur][])

### Changed

#### Fontique

- Breaking change: `Stretch`, `Style`, and `Weight` renamed to `FontWidth`, `FontStyle`, `FontWeight` ([#211][], [#223][] by [@waywardmonkeys][])

#### Parley

- Breaking change: `PlainEditor`'s semantics are no longer transactional ([#192][] by [@DJMcNab][])

## [0.2.0] - 2024-10-10

This release has an [MSRV] of 1.75.

### Added

#### Parley

- Example using tiny-skia which renders into a png ([#55] by [@nicoburns])
    - Breaking change: There is now a blanket implementation for `Brush`.
- A swash example which renders into a png ([#54] by [@nicoburns])
- An example with Vello on Winit which shows a basic text editor ([#106] by [@dfrg])
- `PlainEditor`, a basic action-based text editor based on Parley `Selection` and `Cursor` ([#126] by [@xorgy])
- Tree style builder ([#76] by [@nicoburns])
- Conversions for `FontFamily`, `FontStack`, and `StyleProperty` to make styling more ergonomic ([#129] by [@xorgy])

### Changed

#### General

- Repository layout updated to match Linebender standard ([#59] by [@waywardmonkeys])

#### Parley

- Emoji clusters now get an Emoji family added by default ([#56] by [@dfrg])
- Style builders now accept `Into<StyleProperty<'a, B: Brush>>` so you can push a `GenericFamily` or `FontStack` directly. ([#129] by [@xorgy])

#### Fontique

- Removed unsafe code from fontconfig cache ([#78] by [@waywardmonkeys])
- Switched to `windows-rs` for `dwrite` backend ([#85] by [@dfrg])

### Fixed

#### Fontique

- Search correct paths for fonts on Apple platforms ([#71] by [@waywardmonkeys])

### Removed

#### Fontique

- Breaking change: removed conversion to/from `icu_properties::Script` for `fontique::Script` ([#72] by [@waywardmonkeys])
    - This can be restored by using the `icu_properties` feature of `fontique`.

## [0.1.0] - 2024-05-01

This release has an [MSRV] of 1.70.

- Initial release

[MSRV]: README.md#minimum-supported-rust-version-msrv

[@dfrg]: https://github.com/dfrg
[@DJMcNab]: https://github.com/DJMcNab
[@nicoburns]: https://github.com/nicoburns
[@tomcur]: https://github.com/tomcur
[@waywardmonkeys]: https://github.com/waywardmonkeys
[@xorgy]: https://github.com/xorgy

[#54]: https://github.com/linebender/parley/pull/54
[#55]: https://github.com/linebender/parley/pull/55
[#56]: https://github.com/linebender/parley/pull/56
[#59]: https://github.com/linebender/parley/pull/59
[#71]: https://github.com/linebender/parley/pull/71
[#72]: https://github.com/linebender/parley/pull/72
[#76]: https://github.com/linebender/parley/pull/76
[#78]: https://github.com/linebender/parley/pull/78
[#85]: https://github.com/linebender/parley/pull/85
[#106]: https://github.com/linebender/parley/pull/106
[#126]: https://github.com/linebender/parley/pull/126
[#129]: https://github.com/linebender/parley/pull/129
[#143]: https://github.com/linebender/parley/pull/143
[#192]: https://github.com/linebender/parley/pull/192
[#198]: https://github.com/linebender/parley/pull/198
[#211]: https://github.com/linebender/parley/pull/211
[#223]: https://github.com/linebender/parley/pull/223
[#224]: https://github.com/linebender/parley/pull/224

[Unreleased]: https://github.com/linebender/parley/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/linebender/parley/releases/tag/v0.2.0
[0.1.0]: https://github.com/linebender/parley/releases/tag/v0.1.0
