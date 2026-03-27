// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Parley Core provides low level APIs for implementing text layout.
//!
//! ## Features
//!
//! - `std` (enabled by default): This is currently unused and is provided for forward compatibility.

// LINEBENDER LINT SET - lib.rs - v3
// See https://linebender.org/wiki/canonical-lints/
// These lints shouldn't apply to examples or tests.
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
// These lints shouldn't apply to examples.
#![warn(clippy::print_stdout, clippy::print_stderr)]
// Targeting e.g. 32-bit means structs containing usize can give false positives for 64-bit.
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]
// END LINEBENDER LINT SET
#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(missing_docs, reason = "We have many as-yet undocumented items.")]
#![expect(
    missing_debug_implementations,
    clippy::allow_attributes_without_reason,
    clippy::cast_possible_truncation,
    clippy::missing_assert_message,
    reason = "Deferred"
)]
#![expect(
    single_use_lifetimes,
    reason = "False positive: https://github.com/rust-lang/rust/issues/129255"
)]
#![no_std]
#[cfg(feature = "std")]
extern crate std;

extern crate alloc;

#[cfg(not(any(feature = "std", feature = "libm")))]
compile_error!("parley requires either the `std` or `libm` feature to be enabled");

pub use fontique;

mod analysis;
mod bidi;
mod builder;
mod context;
mod inline_box;
mod lru_cache;
mod resolve;
mod shape;
mod util;

#[cfg(test)]
mod tests;

pub mod style;

pub use analysis::{Boundary, cluster::Whitespace};

pub use linebender_resource_handle::FontData;

pub use builder::StyleRunBuilder;
pub use context::ParleyCoreContext;
pub use fontique::FontContext;
pub use inline_box::{InlineBox, InlineBoxKind};

pub use resolve::{Resolved, ResolvedDecoration, ResolvedProperty, ResolvedStyle, StyleRun};
pub use shape::ShapeSink;
pub use shape::data::*;

pub use style::*;

#[deprecated(
    note = "Old name for this type, use `parley::FontData` instead.",
    since = "0.6.0"
)]
pub type Font = FontData;
