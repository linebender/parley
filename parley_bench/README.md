# Parley Benchmarks

A suite of benchmarks used to evaluate Parley performance.

This uses [Tango](https://github.com/bazhenov/tango) to perform paired benchmarking.

## Setup

Install [`cargo-export`](https://crates.io/crates/cargo-export) via:

```sh
cargo install cargo-export
```

## Usage

```shell
# Capture a baseline via
RUSTFLAGS="-Cllvm-args=-align-all-functions=6" cargo export target/benchmarks -- bench --bench=main

# Apply changes to Parley

# Compare changes with baseline
RUSTFLAGS="-Cllvm-args=-align-all-functions=6" cargo bench -q --bench=main -- compare ../target/benchmarks/main
```

We've found that setting the `align-all-functions=6` improves run-to-run consistency.
