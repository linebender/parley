// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Font enumeration and fallback.

// TODO: Remove this dead code allowance and hide the offending code behind the std feature gate.
#![cfg_attr(not(feature = "std"), allow(dead_code))]
#![cfg_attr(all(not(feature = "std"), not(test)), no_std)]

#[cfg(not(any(feature = "std", feature = "libm")))]
compile_error!("fontique requires either the `std` or `libm` feature to be enabled");

extern crate alloc;

mod attributes;
mod backend;
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

#[cfg(feature = "std")]
mod source_cache;

pub use icu_locid::LanguageIdentifier as Language;
pub use peniko::Blob;

pub use attributes::{Attributes, Stretch, Style, Weight};
pub use collection::{Collection, CollectionOptions, Query, QueryFamily, QueryFont, QueryStatus};
pub use fallback::FallbackKey;
pub use family::{FamilyId, FamilyInfo};
pub use font::{AxisInfo, FontInfo, Synthesis};
pub use generic::GenericFamily;
pub use script::Script;
pub use source::{SourceId, SourceInfo, SourceKind};

#[cfg(feature = "std")]
pub use source_cache::{SourceCache, SourceCacheOptions};
