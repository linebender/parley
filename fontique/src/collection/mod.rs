// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Support for working with font collections.

mod query;

pub use query::{Query, QueryFamily, QueryFont, QueryStatus};

#[cfg(feature = "std")]
use super::SourceCache;

use super::{
    backend::SystemFonts,
    fallback::{FallbackKey, FallbackMap},
    family::{FamilyId, FamilyInfo},
    family_name::{FamilyName, FamilyNameMap},
    font::FontInfo,
    generic::GenericFamilyMap,
    source::{SourceId, SourceInfo, SourceKind},
    Blob, GenericFamily, Script,
};
use alloc::{string::String, sync::Arc, vec::Vec};
use core::sync::atomic::AtomicU64;
use hashbrown::HashMap;
#[cfg(feature = "std")]
use std::sync::{atomic::Ordering, Mutex};

type FamilyMap = HashMap<FamilyId, Option<FamilyInfo>>;

/// Options for a font collection.
#[derive(Copy, Clone, Debug)]
pub struct CollectionOptions {
    /// If true, the font collection will use a secondary shared store
    /// guaranteeing that any changes to the collection will be
    /// visible to all clones.
    ///
    /// If the font collection will be used by a single thread, this is
    /// pure overhead and should be disabled.
    ///
    /// The default value is false.
    pub shared: bool,

    /// If true, the font collection will provide access to system fonts
    /// using platform specific APIs.
    ///
    /// The default value is true.
    pub system_fonts: bool,
}

impl Default for CollectionOptions {
    fn default() -> Self {
        Self {
            shared: false,
            system_fonts: true,
        }
    }
}

/// Collection of fonts.
#[derive(Clone)]
pub struct Collection {
    inner: Inner,
    query_state: query::QueryState,
}

impl Collection {
    /// Creates a new collection with the given options.
    pub fn new(options: CollectionOptions) -> Self {
        Self {
            inner: Inner::new(options),
            query_state: Default::default(),
        }
    }

    /// Returns an iterator over all available family names in the collection.
    ///
    /// This includes both system and registered fonts.
    pub fn family_names(&mut self) -> impl Iterator<Item = &str> + '_ + Clone {
        self.inner.family_names()
    }

    /// Returns the family identifier for the given family name.
    pub fn family_id(&mut self, name: &str) -> Option<FamilyId> {
        self.inner.family_id(name)
    }

    /// Returns the family name for the given family identifier.
    pub fn family_name(&mut self, id: FamilyId) -> Option<&str> {
        self.inner.family_name(id)
    }

    /// Returns the family object for the given family identifier.
    pub fn family(&mut self, id: FamilyId) -> Option<FamilyInfo> {
        self.inner.family(id)
    }

    /// Returns the family object for the given name.
    pub fn family_by_name(&mut self, name: &str) -> Option<FamilyInfo> {
        self.inner.family_by_name(name)
    }

    /// Returns an iterator over the family identifiers for the given
    /// generic family.
    pub fn generic_families(
        &mut self,
        family: GenericFamily,
    ) -> impl Iterator<Item = FamilyId> + '_ + Clone {
        self.inner.generic_families(family)
    }

    /// Replaces the set of family identifiers associated with the given generic
    /// family.
    pub fn set_generic_families(
        &mut self,
        generic: GenericFamily,
        families: impl Iterator<Item = FamilyId>,
    ) {
        self.inner.set_generic_families(generic, families);
    }

    /// Appends the set of family identifiers to the given generic family.
    pub fn append_generic_families(
        &mut self,
        generic: GenericFamily,
        families: impl Iterator<Item = FamilyId>,
    ) {
        self.inner.append_generic_families(generic, families);
    }

    /// Returns an iterator over the fallback families for the given
    /// key.
    pub fn fallback_families(
        &mut self,
        key: impl Into<FallbackKey>,
    ) -> impl Iterator<Item = FamilyId> + '_ + Clone {
        self.inner.fallback_families(key)
    }

    /// Replaces the set of family identifiers associated with the fallback
    /// key.
    pub fn set_fallbacks(
        &mut self,
        key: impl Into<FallbackKey>,
        families: impl Iterator<Item = FamilyId>,
    ) -> bool {
        self.inner.set_fallbacks(key, families)
    }

    /// Appends the set of family identifiers to the given fallback key.
    pub fn append_fallbacks(
        &mut self,
        key: impl Into<FallbackKey>,
        families: impl Iterator<Item = FamilyId>,
    ) -> bool {
        self.inner.append_fallbacks(key, families)
    }

    /// Returns an object for selecting fonts from this collection.
    #[cfg(feature = "std")]
    pub fn query<'a>(&'a mut self, source_cache: &'a mut SourceCache) -> Query<'a> {
        Query::new(self, source_cache)
    }

    /// Registers all fonts that exist in the given data.
    ///
    /// Returns a list of pairs each containing the family identifier and fonts
    /// added to that family.
    pub fn register_fonts(&mut self, data: Vec<u8>) -> Vec<(FamilyId, Vec<FontInfo>)> {
        self.inner.register_fonts(data)
    }
}

