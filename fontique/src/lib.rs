// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Font enumeration and fallback.

// LINEBENDER LINT SET - lib.rs - v1
// See https://linebender.org/wiki/canonical-lints/
// These lints aren't included in Cargo.toml because they
// shouldn't apply to examples and tests
#![warn(unused_crate_dependencies)]
#![warn(clippy::print_stdout, clippy::print_stderr)]
// END LINEBENDER LINT SET
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(missing_debug_implementations)]
#![allow(missing_docs)]
#![allow(single_use_lifetimes)]
#![allow(unnameable_types)]
#![allow(unreachable_pub)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::exhaustive_enums)]
#![allow(clippy::partial_pub_fields)]
#![allow(clippy::shadow_unrelated)]
#![allow(clippy::use_self)]

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

mod source_cache;

pub use icu_locid::LanguageIdentifier as Language;
pub use peniko::Blob;

pub use attributes::{Attributes, FontStyle, FontWeight, FontWidth};
pub use collection::{Collection, CollectionOptions, Query, QueryFamily, QueryFont, QueryStatus};
pub use fallback::FallbackKey;
pub use family::{FamilyId, FamilyInfo};
pub use font::{AxisInfo, FontInfo, Synthesis};
pub use generic::GenericFamily;
pub use script::Script;
pub use source::{SourceId, SourceInfo, SourceKind};

pub use source_cache::{SourceCache, SourceCacheOptions};
