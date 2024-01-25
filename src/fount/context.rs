use super::data::*;
use super::font::FontData;
use super::id::*;
use super::library::*;
use super::*;
use std::cell::RefCell;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use swash::text::Script;

/// Interface to a font library providing enumeration, queries and fallbacks.
#[derive(Clone)]
pub struct FontContext {
    library: Library,
    user: RefCell<Arc<(u64, CollectionData)>>,
}

impl FontContext {
    /// Creates a new font context for the associated font library.
    pub fn new(library: &Library) -> Self {
        let library_user = library.inner.user.read().unwrap();
        let user_version = library.inner.user_version.load(Ordering::Relaxed);
        let user = RefCell::new(Arc::new((user_version, library_user.clone())));
        Self {
            library: library.clone(),
            user,
        }
    }

    /// Returns the underlying font library for the context.
    pub fn library(&self) -> &Library {
        &self.library
    }

    /// Returns an iterator over the file system paths where fonts in this
    /// context may be found.
    pub fn source_paths(&self) -> SourcePaths {
        self.library.inner.system.source_paths()
    }

    /// Returns an iterator over the font families in the context.
    pub fn families(&self) -> Families {
        Families {
            user: self.user.borrow().clone(),
            library: self.library.clone(),
            pos: 0,
            stage: 0,
        }
    }

    /// Returns the font family entry for the specified identifier.
    pub fn family(&self, id: FamilyId) -> Option<FamilyEntry> {
        if id.is_user_font() {
            self.sync_user();
            self.user.borrow().1.family(id)
        } else {
            self.library.inner.system.family(id)
        }
    }

    /// Returns the font family entry for the specified name.
    pub fn family_by_name<'a>(&'a self, name: &str) -> Option<FamilyEntry> {
        self.sync_user();
        if let Some(family) = self.user.borrow().1.family_by_name(name) {
            Some(family)
        } else {
            self.library.inner.system.family_by_name(name)
        }
    }

    /// Returns the font entry for the specified identifier.
    pub fn font(&self, id: FontId) -> Option<FontEntry> {
        if id.is_user_font() {
            self.sync_user();
            self.user.borrow().1.font(id)
        } else {
            self.library.inner.system.font(id)
        }
    }

    /// Returns the font source entry for the specified identifier.
    pub fn source(&self, id: SourceId) -> Option<SourceEntry> {
        if id.is_user_font() {
            self.sync_user();
            self.user.borrow().1.source(id)
        } else {
            self.library.inner.system.source(id)
        }
    }

    /// Loads the font data for the specified source.
    pub fn load(&self, id: SourceId) -> Option<FontData> {
        if id.is_user_font() {
            self.sync_user();
            self.user.borrow().1.load(id)
        } else {
            self.library.inner.system.load(id)
        }
    }

    /// Returns an ordered sequence of font family identifers that represent
    /// the default font families.
    pub fn default_families(&self) -> &[FamilyId] {
        self.library.inner.system.default_families()
    }

    /// Returns an ordered sequence of font family identifers that represent the
    /// specified generic font family.
    pub fn generic_families(&self, family: GenericFamily) -> &[FamilyId] {
        self.library.inner.system.generic_families(family)
    }

    /// Returns an ordered sequence of font family identifers that represent the
    /// fallback chain for the specified script and locale.
    pub fn fallback_families(&self, script: Script, locale: Option<Locale>) -> &[FamilyId] {
        self.library.inner.system.fallback_families(script, locale)
    }

    /// Registers the fonts contained in the specified data. Returns identifiers for
    /// the families and fonts added to the context.
    pub fn register_fonts(&self, data: Vec<u8>) -> Option<Registration> {
        use super::scan::FontScanner;
        let mut scanner = FontScanner::default();
        let mut collection = self.library.inner.user.write().unwrap();
        let mut reg = Registration::default();
        let count = collection
            .add_fonts(&mut scanner, FontData::new(data), Some(&mut reg), None)
            .unwrap_or(0);
        if count != 0 {
            self.library
                .inner
                .user_version
                .fetch_add(1, Ordering::Relaxed);
            Some(reg)
        } else {
            None
        }
    }

    fn sync_user(&self) {
        let user_version = self.library.inner.user_version.load(Ordering::Relaxed);
        if self.user.borrow().0 != user_version {
            let mut arc_user = self.user.borrow().clone();
            let mut user = Arc::make_mut(&mut arc_user);
            let library_user = self.library.inner.user.read().unwrap();
            library_user.clone_into(&mut user.1);
            user.0 = self.library.inner.user_version.load(Ordering::Relaxed);
            *self.user.borrow_mut() = arc_user;
        }
    }
}
