use core::{
    ffi::{CStr, c_char},
    iter::once,
    marker::PhantomData,
    ptr::NonNull,
};
use std::{
    borrow::Cow,
    ffi::{CString, OsStr},
    os::unix::ffi::OsStrExt,
    path::Path,
    sync::Arc,
};

use fontconfig_sys::{
    FcChar8, FcCharSet, FcConfig, FcFontSet, FcLangSet, FcMatchKind, FcMatchPattern, FcPattern,
    FcResult, FcResultMatch, FcResultNoId, FcResultNoMatch, FcResultOutOfMemory,
    FcResultTypeMismatch, FcSetSystem,
    constants::{FC_CHARSET, FC_FAMILY, FC_FILE, FC_INDEX, FC_LANG, FC_SLANT, FC_WEIGHT, FC_WIDTH},
    statics::{LIB, LIB_RESULT},
};
use hashbrown::{HashMap, HashSet, hash_map::Entry};
use smallvec::SmallVec;

use crate::{
    FallbackKey, FamilyId, FamilyInfo, FontInfo, FontStyle, FontWeight, FontWidth, GenericFamily,
    Script,
    family_name::{FamilyName, FamilyNameMap},
    generic::GenericFamilyMap,
    source::SourcePathMap,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MatchErr {
    NoMatch,
    TypeMismatch,
    NoId,
    OutOfMemory,
    Other,
}

impl MatchErr {
    fn from_raw(raw: FcResult) -> Self {
        #[allow(non_upper_case_globals)]
        match raw {
            FcResultNoMatch => Self::NoMatch,
            FcResultTypeMismatch => Self::TypeMismatch,
            FcResultNoId => Self::NoId,
            FcResultOutOfMemory => Self::OutOfMemory,
            _ => Self::Other,
        }
    }
}

type MatchResult<T> = Result<T, MatchErr>;

/// Ownership for refcounted Fontconfig objects. Used to track if a given
/// fontconfig function returns an object that it owns or is passing its
/// ownership to us.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ownership {
    /// This object is owned by Fontconfig and needs to be freed by it.
    Fontconfig,
    /// This object is owned by the application.
    Application,
}

/// Wrapper for an `FcPattern`.
struct Pattern {
    inner: NonNull<FcPattern>,
}

impl Pattern {
    fn new() -> Option<Self> {
        Some(unsafe { Self::from_raw((LIB.FcPatternCreate)(), Ownership::Application)? })
    }

    unsafe fn from_raw(raw: *mut FcPattern, ownership: Ownership) -> Option<Self> {
        let inner = NonNull::new(raw)?;
        // Don't free this object when we are dropped.
        if ownership == Ownership::Fontconfig {
            unsafe {
                (LIB.FcPatternReference)(inner.as_ptr());
            }
        }
        Some(Self { inner })
    }

    fn add_string(&mut self, object: &CStr, s: &CStr) -> bool {
        // All objects passed to FcPatternAddWhatever are cloned.
        unsafe {
            (LIB.FcPatternAddString)(self.inner.as_ptr(), object.as_ptr(), s.as_ptr() as *const _)
                != 0
        }
    }

    fn add_charset(&mut self, object: &CStr, s: &CharSet) -> bool {
        unsafe {
            (LIB.FcPatternAddCharSet)(self.inner.as_ptr(), object.as_ptr(), s.inner.as_ptr()) != 0
        }
    }

    fn add_langset(&mut self, object: &CStr, s: &LangSet) -> bool {
        unsafe {
            (LIB.FcPatternAddLangSet)(self.inner.as_ptr(), object.as_ptr(), s.inner.as_ptr()) != 0
        }
    }

    fn get_string<'a>(&'a self, object: &CStr, n: u32) -> MatchResult<Cow<'a, str>> {
        Ok(self.get_c_string(object, n)?.to_string_lossy())
    }

    fn get_c_string<'a>(&'a self, object: &CStr, n: u32) -> MatchResult<&'a CStr> {
        let mut dest: *mut FcChar8 = std::ptr::null_mut();
        let result = unsafe {
            (LIB.FcPatternGetString)(
                self.inner.as_ptr(),
                object.as_ptr(),
                n.try_into().map_err(|_| MatchErr::Other)?,
                &raw mut dest,
            )
        };
        if result != FcResultMatch {
            return Err(MatchErr::from_raw(result));
        }
        let dest = NonNull::new(dest).ok_or(MatchErr::Other)?;
        Ok(unsafe { CStr::from_ptr(dest.as_ptr() as *const _) })
    }

    fn get_int(&self, object: &CStr, n: u32) -> MatchResult<i32> {
        let mut dest = 0;
        let result = unsafe {
            (LIB.FcPatternGetInteger)(
                self.inner.as_ptr(),
                object.as_ptr(),
                n.try_into().map_err(|_| MatchErr::Other)?,
                &raw mut dest,
            )
        };
        if result != FcResultMatch {
            return Err(MatchErr::from_raw(result));
        }
        Ok(dest)
    }
}

