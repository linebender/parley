# parley_tests

Integration test suite for the Parley text layout library.

## Overview

This crate contains the integration test suite for `parley`. It follows a similar architecture to
[`vello_sparse_tests`](https://github.com/linebender/vello/tree/main/sparse_strips/vello_sparse_tests) and uses snapshot
testing to validate layout correctness.

## Running Tests

```bash
$ cargo test -p parley_tests
```

If a test fails, you can compare images in /parley/tests/current (images created by the current test)
and /parley/tests/snapshots (the accepted versions).

If you think that everything is ok, you can start tests as follows:

```bash
$ PARLEY_TEST="accept" cargo test -p parley_tests
```

It will update snapshots of the failed tests.

## Usage of xtask when a test fails

After some tests fail, you may run the following for generating a Kompari HTML report:

```bash
cargo xtask report
```

or start an interactive test blessing

```bash
cargo xtask review
```

## Detect dead snapshots

The following command shows snapshots that are not used in any test. The command also allows to delete such snapshots.

```bash
cargo xtask dead-snapshots
```

## Matching Chrome in line height calculations

The `lines.html` file can be used to generate images roughly similar to the ones in `test_lines.rs`.
It can be used to verify existing test goals or to create additional tests.

## Matching Chrome in CJ justification

Open `justification.html` to compare the single-line fixtures in `tests/justification.rs`.
It uses `text-align-last: justify` and widths twice the natural text advance, matching the Rust fixtures.
The Google Fonts Noto Sans JP font substitutes for the local Noto CJK SC subset, so outlines may differ.
Mixed-direction cases are best-effort and do not claim Chrome parity.

Parley and Chrome agree on the structure, i.e., that there is a justification opportunity after each word separator, and at each boundary where at least one side is "expandable".
However, Parley determines which characters are expandable from Unicode properties, following the block scripts of [CSS Text Level 3, Appendix D](https://www.w3.org/TR/css-text-3/#script-groups) (see `parley_data::Properties::block_script`), with emoji being expandable if they are presented as emoji.
Chrome instead uses a fixed list of characters, which leads to these intentional differences:

| Case                                                                                            | Parley         | Chrome         |
|-------------------------------------------------------------------------------------------------|----------------|----------------|
| Pictographs presented as text which aren't wide (`✌`, `☎`, `🖥`)                                 | Not expandable | Expandable     |
| Symbols from legacy East Asian character sets which aren't wide (`①`, `■`, `★`, `※`)            | Not expandable | Expandable     |
| Emoji presentation sequences of other characters (`©️`, `#️⃣`)                                     | Expandable     | Not expandable |
| Fullwidth currency symbols (`￥`, `＄`, `￦`)                                                      | Not expandable | Expandable     |
| Enclosed and halfwidth Hangul (`㈀`, `ﾡ`)                                                        | Not expandable | Expandable     |
| Halfwidth punctuation and symbols (`｡`, `｢`, `｣`, `､`, `･`, `ｰ`)                                  | Not expandable | Expandable     |
| Yi, ideographs beyond plane 2, kana extensions (`ㇰ`), and some wide punctuation (`〰`, `－`, `；`) | Expandable     | Not expandable |

`justification.html` has a "Differences from Chrome" section to observe these.
Most of these only matter next to a character which isn't itself expandable (e.g., `5★` or `￥100`), as a boundary only needs one expandable side.