impl Default for Collection {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

/// Collection of fonts.
#[derive(Clone)]
struct Inner {
    system: Option<System>,
    data: CommonData,
    #[allow(unused)]
    shared: Option<Arc<Shared>>,
    #[allow(unused)]
    shared_version: u64,
    fallback_cache: FallbackCache,
}

impl Inner {
    /// Creates a new collection with the given options.
    pub fn new(options: CollectionOptions) -> Self {
        let system = options.system_fonts.then(System::new);
        let shared = options.shared.then(|| Arc::new(Shared::default()));
        Self {
            system,
            data: CommonData::default(),
            shared,
            shared_version: 0,
            fallback_cache: Default::default(),
        }
    }

    /// Returns an iterator over all available family names in the collection.
    ///
    /// This includes both system and registered fonts.
    pub fn family_names(&mut self) -> impl Iterator<Item = &str> + '_ + Clone {
        self.sync_shared();
        FamilyNames {
            ours: self.data.family_names.iter(),
            system: self.system.as_ref().map(|sys| sys.family_names.iter()),
        }
        .map(|name| name.name())
    }

    /// Returns the family identifier for the given family name.
    pub fn family_id(&mut self, name: &str) -> Option<FamilyId> {
        self.sync_shared();
        self.data
            .family_names
            .get(name)
            .or_else(|| {
                self.system
                    .as_ref()
                    .and_then(|sys| sys.family_names.get(name))
            })
            .map(|n| n.id())
    }

    /// Returns the family name for the given family identifier.
    pub fn family_name(&mut self, id: FamilyId) -> Option<&str> {
        self.sync_shared();
        self.data
            .family_names
            .get_by_id(id)
            .or_else(|| {
                self.system
                    .as_ref()
                    .and_then(|sys| sys.family_names.get_by_id(id))
            })
            .map(|name| name.name())
    }

    /// Returns the family object for the given family identifier.
    pub fn family(&mut self, id: FamilyId) -> Option<FamilyInfo> {
        self.sync_shared();
        if let Some(family) = self.data.families.get(&id) {
            family.as_ref().cloned()
        } else {
            #[cfg(feature = "system")]
            if let Some(system) = &self.system {
                let family = system.fonts.lock().unwrap().family(id);
                self.data.families.insert(id, family.clone());
                family
            } else {
                None
            }
            #[cfg(not(feature = "system"))]
            {
                None
            }
        }
    }

    /// Returns the family object for the given name.
    pub fn family_by_name(&mut self, name: &str) -> Option<FamilyInfo> {
        if let Some(id) = self.family_id(name) {
            self.family(id)
        } else {
            None
        }
    }