impl Clone for Pattern {
    fn clone(&self) -> Self {
        unsafe { (LIB.FcPatternReference)(self.inner.as_ptr()) };
        Self { inner: self.inner }
    }
}

impl Drop for Pattern {
    fn drop(&mut self) {
        unsafe { (LIB.FcPatternDestroy)(self.inner.as_ptr()) };
    }
}

impl std::fmt::Debug for Pattern {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match NonNull::new(unsafe { (LIB.FcNameUnparse)(self.inner.as_ptr()) }) {
            Some(unparsed) => {
                let res = f.write_str(unsafe {
                    &CStr::from_ptr(unparsed.as_ptr() as *const c_char).to_string_lossy()
                });
                unsafe { (LIB.FcStrFree)(unparsed.as_ptr()) };
                res
            }
            None => f.debug_struct("Pattern").finish(),
        }
    }
}

struct FontSet<'a> {
    inner: NonNull<FcFontSet>,
    ownership: Ownership,
    // If an `FcFontSet` is created from an `FcPattern`, it will reference that
    // pattern's data. Well, maybe. The docs say "The returned FcFontSet
    // references FcPattern structures which may be shared by the return value
    // from multiple FcFontSort calls, applications cannot modify these
    // patterns." It's unclear whether this refers to actual lifetime/ownership
    // semantics or if everything's properly refcounted and you're just not
    // allowed to mutate them.
    _parent: PhantomData<&'a ()>,
}

impl FontSet<'_> {
    unsafe fn from_raw(raw: *mut FcFontSet, ownership: Ownership) -> Option<Self> {
        let inner = NonNull::new(raw)?;
        Some(Self {
            inner,
            ownership,
            _parent: PhantomData,
        })
    }

    fn iter(&self) -> FontSetIter<'_> {
        FontSetIter {
            i: 0,
            font_set: self,
        }
    }
}

impl Drop for FontSet<'_> {
    fn drop(&mut self) {
        if self.ownership == Ownership::Application {
            unsafe { (LIB.FcFontSetDestroy)(self.inner.as_ptr()) };
        }
    }
}

struct FontSetIter<'a> {
    i: usize,
    font_set: &'a FontSet<'a>,
}

impl Iterator for FontSetIter<'_> {
    type Item = Pattern;

    fn next(&mut self) -> Option<Self::Item> {
        let font_set = self.font_set.inner.as_ptr();
        if self.i >= unsafe { (*font_set).nfont }.try_into().ok()? {
            None
        } else {
            let pattern: *mut FcPattern = unsafe { *(*font_set).fonts.add(self.i) };
            self.i += 1;
            Some(unsafe { Pattern::from_raw(pattern, Ownership::Fontconfig) }?)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let nfont: Result<usize, _> = unsafe { (*self.font_set.inner.as_ptr()).nfont }.try_into();
        let Ok(nfont) = nfont else {
            return (0, None);
        };
        (nfont, Some(nfont))
    }
}

struct LangSet {
    inner: NonNull<FcLangSet>,
}

impl LangSet {
    fn new() -> Option<Self> {
        let inner = NonNull::new(unsafe { (LIB.FcLangSetCreate)() })?;
        Some(Self { inner })
    }

    fn add(&mut self, lang: &CStr) -> bool {
        unsafe { (LIB.FcLangSetAdd)(self.inner.as_ptr(), lang.as_ptr() as *const _) != 0 }
    }
}

impl Drop for LangSet {
    fn drop(&mut self) {
        unsafe { (LIB.FcLangSetDestroy)(self.inner.as_ptr()) };
    }
}

impl Clone for LangSet {
    fn clone(&self) -> Self {
        Self {
            inner: unsafe { NonNull::new((LIB.FcLangSetCopy)(self.inner.as_ptr())).unwrap() },
        }
    }
}

struct CharSet {
    inner: NonNull<FcCharSet>,
}

impl CharSet {
    fn new() -> Option<Self> {
        let inner = NonNull::new(unsafe { (LIB.FcCharSetCreate)() })?;
        Some(Self { inner })
    }

