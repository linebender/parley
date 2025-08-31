<!-- Instructions

This changelog follows the patterns described here: <https://keepachangelog.com/en/>.

Subheadings to categorize changes are `added, changed, deprecated, removed, fixed, security`.

-->

# Changelog

The latest published Parley release is [0.5.0](#050---2025-06-01) which was released on 2025-06-01.
You can find its changes [documented below](#050---2025-06-01).

## [Unreleased]

This release has an [MSRV] of 1.82.

### Added

#### Parley

- Shift-click support through `Selection::shift_click_extension` and `PlainEditorDriver::shift_click_extension`. ([#385][] by [@kekelp][])
- Benchmarks. ([#405]() by [@taj-p][])

#### Fontique

- Cache character mapping metadata for each font to improve performance of font selection. ([#413][] by [@dfrg][])

### Changed

#### Parley

- `Alignment`` variants have been renamed to better match CSS. `Alignment::Justified` is now `Alignment::Justify` and `Alignment::Middle` is now `Alignment::Center`. ([#389][] by [@waywardmonkeys][])
- Updated to `accesskit` 0.21. ([#390][] by [@mwcampbell][])
- Uses `HarfRust` for text shaping ([[#400][] by [@taj-p][]).

#### Fontique

- The fontconfig backend, used to enumerate system fonts on Linux, has been rewritten to call into the system's fontconfig library instead of parsing fontconfig's configuration files itself. This should significantly improve the behavior of system fonts and generic families on Linux. ([#378][] by [@valadaptive][])

### Fixed

#### Parley

- Selection extension moves the focus to the side being extended. ([#385][] by [@kekelp][])
- Ranged builder default style not respecting `scale`. ([#368][] by [@xStrom][])
- Cluster source character not correct ([[#402][] by [@taj-p][]).

#### Fontique

- Font family name aliases (secondary names for font families, often in another language) not being registered. ([#380][] by [@valadaptive][])

## [0.5.0] - 2025-06-01

This release has an [MSRV] of 1.82.

### Added

#### Parley

- Editor features required by Android IME. ([#334][] by [@mwcampbell][])

### Changed

#### Parley

- Breaking change: `Layout::min_content_width`, `Layout::max_content_width`, and `Layout::content_widths` have been replaced with `Layout::calculate_content_widths`, which does not internally cache the widths. This means that `Layout` is now `Sync` again, but callers will have to cache the min and max content widths themselves. ([#353][] by [@valadaptive][])
- Breaking change: the line height style property (`StyleProperty::LineHeight` and the `line_height` field on `TextStyle`) is now a `LineHeight` enum that allows you to specify absolute, font-size-relative, and font-metrics-relative line heights.
  Previously, it was always font-size-relative. ([#362][] by [@valadaptive][])
  - The default line height was previously `LineHeight::FontSizeRelative(1.0)` if you used `RangedStyleBuilder`, or `LineHeight::FontSizeRelative(1.2)` if you used `TreeStyleBuilder`.
    It is now `LineHeight::MetricsRelative(1.0)` in both cases.
    This will affect layout if you don't specify your own line height.
- Breaking change: `{RangedBuilder, TreeBuilder}::{build_into, build}` methods now consume `self`. ([#369][] by [@dhardy][])

## [0.4.0] - 2025-05-08

This release has an [MSRV] of 1.82.

### Migration

Quantization of vertical layout metrics is now optional.
For an easy upgrade we recommend enabling it by setting `quantize` to `true` when calling [`LayoutContext::ranged_builder`](https://docs.rs/parley/0.4.0/parley/struct.LayoutContext.html#method.ranged_builder) or [`LayoutContext::tree_builder`](https://docs.rs/parley/0.4.0/parley/struct.LayoutContext.html#method.tree_builder).

### Added

#### Parley

- Option to skip quantization of vertical layout metrics for advanced rendering use cases. ([#297][] by [@valadaptive][], [#344][] by [@xStrom][])
- The `WordBreak` and `OverflowWrap` style properties for controlling line wrapping. ([#315][] by [@valadaptive][])
- `PlainEditor` methods `raw_selection` and `raw_text`. ([#316][], [#317][] by [@mwcampbell][])
- `PlainEditor::selection_geometry_with`, the equivalent of `Selection::geometry_with` method. ([#318][] by [@valadaptive][])
- `BreakLines::is_done` method to check if all the text has been placed into lines. ([#319][] by [@valadaptive][])

### Changed

#### Parley

- Breaking change: `Selection::geometry`, `Selection::geometry_with`, and `PlainEditor::selection_geometry` now include the line indices that the selection rectangles belong to. ([#318][] by [@valadaptive][])
- Updated to `accesskit` 0.19. ([#294][] by [@waywardmonkeys][], [#348][] by [@xStrom][])
- Now displaying selected newlines as whitespace in the selection highlight. ([#296][] by [@valadaptive][])
- Made `BreakReason` public. ([#300][] by [@valadaptive][])

#### Fontique

- Breaking change: `Collection::register_fonts` now takes a `Blob<u8>` instead of a `Vec<u8>`. ([#306][] by [@valadaptive][])
- Breaking change: `Collection::register_fonts` now takes an optional second parameter which allows overriding the metadata used for matching the font. ([#312][] by [@valadaptive][])

### Fixed

#### Parley

- Text editing for layouts which contain inline boxes. ([#299][] by [@valadaptive][])
- Cursor navigation in RTL text sometimes getting stuck within a line. ([#331][] by [@valadaptive][])
- Using `Layout::align` on an aligned layout without breaking lines again. ([#342][] by [@xStrom][])
- Selection box height going below ascent + descent with small line heights. ([#344][] by [@xStrom][])
- Rounding error accumulation of vertical layout metrics. ([#344][] by [@xStrom][])

#### Fontique

- Panic on macOS when running in debug mode. ([#335][] by [@NoahR02][])

## [0.3.0] - 2025-02-27

This release has an [MSRV] of 1.82.

### Added

#### Parley

- `Generation` on `PlainEditor` to help implement lazy drawing. ([#143][] by [@xorgy][])
- Support for preedit for input methods in `PlainEditor`. ([#192][] by [@DJMcNab][], [#198][] by [@tomcur][])
- `PlainEditor` method to get a cursor area for use by the platform's input method. ([#224][] by [@tomcur][])
- `Layout` methods to calculate minimum and maximum content widths. ([#259][] by [@wfdewith][])
- `PlainEditor` now implements `Clone`. ([#133][] by [@nicoburns][])
- Navigation and byte selection operations for `PlainEditor`. ([#146][] by [@xorgy][])
- AccessKit integration. ([#166][] by [@mwcampbell][])
- `first_style` method to `Cluster`. ([#264][] by [@nicoburns][])

#### Fontique

- `FontStretch`, `FontStyle`, and `FontWeight` get helper functions `from_fontconfig`. ([#212][] by [@waywardmonkeys][])
- Impl `bytemuck` traits for `GenericFamily`. ([#213][] by [@waywardmonkeys][])

### Changed

#### Parley

- Breaking change: The cursor API has been completely reworked. ([#170][] by [@dfrg][])
- Breaking change: `PlainEditor`s API is now method-based rather than enum based. ([#154][] by [@mwcampbell][])
- Breaking change: `PlainEditor`'s semantics are no longer transactional. ([#192][] by [@DJMcNab][])
- Breaking change: `Alignment::Start` and `Alignment::End` now depend on text base direction.
  `Alignment::Left` and `Alignment::Right` are introduced for text direction-independent alignment. ([#250][] by [@tomcur][])
- Breaking change: `Layout` is no longer `Sync`. ([#259][] by [@wfdewith][])
- Breaking change: `PlainEditor`'s width is now `Option<f32>` rather than `f32`. ([#137][] by [@nicoburns][])
- Breaking change: Make alignment when free space is negative configurable. ([#241][] by [@nicoburns][])
- Allow Bidi `base_level` to be determined from text. ([#245][] by [@tomcur][])

#### Fontique

- Breaking change: `Stretch`, `Style`, and `Weight` renamed to `FontWidth`, `FontStyle`, `FontWeight`. ([#211][], [#223][] by [@waywardmonkeys][])
- Depend on `read-fonts` instead of `skrifa`. ([#162][] by [@nicoburns][])

### Fixed

#### Parley

- Alignment of right-to-left text. ([#250][], [#268][] by [@tomcur][])
- Performing line breaking or aligning a layout again, after justified alignment had been applied previously. ([#271][] by [@tomcur][])
- Placement of inline boxes. ([#163][] by [@spirali][])
- Cursor position for lines that are not left-aligned. ([#169][] by [@mwcampbell][])
- `Cursor::from_point` not using the line's offset. ([#176][] by [@DJMcNab][])
- Off-by-one error in `PlainEditor::cursor_at`. ([#187][] by [@tomcur][])
- Binary search in `Layout::line_for_offset`. ([#188][] by [@tomcur][])
- Whitespace collapsing at the start of inline spans. ([#191][] by [@nicoburns][])
- Collapsing selection. ([#201][] by [@tomcur][])
- Affinities not being ignored in `Selection::is_collapsed`. ([#202][] by [@tomcur][])
- Misc. inline box layout issues. ([#207][] by [@nicoburns][])
- Linebreaking for lines without text runs. ([#249][] by [@wfdewith][])
- Calculating trailing whitespace for all lines. ([#256][] by [@wfdewith][])
- Strip whitespace following a newline when whitespace collapsing is enabled. ([#254][] by [@nicoburns][])
- Account for inline boxes when collapsing whitespace after newlines. ([#280][] by [@nicoburns][])

#### Fontique

- Skip adding font family as fallback if it has zero coverage for given script. ([#182][] by [@richardhozak][])

## [0.2.0] - 2024-10-10

This release has an [MSRV] of 1.75.

### Added

#### Parley

- Example using `tiny-skia` which renders into a PNG. ([#55][] by [@nicoburns][])
    - Breaking change: There is now a blanket implementation for `Brush`.
- A Swash example which renders into a PNG. ([#54][] by [@nicoburns][])
- An example with Vello on Winit which shows a basic text editor .([#106][] by [@dfrg][])
- `PlainEditor`, a basic action-based text editor based on Parley `Selection` and `Cursor`. ([#126][] by [@xorgy][])
- Tree style builder. ([#76][] by [@nicoburns][])
- Conversions for `FontFamily`, `FontStack`, and `StyleProperty` to make styling more ergonomic. ([#129][] by [@xorgy][])

### Changed

#### General

- Repository layout updated to match Linebender standard. ([#59][] by [@waywardmonkeys][])

#### Parley

- Emoji clusters now get an Emoji family added by default. ([#56][] by [@dfrg][])
- Style builders now accept `Into<StyleProperty<'a, B: Brush>>` so you can push a `GenericFamily` or `FontStack` directly. ([#129][] by [@xorgy][])

#### Fontique

- Removed unsafe code from fontconfig cache. ([#78][] by [@waywardmonkeys][])
- Switched to `windows-rs` for `dwrite` backend. ([#85][] by [@dfrg][])

### Fixed

#### Fontique

- Search correct paths for fonts on Apple platforms. ([#71][] by [@waywardmonkeys][])

### Removed

#### Fontique

- Breaking change: removed conversion to/from `icu_properties::Script` for `fontique::Script`. ([#72][] by [@waywardmonkeys][])
    - This can be restored by using the `icu_properties` feature of `fontique`.

## [0.1.0] - 2024-05-01

This release has an [MSRV][] of 1.70.

- Initial release

[MSRV]: README.md#minimum-supported-rust-version-msrv

[@dfrg]: https://github.com/dfrg
[@DJMcNab]: https://github.com/DJMcNab
[@mwcampbell]: https://github.com/mwcampbell
[@nicoburns]: https://github.com/nicoburns
[@NoahR02]: https://github.com/NoahR02
[@spirali]: https://github.com/spirali
[@tomcur]: https://github.com/tomcur
[@valadaptive]: https://github.com/valadaptive
[@waywardmonkeys]: https://github.com/waywardmonkeys
[@wfdewith]: https://github.com/wfdewith
[@xorgy]: https://github.com/xorgy
[@xStrom]: https://github.com/xStrom
[@dhardy]: https://github.com/dhardy
[@kekelp]: https://github.com/kekelp
[@taj-p]: https://github.com/taj-p

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
[#162]: https://github.com/linebender/parley/pull/162
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
[#241]: https://github.com/linebender/parley/pull/241
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
[#294]: https://github.com/linebender/parley/pull/294
[#296]: https://github.com/linebender/parley/pull/296
[#297]: https://github.com/linebender/parley/pull/297
[#299]: https://github.com/linebender/parley/pull/299
[#300]: https://github.com/linebender/parley/pull/300
[#306]: https://github.com/linebender/parley/pull/306
[#312]: https://github.com/linebender/parley/pull/312
[#315]: https://github.com/linebender/parley/pull/315
[#316]: https://github.com/linebender/parley/pull/316
[#317]: https://github.com/linebender/parley/pull/317
[#318]: https://github.com/linebender/parley/pull/318
[#319]: https://github.com/linebender/parley/pull/319
[#331]: https://github.com/linebender/parley/pull/331
[#334]: https://github.com/linebender/parley/pull/334
[#335]: https://github.com/linebender/parley/pull/335
[#342]: https://github.com/linebender/parley/pull/342
[#344]: https://github.com/linebender/parley/pull/344
[#348]: https://github.com/linebender/parley/pull/348
[#353]: https://github.com/linebender/parley/pull/353
[#362]: https://github.com/linebender/parley/pull/362
[#368]: https://github.com/linebender/parley/pull/368
[#369]: https://github.com/linebender/parley/pull/369
[#378]: https://github.com/linebender/parley/pull/378
[#380]: https://github.com/linebender/parley/pull/380
[#385]: https://github.com/linebender/parley/pull/385
[#389]: https://github.com/linebender/parley/pull/389
[#390]: https://github.com/linebender/parley/pull/390
[#400]: https://github.com/linebender/parley/pull/400
[#402]: https://github.com/linebender/parley/pull/402
[#405]: https://github.com/linebender/parley/pull/405
[#413]: https://github.com/linebender/parley/pull/413

[Unreleased]: https://github.com/linebender/parley/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/linebender/parley/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/linebender/parley/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/linebender/parley/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/linebender/parley/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/linebender/parley/compare/v0.0.0...v0.1.0
