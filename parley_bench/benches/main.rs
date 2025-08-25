//! Parley benchmarks.

use tango_bench::{tango_benchmarks, tango_main};

use parley_bench::benches::{defaults, styled};

tango_benchmarks!(defaults(), styled());
tango_main!();
