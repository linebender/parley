# Parley Benchmarks

A suite of benchmarks used to evaluate Parley performance.

## Setup

Install `cargo-export` via `cargo install cargo-export`.

## Usage

```shell
# Capture a baseline via
cargo export target/benchmarks -- bench --bench=main

# Apply changes to Parley

# Compare changes with baseline
cargo bench -q --bench=main -- compare target/benchmarks/main
```
