// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Font enumeration and fallback.

#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]

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

pub use attributes::{Attributes, Stretch, Style, Weight};
pub use collection::{Collection, CollectionOptions, Query, QueryFamily, QueryFont, QueryStatus};
pub use fallback::FallbackKey;
pub use family::{FamilyId, FamilyInfo};
pub use font::{AxisInfo, FontInfo, Synthesis};
pub use generic::GenericFamily;
pub use script::Script;
pub use source::{SourceId, SourceInfo, SourceKind};

pub use source_cache::{SourceCache, SourceCacheOptions};
