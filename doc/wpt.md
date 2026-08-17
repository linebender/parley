# Running WPT tests against Parley (via Blitz)

[Blitz](https://github.com/DioxusLabs/blitz) is an HTML/CSS rendering engine that uses Parley
for all of its text layout. It ships a [Web Platform Tests](https://github.com/web-platform-tests/wpt)
(WPT) runner, and because the text-related WPT suites (`css/CSS2/text`, `css/css-text`, etc.) exercise line breaking, shaping, bidi, font fallback and font selection, this is a convenient way to test Parley against thousands of real-world text layout tests.

This document explains how to run the Blitz WPT runner against Parley.

## Prerequisites

1. **A clone of Parley** (this repository):
   ```sh
   git clone https://github.com/linebender/parley.git
   ```
2. **A clone of Blitz**:
   ```sh
   git clone https://github.com/DioxusLabs/blitz.git
   ```
3. **A clone of the WPT test suite** (large, a shallow clone is fine):
   ```sh
   git clone --depth 1 https://github.com/web-platform-tests/wpt.git
   ```

The layout assumed by the rest of this document is sibling directories:

```
~/code/blitz
~/code/parley
~/code/wpt
```

### You may also find the following tools useful:

- **The WPT cli** (an unofficial Rust CLI maintained by nicoburns)
   ```sh
   cargo install wpt --locked
   ```

## Pointing Blitz at your local Parley

Blitz normally depends on a released version of Parley from crates.io. To test local changes,
change the dependency to be a local path dependency in Blitz'a root/workspace `Cargo.toml`:

```toml
parley = { path = "../parley/parley" }
```

Note: use of `parley/parley` rather than plain `parley`. This is because the `parley` crate is in the `parley` directory within it's own repository.

### Version compatibility gotchas

- **It is generally best to remove the `version` specifier from the dependency specification** (as above). If `version` is present then Cargo will require that your local checkout of Parley's version matches. If is absent then no such check occurs.

- **Blitz usually tracks Parley *releases*, not Parley `main`.** If your branch is based on `main` and Parley's API has moved on since the last release, Blitz may fail to compile against it. Options:
  - Fix up the (usually small) API mismatches in Blitz locally, or
  - Check whether Blitz has a branch that already tracks a newer Parley.
  - Base your Parley branch on the branch/tag matching the release Blitz uses
    (e.g. `v0.11.x`), or

You can verify the patch took effect with:

```sh
cargo tree -p parley    # should print: parley vX.Y.Z (/path/to/your/parley/parley)
```

## Running the tests

The runner needs the `WPT_DIR` environment variable pointing at your WPT clone. From the Blitz
repo root:

```sh
WPT_DIR=../wpt cargo run -rp wpt css/css-text
```

You may wish to consider exporting the WPT_DIR environment variable from your zshrc or similar to avoid having to set it every time. You may also wish to install the [just](https://github.com/casey/just) task runner. With both of those adjustments, the above becomes simply:

```sh
just wpt css/css-text
```

The positional argument(s) are path filters relative to the WPT root. You can pass:

- a directory: `css/css-text/word-break`
- multiple suites: `css/css-text css/css-fonts`
- a single test file: `css/css-text/word-break/word-break-normal-ja-000.html`

If no filter is given, it defaults to `css/css-flexbox` and `css/css-grid` (layout suites),
so for Parley work you'll always want to pass a text-related filter.

### Suites most relevant to Parley

Core suites for layout:

| Suite | Exercises |
| --- | --- |
| `css/CSS2/text` | Basic (/old) tests for line breaking, `word-break`, `overflow-wrap`, `white-space`, `text-align`, letter/word spacing |
| `css/CSS2/bidi-text` | Older, more basic tests for bidi text |
| `css/css-text` | Advanced (/new) tests for line breaking, `word-break`, `overflow-wrap`, `white-space`, `text-align`, letter/word spacing |
| `css/css-inline` | Inline layout, baselines, `line-height`, `vertical-align` |
| `css/css-writing-modes` | Vertical text, bidi, `direction` |


Other suites we may be interested:

| Suite | Exercises |
| --- | --- |
| `css/css-fonts` | Font selection, fallback, `font-variant`, weights/styles (Fontique) |
| `css/css-ruby` | Ruby annotation layout (not yet implement in Parley) |
| `css/css-text-decor` | Underlines, `text-decoration`, `text-emphasis` |


Note that failures in these suites are not necessarily Parley bugs — the test may exercise
CSS features that Blitz doesn't implement (yet), or the bug may be in Blitz's inline layout
integration (`blitz-dom`'s "inline root" construction) rather than in Parley itself.

### Useful flags and environment variables

- `-v` / `--verbose`: print each test result as it completes (instead of a progress display).
- `RUST_LOG=info`: enable the runner's logging (it uses `env_logger`).
- `RAYON_NUM_THREADS=1` for single-threaded runs when debugging.

## Interpreting the output

You should get a line per-test like below:

```
[0011/1902] FAIL (0/1) css/css-text/bidi/bidi-lines-001.html (4ms) REF
[0012/1902] FAIL (0/1) css/css-text/bidi/bidi-lines-002.html (4ms) REF (D)
[0013/1902] PASS (1/1) css/css-text/bidi/bidi-tab-001.html (2ms) REF
[0014/1902] FAIL (0/1) css/css-text/bidi/empty-span-001.html (19ms) REF
[0015/1902] PASS (1/1) css/css-text/boundary-shaping/boundary-shaping-001.html (6ms) REF
[0016/1902] FAIL (0/1) css/css-text/boundary-shaping/boundary-shaping-002.html (9ms) REF
```

And at the end of a run you get a summary like:

```
 105 tests FOUND
   1 tests SKIPPED (0.95%)
 104 tests RUN (99.05%)
  39 tests PASSED (37.50% of run; 37.14% of found)
  65 tests FAILED (62.50% of run; 61.90% of found)

Of those tests which failed:
  22 do not use unsupported features
   4 use floats (F)
   9 use intrinsic size keywords (I)
  30 use script (X)
```

The runner supports three kinds of test:
- reftests (`REF`, image comparison against a reference page)
- attr tests (`ATT`, `checkLayout()`-style tests whose expectations are encoded
in `data-expected-*` attributes)
- crashtests (`CRA`, pass if they render without panicking).
- `testharness.js` tests (`HAR`) require a JavaScript engine. They are currently skipped if using Blitz `main`, but experimental support is available on the [js-wpt-runner](https://github.com/DioxusLabs/blitz/pull/738) branch.

The single-letter flags after each result (`F`, `I`, `C`, `D`, `W`, `X`, ...) mark tests that use features Blitz doesn't fully support (floats, intrinsic sizing keywords, calc, direction, writing modes, script).

When using the `main`-branch runner which does not support JavaScript, failures marked `X` (script) are often false failures (although some use trivial enough JS that they don't affect the test outcome). If you want to validate these tests, consider using the `js-wpt-runner` branch of Blitz as above.

### Artifacts in `wpt/output/`

Each run wipes and repopulates `wpt/output/` in the Blitz repo:

- `<test>.html-test.png`: Blitz's rendering of the test page
- `<test>.html-ref.png`: (or `-ref-N.png`) - rendering of the reference page(s)
- `<test>.html-diff.png`: pixel diff, written for failing comparisons
- `wptreport.json` - standard "WPT report" format, consumable by WPT tooling and dashboards.

## A typical Parley-change workflow

1. Set up the path dependency as above.
2. Export `$WPT_DIR` environment variable as above
3. Run the relevant suite **before** your change and save the report:
   ```sh
   cargo run -rp wpt css/css-text
   cp wpt/output/wptreport.json /tmp/before.json
   ```
4. Make your Parley change.
5. Re-run and diff:
   ```sh
   cargo run -rp wpt css/css-text
   wpt diff /tmp/before.json wpt/output/wptreport.json
   ```
6. For any regression, open the `-test.png`, `-ref.png` and `-diff.png` images for that test
   in `wpt/output/` to see what changed visually. The test itself lives in your WPT clone and
   can also be viewed at `https://wpt.live/<test path>` for comparison against real browsers.
