// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Parley benchmarks.

use tango_bench::tango_benchmarks;

use parley_bench::benches::{
    defaults, iterate_styled_items, long_line, repeated_justification, spacing, styled,
};
use parley_bench::fontique_benches::system_fonts_init;

tango_benchmarks!(
    defaults(),
    styled(),
    iterate_styled_items(),
    spacing(),
    repeated_justification(),
    long_line(),
    system_fonts_init()
);