    /// Returns an iterator over the family identifiers for the given
    /// generic family.
    pub fn generic_families(
        &mut self,
        family: GenericFamily,
    ) -> impl Iterator<Item = FamilyId> + '_ + Clone {
        self.sync_shared();
        GenericFamilies {
            ours: self.data.generic_families.get(family).iter().copied(),
            system: self
                .system
                .as_ref()
                .map(|sys| sys.generic_families.get(family).iter().copied()),
        }
    }

    /// Replaces the set of family identifiers associated with the given generic
    /// family.
    pub fn set_generic_families(
        &mut self,
        generic: GenericFamily,
        families: impl Iterator<Item = FamilyId>,
    ) {
        self.sync_shared();
        #[cfg(feature = "std")]
        if let Some(shared) = &self.shared {
            shared
                .data
                .lock()
                .unwrap()
                .generic_families
                .set(generic, families);
            shared.bump_version();
        } else {
            self.data.generic_families.set(generic, families);
        }
        #[cfg(not(feature = "std"))]
        self.data.generic_families.set(generic, families);
    }

    /// Appends the set of family identifiers to the given generic family.
    pub fn append_generic_families(
        &mut self,
        generic: GenericFamily,
        families: impl Iterator<Item = FamilyId>,
    ) {
        self.sync_shared();
        #[cfg(feature = "std")]
        if let Some(shared) = &self.shared {
            shared
                .data
                .lock()
                .unwrap()
                .generic_families
                .append(generic, families);
            shared.bump_version();
        } else {
            self.data.generic_families.append(generic, families);
        }
        #[cfg(not(feature = "std"))]
        self.data.generic_families.append(generic, families);
    }

    /// Returns an iterator over the fallback families for the given
    /// key.
    pub fn fallback_families(
        &mut self,
        key: impl Into<FallbackKey>,
    ) -> impl Iterator<Item = FamilyId> + '_ + Clone {
        let selector = key.into();
        let script = selector.script();
        let lang_key = selector.locale();
        if self.fallback_cache.script != Some(script) || self.fallback_cache.language != lang_key {
            self.sync_shared();
            self.fallback_cache.reset();
            #[cfg(feature = "system")]
            if let Some(families) = self.data.fallbacks.get(selector) {
                self.fallback_cache.set(script, lang_key, families);
            } else if let Some(system) = self.system.as_ref() {
                // Some platforms don't need mut System
                #[allow(unused_mut)]
                let mut system = system.fonts.lock().unwrap();
                if let Some(family) = system.fallback(selector) {
                    self.data.fallbacks.set(selector, core::iter::once(family));
                    self.fallback_cache.set(script, lang_key, &[family]);
                }
            }
            #[cfg(not(feature = "system"))]
            if let Some(families) = self.data.fallbacks.get(selector) {
                self.fallback_cache.set(script, lang_key, families);
            }
        }
        self.fallback_cache.families.iter().copied()
    }

    /// Replaces the set of family identifiers associated with the fallback
    /// key.
    pub fn set_fallbacks(
        &mut self,
        key: impl Into<FallbackKey>,
        families: impl Iterator<Item = FamilyId>,
    ) -> bool {
        self.sync_shared();
        #[cfg(feature = "std")]
        if let Some(shared) = &self.shared {
            let result = shared.data.lock().unwrap().fallbacks.set(key, families);
            shared.bump_version();
            result
        } else {
            self.data.fallbacks.set(key, families)
        }
        #[cfg(not(feature = "std"))]
        self.data.fallbacks.set(key, families)
    }

    /// Appends the set of family identifiers to the given fallback key.
    pub fn append_fallbacks(
        &mut self,
        key: impl Into<FallbackKey>,
        families: impl Iterator<Item = FamilyId>,
    ) -> bool {
        self.sync_shared();
        #[cfg(feature = "std")]
        if let Some(shared) = &self.shared {
            let result = shared.data.lock().unwrap().fallbacks.append(key, families);
            shared.bump_version();
            result
        } else {
            self.data.fallbacks.append(key, families)
        }
        #[cfg(not(feature = "std"))]
        self.data.fallbacks.append(key, families)
    }

    /// Registers all fonts that exist in the given data.
    ///
    /// Returns a list of pairs each containing the family identifier and fonts
    /// added to that family.
    pub fn register_fonts(&mut self, data: Vec<u8>) -> Vec<(FamilyId, Vec<FontInfo>)> {
        #[cfg(feature = "std")]
        if let Some(shared) = &self.shared {
            let result = shared.data.lock().unwrap().register_fonts(data);
            shared.bump_version();
            result
        } else {
            self.data.register_fonts(data)
        }
        #[cfg(not(feature = "std"))]
        self.data.register_fonts(data)
    }

    fn sync_shared(&mut self) {
        #[cfg(feature = "std")]
        if let Some(shared) = &self.shared {
            let version = shared.version.load(Ordering::Acquire);
            if self.shared_version != version {
                // This is an ugly deep copy, but the assumption is that
                // modifications to font collections are fairly rare.
                // If this becomes a problem, do more fine grained tracking
                // of changes.
                self.data = shared.data.lock().unwrap().clone();
                self.shared_version = version;
                self.fallback_cache.reset();
            }
        }
    }
}

#[derive(Clone)]
struct FamilyNames<I> {
    ours: I,
    system: Option<I>,
}

