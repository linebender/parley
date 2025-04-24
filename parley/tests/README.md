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

The following command shows snapshots that are not used in any test. The command also allows to delete such snaphosts.

```bash
cargo xtask dead-snapshots 
```