    fn add(&mut self, c: char) -> bool {
        unsafe { (LIB.FcCharSetAddChar)(self.inner.as_ptr(), c as u32) != 0 }
    }
}

impl Drop for CharSet {
    fn drop(&mut self) {
        unsafe { (LIB.FcCharSetDestroy)(self.inner.as_ptr()) };
    }
}

impl Clone for CharSet {
    fn clone(&self) -> Self {
        Self {
            inner: unsafe { NonNull::new((LIB.FcCharSetCopy)(self.inner.as_ptr())).unwrap() },
        }
    }
}

struct Config {
    inner: NonNull<FcConfig>,
}

impl Config {
    unsafe fn from_raw(raw: *mut FcConfig, ownership: Ownership) -> Option<Self> {
        let inner = NonNull::new(raw)?;
        // Don't free this object when we are dropped.
        if ownership == Ownership::Fontconfig {
            unsafe {
                (LIB.FcConfigReference)(inner.as_ptr());
            }
        }
        Some(Self { inner })
    }

    fn substitute(&self, pattern: &mut Pattern, kind: FcMatchKind) {
        unsafe { (LIB.FcConfigSubstitute)(self.inner.as_ptr(), pattern.inner.as_ptr(), kind) };
    }

    fn font_sort<'me, 'ret, 'pat: 'ret>(
        &'me self,
        pattern: &'pat Pattern,
        trim: bool,
    ) -> MatchResult<FontSet<'ret>> {
        let mut result = 0;
        // The returned FcFontSet is for us to free
        let font_set = unsafe {
            FontSet::from_raw(
                (LIB.FcFontSort)(
                    self.inner.as_ptr(),
                    pattern.inner.as_ptr(),
                    trim as i32,
                    std::ptr::null_mut(),
                    &raw mut result,
                ),
                Ownership::Application,
            )
        }
        .ok_or(MatchErr::Other)?;
        if result != FcResultMatch {
            return Err(MatchErr::from_raw(result));
        }

        Ok(font_set)
    }

    fn font_match(&self, pattern: &Pattern) -> MatchResult<Pattern> {
        let mut result = 0;
        let pattern = unsafe {
            Pattern::from_raw(
                (LIB.FcFontMatch)(self.inner.as_ptr(), pattern.inner.as_ptr(), &raw mut result),
                Ownership::Application,
            )
        }
        .ok_or(MatchErr::Other)?;
        if result != FcResultMatch {
            return Err(MatchErr::from_raw(result));
        }

        Ok(pattern)
    }

    fn font_render_prepare(&self, pat: &Pattern, font: &Pattern) -> Option<Pattern> {
        unsafe {
            Pattern::from_raw(
                (LIB.FcFontRenderPrepare)(
                    self.inner.as_ptr(),
                    pat.inner.as_ptr(),
                    font.inner.as_ptr(),
                ),
                Ownership::Application,
            )
        }
    }
}

impl Clone for Config {
    fn clone(&self) -> Self {
        unsafe { (LIB.FcConfigReference)(self.inner.as_ptr()) };
        Self { inner: self.inner }
    }
}

impl Drop for Config {
    fn drop(&mut self) {
        unsafe { (LIB.FcConfigDestroy)(self.inner.as_ptr()) };
    }
}

/// Cache wrapper that maps Unicode scripts to fontconfig [`CharSet`]s.
#[derive(Default)]
struct ScriptCharSetMap(HashMap<Script, Option<CharSet>>);

impl ScriptCharSetMap {
    fn charset_for_script(&mut self, script: Script) -> Option<&CharSet> {
        match self.0.entry(script) {
            Entry::Occupied(e) => e.into_mut().as_ref(),
            Entry::Vacant(e) => {
                let Some(sample) = script.sample() else {
                    return e.insert(None).as_ref();
                };

                let mut charset = CharSet::new()?;
                for c in sample.chars() {
                    charset.add(c);
                }
                e.insert(Some(charset)).as_ref()
            }
        }
    }
}

/// Raw access to the collection of local system fonts.
#[derive(Default)]
pub(crate) struct SystemFonts {
    pub(crate) name_map: Arc<FamilyNameMap>,
    pub(crate) generic_families: Arc<GenericFamilyMap>,
    source_cache: SourcePathMap,
    family_map: HashMap<FamilyId, Option<FamilyInfo>>,
    config: Option<Config>,
    script_charsets: ScriptCharSetMap,
}

unsafe impl Send for SystemFonts {}

