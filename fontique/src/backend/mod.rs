// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! System backends.

#[cfg(all(feature = "system", target_os = "windows"))]
#[path = "dwrite.rs"]
mod system;

#[cfg(all(feature = "system", target_vendor = "apple"))]
#[path = "coretext.rs"]
mod system;

#[cfg(all(feature = "system", any(target_os = "linux", target_os = "freebsd")))]
#[path = "fontconfig.rs"]
mod system;

#[cfg(all(feature = "system", target_os = "android"))]
#[path = "android.rs"]
mod system;

#[allow(unused_imports)]
use super::{
    FallbackKey, FamilyId, FamilyInfo, FontInfo, GenericFamily, Language, Script, ScriptExt,
    SourceInfo,
    family_name::{FamilyName, FamilyNameMap},
    generic::GenericFamilyMap,
    scan,
};

#[cfg(feature = "std")]
#[allow(unused_imports)]
use super::source::SourcePathMap;

/// An ordered list of fallback families returned by a system font backend.
#[cfg(feature = "system")]
pub(crate) type FallbackFamilies = smallvec::SmallVec<[FamilyId; 4]>;

pub(crate) use system::SystemFonts;

// Dummy system font backend for targets like wasm32-unknown-unknown
#[cfg(any(
    not(feature = "system"),
    not(any(
        target_os = "windows",
        target_os = "linux",
        target_os = "freebsd",
        target_os = "android",
        target_vendor = "apple"
    ))
))]
mod system {
    #[cfg(feature = "system")]
    use super::{FallbackFamilies, FallbackKey, FamilyId, FamilyInfo};
    use super::{FamilyNameMap, GenericFamilyMap};
    use alloc::sync::Arc;

    #[derive(Default)]
    pub(crate) struct SystemFonts {
        pub(crate) name_map: Arc<FamilyNameMap>,
        pub(crate) generic_families: Arc<GenericFamilyMap>,
    }

    impl SystemFonts {
        pub(crate) fn new() -> Self {
            Self::default()
        }

        #[cfg(feature = "system")]
        pub(crate) fn family(&mut self, _id: FamilyId) -> Option<FamilyInfo> {
            None
        }

        #[cfg(feature = "system")]
        pub(crate) fn fallback(&mut self, _key: impl Into<FallbackKey>) -> FallbackFamilies {
            FallbackFamilies::new()
        }

        #[cfg(feature = "system")]
        pub(crate) fn fallback_for_text(
            &mut self,
            _text: &str,
            _locale: Option<&str>,
        ) -> Option<FamilyId> {
            None
        }
    }
}
