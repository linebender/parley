// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Styled Text and Text Styles

// LINEBENDER LINT SET - lib.rs - v1
// See https://linebender.org/wiki/canonical-lints/
// These lints aren't included in Cargo.toml because they
// shouldn't apply to examples and tests
#![warn(unused_crate_dependencies)]
#![warn(clippy::print_stdout, clippy::print_stderr)]
// END LINEBENDER LINT SET
#![allow(elided_lifetimes_in_paths)]
#![allow(missing_docs)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::exhaustive_enums)]
#![allow(clippy::use_self)]

#[cfg(not(any(feature = "std", feature = "libm")))]
compile_error!("parley requires either the `std` or `libm` feature to be enabled");

extern crate alloc;

mod attributes;
mod font_family;
mod generic;

pub use attributes::{Stretch, Style, Weight};
pub use font_family::FontFamily;
pub use generic::GenericFamily;