impl SystemFonts {
    pub(crate) fn new() -> Self {
        let library_exists = LIB_RESULT.as_ref().ok().is_some();
        // We couldn't find the fontconfig library; maybe it doesn't exist. Just
        // return a `SystemFonts` with no `config`. All our methods will return
        // `None` and shouldn't attempt any FFI calls because the first thing we
        // do is check for `config`.
        if !library_exists {
            return Default::default();
        }

        // Initialize the config
        let config = unsafe { (LIB.FcInitLoadConfig)() };
        // fontconfig returns a new config object each time we call FcInitLoadConfig
        let Some(config) = (unsafe { Config::from_raw(config, Ownership::Application) }) else {
            return Default::default();
        };
        unsafe {
            (LIB.FcConfigBuildFonts)(config.inner.as_ptr());
        }

        // Get all the fonts

        // The fontconfig docs say this "isn't threadsafe", but this seems to be
        // related to refcounting:
        // https://gitlab.freedesktop.org/fontconfig/fontconfig/-/commit/b5bcf61fe789e66df2de609ec246cb7e4d326180
        // The source code for this function just calls `FcConfigGetCurrent` if
        // none is provided (which we do, and it's atomic anyway) and then
        // dereferences a pointer. I *think* the safety issue they're referring
        // to is that if we destroyed this config on another thread and then
        // tried to access what it returns, it would dereference a null pointer.
        // But we're not doing that.
        let font_set =
            NonNull::new(unsafe { (LIB.FcConfigGetFonts)(config.inner.as_ptr(), FcSetSystem) })
                .unwrap();
        let fonts = unsafe { (*font_set.as_ptr()).fonts };
        let n_fonts = unsafe { (*font_set.as_ptr()).nfont };

        // Populate the family name map
        let mut name_map = FamilyNameMap::default();
        for i in 0..n_fonts as usize {
            let pattern: *mut FcPattern = unsafe { *fonts.add(i) };
            let pattern = unsafe { Pattern::from_raw(pattern, Ownership::Fontconfig) }.unwrap();
            let mut i = 0;

            let mut first_name_id = None;
            // For fonts with more than one family name, the second one is
            // *often* (but not always) an RBIZ name
            while let Ok(name) = pattern.get_string(FC_FAMILY, i) {
                if i == 0 {
                    // First name
                    first_name_id = Some(name_map.get_or_insert(strip_rbiz(&name)).id());
                } else if let Some(first_name_id) = first_name_id {
                    name_map.add_alias(first_name_id, strip_rbiz(&name));
                }
                i += 1;
            }
        }

        // Populate the generic family map
        let mut generic_families = GenericFamilyMap::default();
        for (generic_family, name) in GENERIC_FAMILY_NAMES {
            let mut pattern = Pattern::new().unwrap();
            pattern.add_string(FC_FAMILY, name);
            // TODO: do we need FcConfigSetDefaultSubstitute?

            config.substitute(&mut pattern, FcMatchPattern);

            // We enable the "trim" option here which ignores later fonts if
            // they provide no new Unicode coverage.
            let font_set = config.font_sort(&pattern, true).unwrap();

            // There are a lot of duplicate font names in the substituted
            // pattern. Keep track of which ones have already been added to the
            // list.
            let mut added_names = HashSet::new();

            for font in font_set.iter() {
                // Not sure if FcFontRenderPrepare performs any substitutions
                // relevant to fallback family name matching, but it's a good
                // idea to call it just in case
                let Some(font) = config.font_render_prepare(&pattern, &font) else {
                    continue;
                };
                // Generic families can have more than one name, but the only
                // one we care about is the first one
                let Ok(name) = font.get_string(FC_FAMILY, 0) else {
                    continue;
                };

                let name = strip_rbiz(&name);
                if added_names.contains(name) {
                    continue;
                }
                let Some(family_name) = name_map.get(name) else {
                    continue;
                };

                added_names.insert(name.to_owned());
                generic_families.append(*generic_family, once(family_name.id()));
            }
        }

        Self {
            name_map: Arc::new(name_map),
            generic_families: Arc::new(generic_families),
            source_cache: Default::default(),
            family_map: Default::default(),
            config: Some(config),
            script_charsets: Default::default(),
        }
    }

    pub(crate) fn family(&mut self, id: FamilyId) -> Option<FamilyInfo> {
        match self.family_map.get(&id) {
            Some(Some(family)) => return Some(family.clone()),
            Some(None) => return None,
            None => {}
        }

        let family = self.family_uncached(id);
        self.family_map.insert(id, family.clone());
        family
    }

