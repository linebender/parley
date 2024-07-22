// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Rich text layout.

// TODO: Remove this dead code allowance and hide the offending code behind the std feature gate.
#![cfg_attr(not(feature = "std"), allow(dead_code))]
#![cfg_attr(all(not(feature = "std"), not(test)), no_std)]

#[cfg(not(any(feature = "std", feature = "libm")))]
compile_error!("parley requires either the `std` or `libm` feature to be enabled");

extern crate alloc;

/// A reference counted string slice.
///
/// This is a data-friendly way to represent strings in Masonry. Unlike `String`
/// it cannot be mutated, but unlike `String` it can be cheaply cloned.
pub type ArcStr = std::sync::Arc<str>;

pub use fontique;
pub use swash;

mod bidi;
pub mod font;
mod inline_box;
mod resolve;
mod shape;
mod swash_convert;
mod util;

pub mod builder;
pub mod context;
pub mod layout;
pub mod style;

pub use peniko::Font;

pub use context::LayoutContext;
pub use font::FontContext;
pub use inline_box::InlineBox;
pub use layout::Layout;
