use tango_bench::{IntoBenchmarks, benchmark_fn, tango_benchmarks, tango_main};

use parley_bench::default_style::default_style;

tango_benchmarks!(default_style());
tango_main!();
