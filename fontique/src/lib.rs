// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Font enumeration and fallback.

// LINEBENDER LINT SET - lib.rs - v4
// See https://linebender.org/wiki/canonical-lints/
// These lints shouldn't apply to examples or tests.
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
// These lints shouldn't apply to examples.
#![warn(clippy::print_stdout, clippy::print_stderr)]
// Targeting e.g. 32-bit means structs containing usize can give false positives for 64-bit.
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]
// END LINEBENDER LINT SET
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(unsafe_code, reason = "We access platform libraries using ffi.")]
#![allow(missing_docs, reason = "We have many as-yet undocumented items.")]
#![expect(
    missing_debug_implementations,
    unnameable_types,
    unreachable_pub,
    clippy::allow_attributes_without_reason,
    clippy::cast_possible_truncation,
    clippy::shadow_unrelated,
    reason = "Deferred"
)]
#![allow(
    single_use_lifetimes,
    reason = "False positive: https://github.com/rust-lang/rust/issues/129255"
)]

#[cfg(not(any(feature = "std", feature = "libm")))]
compile_error!("fontique requires either the `std` or `libm` feature to be enabled");

extern crate alloc;

mod attributes;
mod backend;
mod charmap;
mod collection;
mod fallback;
mod family;
mod family_name;
mod font;
mod generic;
mod matching;
mod scan;
mod script;
mod source;

mod source_cache;

pub use icu_locale_core::LanguageIdentifier as Language;
pub use linebender_resource_handle::Blob;

pub use attributes::{Attributes, FontStyle, FontWeight, FontWidth};
pub use charmap::{Charmap, CharmapIndex};
pub use collection::{Collection, CollectionOptions, Query, QueryFamily, QueryFont, QueryStatus};
pub use fallback::FallbackKey;
pub use family::{FamilyId, FamilyInfo};
pub use font::{AxisInfo, FontInfo, FontInfoOverride, Synthesis};
pub use generic::GenericFamily;
pub use script::Script;
pub use source::{SourceId, SourceInfo, SourceKind};

#[cfg(all(feature = "system", target_vendor = "apple"))]
use objc2 as _;
pub use source_cache::{SourceCache, SourceCacheOptions};
