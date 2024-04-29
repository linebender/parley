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
    family_name::{FamilyName, FamilyNameMap},
    generic::GenericFamilyMap,
    scan, FallbackKey, FamilyId, FamilyInfo, FontInfo, GenericFamily, Script, SourceInfo,
};

#[cfg(feature = "std")]
#[allow(unused_imports)]
use super::source::SourcePathMap;

#[cfg(feature = "system")]
pub use system::SystemFonts;

#[cfg(not(feature = "system"))]
pub use null_backend::SystemFonts;

#[cfg(not(feature = "system"))]
mod null_backend {
    use super::{FamilyNameMap, GenericFamilyMap};
    use alloc::sync::Arc;

    #[derive(Default)]
    pub struct SystemFonts {
        pub name_map: Arc<FamilyNameMap>,
        pub generic_families: Arc<GenericFamilyMap>,
    }

    impl SystemFonts {
        pub fn new() -> Self {
            Self::default()
        }
    }
}
