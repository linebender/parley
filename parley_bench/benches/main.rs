// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Parley benchmarks.

use tango_bench::tango_benchmarks;

use parley_bench::benches::{defaults, styled, tracked};
use parley_bench::fontique_benches::system_fonts_init;

tango_benchmarks!(defaults(), tracked(), styled(), system_fonts_init());
