// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Parley benchmarks.

use tango_bench::{tango_benchmarks, tango_main};

use parley_bench::benches::{defaults, styled};
use parley_bench::fontique_benches::system_fonts_init;

tango_benchmarks!(defaults(), styled(), system_fonts_init());
tango_main!();
