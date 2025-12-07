// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{
    FallbackKey, FamilyId, FamilyInfo, FamilyNameMap, GenericFamily, GenericFamilyMap, scan,
};
use alloc::sync::Arc;
use core::ptr::{null, null_mut};
use hashbrown::HashMap;
use objc2_core_foundation::{CFDictionary, CFRange, CFRetained, CFString};
use objc2_core_text::{CTFont, CTFontDescriptor, CTFontUIFontType};
use objc2_foundation::{
    NSSearchPathDirectory, NSSearchPathDomainMask, NSSearchPathForDirectoriesInDomains,
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

pub(crate) struct SystemFonts {
    pub(crate) name_map: Arc<FamilyNameMap>,
    pub(crate) generic_families: Arc<GenericFamilyMap>,
    family_map: HashMap<FamilyId, FamilyInfo>,
}

impl SystemFonts {
    pub(crate) fn new() -> Self {
        let paths = NSSearchPathForDirectoriesInDomains(
            NSSearchPathDirectory::LibraryDirectory,
            NSSearchPathDomainMask::AllDomainsMask,
            true,
        )
        .into_iter()
        .map(|p| format!("{p}/Fonts/"));
        let scanned = scan::ScannedCollection::from_paths(paths, 8);
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

    pub(crate) fn family(&mut self, id: FamilyId) -> Option<FamilyInfo> {
        self.family_map.get(&id).cloned()
    }

    pub(crate) fn fallback(&mut self, key: impl Into<FallbackKey>) -> Option<FamilyId> {
        let key = key.into();
        let sample = key.script().sample()?;
        let font = create_fallback_font_for_text(sample, key.locale(), false)?;
        let family_name = unsafe { font.family_name() };
        self.name_map.get(&family_name.to_string()).map(|n| n.id())
    }
}

fn create_base_font(prefer_ui_font: bool) -> CFRetained<CTFont> {
    if prefer_ui_font {
        if let Some(font) =
            unsafe { CTFont::new_ui_font_for_language(CTFontUIFontType::System, 0.0, None) }
        {
            return font;
        }
    }
    unsafe {
        let attrs = CFDictionary::new(None, null_mut(), null_mut(), 0, null(), null());
        let desc = CTFontDescriptor::with_attributes(&attrs.unwrap());
        CTFont::with_font_descriptor(&desc, 0.0, null())
    }
}

fn create_fallback_font_for_text(
    text: &str,
    locale: Option<&str>,
    prefer_ui_font: bool,
) -> Option<CFRetained<CTFont>> {
    let text = CFString::from_str(text);
    let text_range = CFRange {
        location: 0,
        length: text.length(),
    };
    let locale = locale.map(CFString::from_str);
    let base_font = create_base_font(prefer_ui_font);
    let font = unsafe {
        if let Some(locale) = locale {
            CTFont::for_string_with_language(&base_font, &text, text_range, Some(&locale))
        } else {
            CTFont::for_string(&base_font, &text, text_range)
        }
    };
    Some(font)
}
