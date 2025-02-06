<!-- Instructions

This changelog follows the patterns described here: <https://keepachangelog.com/en/>.

Subheadings to categorize changes are `added, changed, deprecated, removed, fixed, security`.

-->

# Changelog

The latest published Parley release is [0.3.0](#030---2025-02-27) which was released on 2025-02-27.
You can find its changes [documented below](#030---2025-02-27).

## [Unreleased]

This release has an [MSRV] of 1.82.

## [0.3.0] - 2025-02-27

This release has an [MSRV] of 1.82.

### Added

#### Fontique

- `FontStretch`, `FontStyle`, and `FontWeight` get helper functions `from_fontconfig` ([#212][] by [@waywardmonkeys][])
- Impl bytemuck traits for GenericFamily ([#213][] by [@waywardmonkeys][])

#### Parley

- `Generation` on `PlainEditor` to help implement lazy drawing. ([#143] by [@xorgy])
- Support for preedit for input methods in `PlainEditor` ([#192][], [#198][] by [@tomcur][])
- `PlainEditor` method to get a cursor area for use by the platform's input method ([#224][] by [@tomcur][])
- `Layout` methods to calculate minimum and maximum content widths. ([#259][] by [@wfdewith][])
- `PlainEditor` now implements `Clone` for ([#133][] by [@nicoburns])
- `PlainEditor`: Add byte selection and navigation operations. ([#146][] by [@xorgy])
- AccessKit integration ([#166][] by [@mwcampbell])
- Add `first_style` method to Cluster ([#264][] by [@nicoburns])

### Changed

#### Fontique

- Breaking change: `Stretch`, `Style`, and `Weight` renamed to `FontWidth`, `FontStyle`, `FontWeight` ([#211][], [#223][] by [@waywardmonkeys][])
- Fontique: depend on read-fonts instead of skrifa ([#162][] by [@nicoburns][])

#### Parley

- Breaking change: The cursor API has been completely reworked ([#170][] by [@dfrg])
- Breaking change: `PlainEditor`s API is now method-based rather than enum based ([#154][] by @mwcampbell)
- Breaking change: `PlainEditor`'s semantics are no longer transactional ([#192][] by [@DJMcNab][])
- Breaking change: `Alignment::Start` and `Alignment::End` now depend on text base direction.
  `Alignment::Left` and `Alignment::Right` are introduced for text direction-independent alignment. ([#250][] by [@tomcur][])
- Breaking change: `Layout` is no longer `Sync`. ([#259][] by [@wfdewith][])
- Breaking change: `PlainEditor`'s width is now `Option<f32>` rather than `f32` ([#137][] by [@nicoburns])
- Breaking change: Make alignment when free space is negative configurable ([#241][] by [@nicoburns])

241

### Fixed

#### Parley

- Fix alignment of right-to-left text. ([#250][], [#268][] by [@tomcur][])
- Performing line breaking or aligning a layout again, after justified alignment had been applied previously, now lead to the correct results. ([#271][] by [@tomcur][])
- Fix placement of inline boxes ([#163][] by [@spirali][])
- Cursor position for lines that are not left-aligned ([#169][] by [@mwcampbell])
- Fix Cursor::from_point to use the line's offset ([#176][] by [@DJMcNab])
- Fix off-by-one error in PlainEditor::cursor_at ([#187][] by [@tomcur])
- Fix binary search in Layout::line_for_offset ([#188][] by [@tomcur])
- Fix whitespace collapsing at the start of inline spans ([#191][] by [@nicoburns])
- Fix collapsing selection ([#201][] by [@tomcur])
- Ignore affinities in Selection::is_collapsed ([#202][] by [@nicoburns])
- Misc. inline box layout fixes ([#207][] by [@nicoburns])
- Allow Bidi base_level to be determined from text ([#245][] by [@nicoburns])
- Fix linebreaking for lines without text runs ([#249][] by [@wfdewith])
- Correctly calculate trailing whitespace for all lines ([#256][] by [@wfdewith])
- Strip whitespace following a newline when whitespace collapsing is enabled ([#254][] by [@nicoburns])
- Account for inline boxes when collapsing whitespace after newlines ([#280][] by [@nicoburns])

#### Fontique

- Skip adding font family as fallback if it has zero coverage for given script ([#182][] by [@richardhozak]

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
[@mwcampbell]: https://github.com/mwcampbell
[@nicoburns]: https://github.com/nicoburns
[@spirali]: https://github.com/spirali
[@tomcur]: https://github.com/tomcur
[@waywardmonkeys]: https://github.com/waywardmonkeys
[@wfdewith]: https://github.com/wfdewith
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
[#133]: https://github.com/linebender/parley/pull/133
[#137]: https://github.com/linebender/parley/pull/137
[#143]: https://github.com/linebender/parley/pull/143
[#146]: https://github.com/linebender/parley/pull/146
[#154]: https://github.com/linebender/parley/pull/154
[#163]: https://github.com/linebender/parley/pull/163
[#166]: https://github.com/linebender/parley/pull/166
[#169]: https://github.com/linebender/parley/pull/169
[#170]: https://github.com/linebender/parley/pull/170
[#176]: https://github.com/linebender/parley/pull/176
[#182]: https://github.com/linebender/parley/pull/182
[#187]: https://github.com/linebender/parley/pull/187
[#188]: https://github.com/linebender/parley/pull/188
[#191]: https://github.com/linebender/parley/pull/191
[#192]: https://github.com/linebender/parley/pull/192
[#194]: https://github.com/linebender/parley/pull/194
[#198]: https://github.com/linebender/parley/pull/198
[#201]: https://github.com/linebender/parley/pull/201
[#202]: https://github.com/linebender/parley/pull/202
[#207]: https://github.com/linebender/parley/pull/207
[#211]: https://github.com/linebender/parley/pull/211
[#212]: https://github.com/linebender/parley/pull/212
[#213]: https://github.com/linebender/parley/pull/213
[#223]: https://github.com/linebender/parley/pull/223
[#224]: https://github.com/linebender/parley/pull/224
[#245]: https://github.com/linebender/parley/pull/245
[#249]: https://github.com/linebender/parley/pull/249
[#250]: https://github.com/linebender/parley/pull/250
[#254]: https://github.com/linebender/parley/pull/254
[#256]: https://github.com/linebender/parley/pull/256
[#259]: https://github.com/linebender/parley/pull/259
[#264]: https://github.com/linebender/parley/pull/264
[#268]: https://github.com/linebender/parley/pull/268
[#271]: https://github.com/linebender/parley/pull/271
[#280]: https://github.com/linebender/parley/pull/280

[Unreleased]: https://github.com/linebender/parley/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/linebender/parley/releases/tag/v0.3.0
[0.2.0]: https://github.com/linebender/parley/releases/tag/v0.2.0
[0.1.0]: https://github.com/linebender/parley/releases/tag/v0.1.0
