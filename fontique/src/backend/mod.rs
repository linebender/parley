// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! System backends.

#[cfg(all(feature = "system", target_os = "windows"))]
#[path = "dwrite.rs"]
mod system;

#[cfg(all(feature = "system", target_vendor = "apple"))]
#[path = "coretext.rs"]
mod system;

#[cfg(all(feature = "system", target_os = "linux"))]
#[path = "fontconfig/mod.rs"]
mod system;

#[cfg(all(feature = "system", target_os = "android"))]
#[path = "android.rs"]
mod system;

#[allow(unused_imports)]
use super::{
    FallbackKey, FamilyId, FamilyInfo, FontInfo, GenericFamily, Script, SourceInfo,
    family_name::{FamilyName, FamilyNameMap},
    generic::GenericFamilyMap,
    scan,
};

#[cfg(feature = "std")]
#[allow(unused_imports)]
use super::source::SourcePathMap;

#[cfg(feature = "system")]
pub(crate) use system::SystemFonts;

#[cfg(not(feature = "system"))]
pub(crate) use null_backend::SystemFonts;

#[cfg(not(feature = "system"))]
mod null_backend {
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
    }
}
