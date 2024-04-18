// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

// TODO: Remove this dead code allowance and hide the offending code behind the std feature gate.
#![cfg_attr(not(feature = "std"), allow(dead_code))]

extern crate alloc;

pub use swash;

mod bidi;
pub mod font;
mod resolve;
mod shape;
mod swash_convert;
mod util;

pub mod context;
pub mod fontique;
pub mod layout;
pub mod style;

pub use peniko::Font;

pub use context::LayoutContext;
pub use font::FontContext;
pub use layout::Layout;
