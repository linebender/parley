# Usage

```bash
$ cargo test
```

If a test fails, you can compare images in /parley/tests/current (images created by the current test)
and /parley/tests/snapshots (the accepted versions).

If you think that everything is ok, you can start tests as follows:

```bash
$ PARLEY_TEST="accept" cargo test
```

It will update snapshots of the failed tests.

## Report with diffs

If you want to create a report with image diffs use:

```bash
$ cargo xtask-test report
```