    pub(crate) fn fallback(&mut self, key: impl Into<FallbackKey>) -> Option<FamilyId> {
        let config = self.config.as_ref()?;
        let key: FallbackKey = key.into();

        let mut pattern = Pattern::new()?;

        let locale_lang_set = key.locale().and_then(|locale| {
            let mut lang_set = LangSet::new()?;
            lang_set.add(CString::new(locale).ok()?.as_c_str());
            Some(lang_set)
        });
        let script_char_set = self.script_charsets.charset_for_script(key.script());

        if let Some(set) = locale_lang_set {
            pattern.add_langset(FC_LANG, &set);
        }
        if let Some(set) = script_char_set {
            pattern.add_charset(FC_CHARSET, set);
        }

        config.substitute(&mut pattern, FcMatchPattern);

        // This calls FcFontRenderPrepare for us
        let font = config.font_match(&pattern).ok()?;

        let family_name = font.get_string(FC_FAMILY, 0).ok()?;
        self.name_map.get(&family_name).map(FamilyName::id)
    }
}

impl SystemFonts {
    fn family_uncached(&mut self, id: FamilyId) -> Option<FamilyInfo> {
        let config = self.config.as_ref()?;
        let name = self.name_map.get_by_id(id).cloned()?;

        // Match by family name
        let mut pattern = Pattern::new()?;
        pattern.add_string(FC_FAMILY, CString::new(name.name()).ok()?.as_c_str());
        config.substitute(&mut pattern, FcMatchPattern);

        let fc_fonts = config.font_sort(&pattern, false).ok()?;
        let mut font_infos = SmallVec::<[FontInfo; 4]>::new();
        for font in fc_fonts.iter() {
            let Some(font) = config.font_render_prepare(&pattern, &font) else {
                continue;
            };
            let Ok(family_name) = font.get_string(FC_FAMILY, 0) else {
                continue;
            };
            // We've performed font substitution and then sorted everything by
            // "closeness", so the good fonts should be at the top. Once we see
            // a fallback font (one that's not part of the family we explicitly
            // asked for), we can stop.
            if family_name != name.name() {
                break;
            }

            if let Some(font_info) = (|| {
                let path = font.get_c_string(FC_FILE, 0).ok()?;
                // This part is Unix-specific. Sorry, Windows fontconfig user.
                let path = Path::new(OsStr::from_bytes(path.to_bytes()));
                let source_info = self.source_cache.get_or_insert(path);

                let weight = font
                    .get_int(FC_WEIGHT, 0)
                    .map(FontWeight::from_fontconfig)
                    .unwrap_or_default();
                let width = font
                    .get_int(FC_WIDTH, 0)
                    .map(FontWidth::from_fontconfig)
                    .unwrap_or_default();
                let style = font
                    .get_int(FC_SLANT, 0)
                    .map(FontStyle::from_fontconfig)
                    .unwrap_or_default();
                let index = font.get_int(FC_INDEX, 0).map_or(0, |idx| idx.max(0) as u32);

                let mut font_info = FontInfo::from_source(source_info, index)?;
                // TODO(valadaptive): does this do anything anymore?
                font_info.maybe_override_attributes(width, style, weight);
                Some(font_info)
            })() {
                font_infos.push(font_info);
            }
        }

        if font_infos.is_empty() {
            return None;
        }

        Some(FamilyInfo::new(name.clone(), font_infos))
    }
}

const GENERIC_FAMILY_NAMES: &[(GenericFamily, &CStr)] = &[
    (GenericFamily::Serif, c"serif"),
    (GenericFamily::SansSerif, c"sans-serif"),
    (GenericFamily::Monospace, c"monospace"),
    (GenericFamily::Cursive, c"cursive"),
    (GenericFamily::Fantasy, c"fantasy"),
    (GenericFamily::SystemUi, c"system-ui"),
    (GenericFamily::Emoji, c"emoji"),
    (GenericFamily::Math, c"math"),
];

/// Fontconfig seems to force RBIZ (regular, bold, italic, bold italic) when
/// categorizing fonts. This removes those suffixes from family names so that
/// we can match on all attributes.
fn strip_rbiz(name: &str) -> &str {
    // TODO(valadaptive): this seems incomplete. check fcname.c for their
    // constants
    const SUFFIXES: &[&str] = &[
        " Thin",
        " ExtraLight",
        " DemiLight",
        " Light",
        " Medium",
        " Black",
        " SemiBold",
        " Semibold",
        " ExtraBold",
        " Extra Bold",
        " Black",
        " Narrow",
    ];
    for suffix in SUFFIXES {
        if let Some(name) = name.strip_suffix(suffix) {
            return name;
        }
    }
    name
}