impl<'a, I> Iterator for FamilyNames<I>
where
    I: Iterator<Item = &'a FamilyName> + 'a,
{
    type Item = &'a FamilyName;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ours) = self.ours.next() {
            return Some(ours);
        }
        self.system.as_mut()?.next()
    }
}

#[derive(Clone)]
struct GenericFamilies<I> {
    ours: I,
    system: Option<I>,
}

impl<'a, I> Iterator for GenericFamilies<I>
where
    I: Iterator<Item = FamilyId> + 'a,
{
    type Item = FamilyId;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ours) = self.ours.next() {
            return Some(ours);
        }
        self.system.as_mut()?.next()
    }
}

#[derive(Clone, Default)]
struct FallbackCache {
    script: Option<Script>,
    language: Option<&'static str>,
    families: Vec<FamilyId>,
}

impl FallbackCache {
    fn reset(&mut self) {
        self.script = None;
        self.language = None;
        self.families.clear();
    }

    fn set(&mut self, script: Script, language: Option<&'static str>, families: &[FamilyId]) {
        self.script = Some(script);
        self.language = language;
        self.families.clear();
        self.families.extend_from_slice(families);
    }
}

/// Data taken from the system font collection.
#[derive(Clone)]
struct System {
    #[cfg(feature = "system")]
    fonts: Arc<Mutex<SystemFonts>>,
    family_names: Arc<FamilyNameMap>,
    generic_families: Arc<GenericFamilyMap>,
}

impl System {
    fn new() -> Self {
        let fonts = SystemFonts::new();
        let family_names = fonts.name_map.clone();
        let generic_families = fonts.generic_families.clone();
        #[cfg(feature = "system")]
        let fonts = Arc::new(Mutex::new(fonts));
        Self {
            #[cfg(feature = "system")]
            fonts,
            family_names,
            generic_families,
        }
    }
}

/// Common data for base and shared collections.
#[derive(Clone, Default)]
struct CommonData {
    family_names: FamilyNameMap,
    families: FamilyMap,
    generic_families: GenericFamilyMap,
    fallbacks: FallbackMap,
}

impl CommonData {
    fn register_fonts(&mut self, data: Vec<u8>) -> Vec<(FamilyId, Vec<FontInfo>)> {
        let blob = Blob::new(Arc::new(data));
        let mut families: HashMap<FamilyId, (FamilyName, Vec<FontInfo>)> = Default::default();
        let mut family_name = String::default();
        let data_id = SourceId::new();
        super::scan::scan_memory(blob.as_ref(), |scanned_font| {
            use skrifa::raw::types::NameId;
            family_name.clear();
            let family_chars = scanned_font
                .english_or_first_name(NameId::TYPOGRAPHIC_FAMILY_NAME)
                .or_else(|| scanned_font.english_or_first_name(NameId::FAMILY_NAME))
                .map(|name| name.chars());
            let Some(family_chars) = family_chars else {
                return;
            };
            family_name.extend(family_chars);
            if family_name.is_empty() {
                return;
            }
            let data = SourceInfo {
                id: data_id,
                kind: SourceKind::Memory(blob.clone()),
            };
            let Some(font) = FontInfo::from_font_ref(&scanned_font.font, data, scanned_font.index)
            else {
                return;
            };
            let name = self.family_names.get_or_insert(&family_name);
            families
                .entry(name.id())
                .or_insert_with(|| (name, Default::default()))
                .1
                .push(font);
        });
        for (id, (name, fonts)) in &families {
            if let Some(Some(family)) = self.families.get_mut(id) {
                let new_fonts = family.fonts().iter().chain(fonts).cloned();
                *family = FamilyInfo::new(name.clone(), new_fonts);
            } else {
                let family = FamilyInfo::new(name.clone(), fonts.iter().cloned());
                self.families.insert(*id, Some(family));
            }
        }
        families
            .into_iter()
            .map(|(id, (_, fonts))| (id, fonts))
            .collect()
    }
}

/// Synchronized shared collection data.
#[derive(Default)]
struct Shared {
    #[allow(unused)]
    version: AtomicU64,
    #[cfg(feature = "std")]
    #[allow(unused)]
    data: Mutex<CommonData>,
}

impl Shared {
    #[cfg(feature = "std")]
    fn bump_version(&self) {
        self.version.fetch_add(1, Ordering::Release);
    }
}
