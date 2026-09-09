// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Parley benchmarks.

use tango_bench::tango_benchmarks;

use parley_bench::benches::{
    content_widths, defaults, iterate_glyph_runs, line_breaking, long_line, page,
    repeated_justification, spacing, styled,
};
use parley_bench::fontique_benches::system_fonts_init;

tango_benchmarks!(
    defaults(),
    styled(),
    iterate_glyph_runs(),
    content_widths(),
    spacing(),
    repeated_justification(),
    line_breaking(),
    long_line(),
    page(),
    system_fonts_init()
);
