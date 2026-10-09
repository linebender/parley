<!-- Instructions

This changelog follows the patterns described here: <https://keepachangelog.com/en/>.

Subheadings to categorize changes are `added, changed, deprecated, removed, fixed, security`.

-->

# Changelog

## [Unreleased]

This release has an [MSRV] of 1.88.

## [0.1.1] - 2026-10-09

This release has an [MSRV] of 1.88.

### Added

- `BidiLevel` to encode bidirectional text embedding levels. ([#710][] by [@tomcur][])
- `Script::is_cursive` returning whether a script is cursive. ([#728][] by [@tomcur][])  
  This can be used to decide, for example, whether to apply letter spacing.
- `NormalizedCoord` to encode normalized font coordinates. ([#813][] by [@nicoburns][])
- `LineBreak` to represent the CSS `line-break` property. ([#843][] by [@nicoburns][])

### Deprecated

- `Script::from_str_unchecked` has been deprecated. ([#677][] by [@DJMcNab][])  
  Use `Script::from_bytes` instead.

### Fixed

- `GenericFamily::parse` is now case-insensitive. ([#654][] by [@mvanhorn][])

## [0.1.0] - 2026-03-27

This release has an [MSRV] of 1.88.

### Added

This is lightweight crate that contains types representing text-related concepts such as `FontWeight`, `FontFamily`, `Script`, `Language`, etc.
It is developed as part of the Parley project but is intended to be widely useful for any Rust project working with text.
([#500][], [#501][], [#502][], [#505][], [#508][] by [@waywardmonkeys][])

[MSRV]: README.md#minimum-supported-rust-version-msrv

[@DJMcNab]: https://github.com/DJMcNab
[@mvanhorn]: https://github.com/mvanhorn
[@nicoburns]: https://github.com/nicoburns
[@tomcur]: https://github.com/tomcur
[@waywardmonkeys]: https://github.com/waywardmonkeys

[#500]: https://github.com/linebender/parley/pull/500
[#501]: https://github.com/linebender/parley/pull/501
[#502]: https://github.com/linebender/parley/pull/502
[#505]: https://github.com/linebender/parley/pull/505
[#508]: https://github.com/linebender/parley/pull/508
[#654]: https://github.com/linebender/parley/pull/654
[#677]: https://github.com/linebender/parley/pull/677
[#710]: https://github.com/linebender/parley/pull/710
[#728]: https://github.com/linebender/parley/pull/728
[#813]: https://github.com/linebender/parley/pull/813
[#843]: https://github.com/linebender/parley/pull/843

[Unreleased]: https://github.com/linebender/parley/compare/parlance-v0.1.1...HEAD
[0.1.1]: https://github.com/linebender/parley/compare/parlance-v0.1.0...parlance-v0.1.1
[0.1.0]: https://github.com/linebender/parley/compare/parlance-v0.0.0...parlance-v0.1.0
