# Vendored Unicode Conformance Test Data

This directory contains test data files from the Unicode Character Database,
pinned to Unicode **16.0.0** to match `icu_properties 2.x`'s bundled data.

## Files

| File | Source URL |
|------|------------|
| `BidiTest.txt` | <https://www.unicode.org/Public/16.0.0/ucd/BidiTest.txt> |
| `BidiCharacterTest.txt` | <https://www.unicode.org/Public/16.0.0/ucd/BidiCharacterTest.txt> |

## License

These files are copyright © 2024 Unicode, Inc., and are distributed under the
[Unicode License v3](https://www.unicode.org/license.txt) (Unicode-3.0).

## Version policy

Bump these files in lockstep with `icu_properties`'s Unicode version.
The conformance harness (`parley_core/tests/bidi_conformance.rs`) asserts
the file header version equals `EXPECTED_UCD_VERSION` and will fail with a
clear message if the two drift out of sync.
