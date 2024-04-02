use super::{
    scan, FallbackKey, FamilyId, FamilyInfo, FamilyNameMap, GenericFamily, GenericFamilyMap,
};
use alloc::sync::Arc;
use hashbrown::HashMap;
use {
    core_foundation::{
        base::TCFType,
        dictionary::CFDictionary,
        string::{CFString, CFStringRef},
    },
    core_foundation_sys::base::CFRange,
    core_text::{
        font::{self, kCTFontSystemFontType, CTFont, CTFontRef, CTFontUIFontType},
        font_descriptor,
    },
};

const DEFAULT_GENERIC_FAMILIES: &[(GenericFamily, &[&str])] = &[
    (GenericFamily::Serif, &["Times", "Times New Roman"]),
    (GenericFamily::SansSerif, &["Helvetica"]),
    (GenericFamily::Monospace, &["Courier", "Courier New"]),
    (GenericFamily::Cursive, &["Apple Chancery"]),
    (GenericFamily::Fantasy, &["Papyrus"]),
    (GenericFamily::SystemUi, &["System Font", ".SF NS"]),
    (GenericFamily::Emoji, &["Apple Color Emoji"]),
    (GenericFamily::Math, &["STIX Two Math"]),
];

pub struct SystemFonts {
    pub name_map: Arc<FamilyNameMap>,
    pub generic_families: Arc<GenericFamilyMap>,
    family_map: HashMap<FamilyId, FamilyInfo>,
}

impl SystemFonts {
    pub fn new() -> Self {
        let scanned = scan::ScannedCollection::from_paths(Some("/System/Library/Fonts"), 8);
        let name_map = scanned.family_names;
        let mut generic_families = GenericFamilyMap::default();
        for (family, names) in DEFAULT_GENERIC_FAMILIES {
            generic_families.set(
                *family,
                names
                    .iter()
                    .filter_map(|name| name_map.get(name))
                    .map(|name| name.id()),
            );
        }
        Self {
            name_map: Arc::new(name_map),
            generic_families: Arc::new(generic_families),
            family_map: scanned.families,
        }
    }

    pub fn family(&mut self, id: FamilyId) -> Option<FamilyInfo> {
        self.family_map.get(&id).cloned()
    }

    pub fn fallback(&mut self, key: impl Into<FallbackKey>) -> Option<FamilyId> {
        let key = key.into();
        let sample = key.script().sample()?;
        self.fallback_for_text(sample, key.locale(), false)
    }
}

impl SystemFonts {
    fn fallback_for_text(
        &mut self,
        text: &str,
        locale: Option<&str>,
        prefer_ui: bool,
    ) -> Option<FamilyId> {
        let font = fallback_for_text(text, locale, prefer_ui)?;
        self.name_map.get(&font.family_name()).map(|n| n.id())
    }
}

fn fallback_for_text(text: &str, locale: Option<&str>, prefer_ui: bool) -> Option<CTFont> {
    let cf_text = CFString::new(text);
    let cf_text_range = CFRange::init(0, cf_text.char_len());
    let cf_locale = locale.map(CFString::new);
    let base_font = {
        let desc_attrs = CFDictionary::from_CFType_pairs(&[]);
        let mut desc = font_descriptor::new_from_attributes(&desc_attrs);
        if prefer_ui {
            if let Some(ui_desc) = ui_font_for_language(kCTFontSystemFontType, 0.0, None)
                .map(|font| font.copy_descriptor())
            {
                desc = ui_desc;
            }
        }
        font::new_from_descriptor(&desc, 0.0)
    };
    let font = unsafe {
        CTFont::wrap_under_create_rule(if let Some(locale) = cf_locale {
            CTFontCreateForStringWithLanguage(
                base_font.as_concrete_TypeRef(),
                cf_text.as_concrete_TypeRef(),
                cf_text_range,
                locale.as_concrete_TypeRef(),
            )
        } else {
            CTFontCreateForString(
                base_font.as_concrete_TypeRef(),
                cf_text.as_concrete_TypeRef(),
                cf_text_range,
            )
        })
    };
    Some(font)
}

fn ui_font_for_language(
    ui_type: CTFontUIFontType,
    size: f64,
    language: Option<CFString>,
) -> Option<CTFont> {
    unsafe {
        let font_ref = CTFontCreateUIFontForLanguage(
            ui_type,
            size,
            language
                .as_ref()
                .map(|x| x.as_concrete_TypeRef())
                .unwrap_or(std::ptr::null()),
        );
        if font_ref.is_null() {
            None
        } else {
            Some(CTFont::wrap_under_create_rule(font_ref))
        }
    }
}

#[link(name = "CoreText", kind = "framework")]
extern "C" {
    fn CTFontCreateUIFontForLanguage(
        uiType: CTFontUIFontType,
        size: f64,
        language: CFStringRef,
    ) -> CTFontRef;

    fn CTFontCreateForString(
        currentFont: CTFontRef,
        string: CFStringRef,
        range: CFRange,
    ) -> CTFontRef;

    fn CTFontCreateForStringWithLanguage(
        currentFont: CTFontRef,
        string: CFStringRef,
        range: CFRange,
        language: CFStringRef,
    ) -> CTFontRef;
}
