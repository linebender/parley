// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use hashbrown::HashMap;
use std::{path::PathBuf, sync::Arc};

use super::{
    super::{Stretch, Style, Weight},
    FallbackKey, FamilyId, FamilyInfo, FamilyName, FamilyNameMap, FontInfo, GenericFamily,
    GenericFamilyMap, Script, SourceInfo, SourcePathMap,
};

mod cache;
mod config;

/// Raw access to the collection of local system fonts.
pub struct SystemFonts {
    pub name_map: Arc<FamilyNameMap>,
    pub generic_families: Arc<GenericFamilyMap>,
    raw_families: HashMap<FamilyId, RawFamily>,
    family_map: HashMap<FamilyId, Option<FamilyInfo>>,
    fallback_map: HashMap<Script, FallbackFamilies>,
}

impl SystemFonts {
    pub fn new() -> Self {
        Self::try_new().unwrap_or_else(|| Self {
            name_map: Default::default(),
            generic_families: Default::default(),
            raw_families: Default::default(),
            family_map: Default::default(),
            fallback_map: Default::default(),
        })
    }

    pub fn family(&mut self, id: FamilyId) -> Option<FamilyInfo> {
        match self.family_map.get(&id) {
            Some(Some(family)) => return Some(family.clone()),
            Some(None) => return None,
            None => {}
        }
        let raw_family = self.raw_families.get(&id)?;
        if raw_family.fonts.is_empty() {
            // TODO: maybe catch this earlier?
            return None;
        }
        let mut fonts: smallvec::SmallVec<[FontInfo; 4]> = Default::default();
        fonts.reserve(raw_family.fonts.len());
        fonts.extend(raw_family.fonts.iter().filter_map(|font| {
            let mut info = FontInfo::from_source(font.source.clone(), font.index);
            if let Some(info) = info.as_mut() {
                info.maybe_override_attributes(font.stretch, font.style, font.weight);
            }
            info
        }));
        if fonts.is_empty() {
            self.family_map.insert(id, None);
            return None;
        }
        let family = FamilyInfo::new(raw_family.name.clone(), fonts);
        self.family_map.insert(id, Some(family.clone()));
        Some(family)
    }

    pub fn fallback(&mut self, key: impl Into<FallbackKey>) -> Option<FamilyId> {
        let key = key.into();
        let script = key.script();
        let locale = key.locale();
        let families = self.fallback_map.get(&script)?;
        let style = StyleClass::SansSerif;
        if let Some(locale) = locale {
            if !key.is_default() {
                if let Some(family) = families.select_lang(locale, style) {
                    return Some(family);
                }
            }
        }
        families.select_default(style)
    }
}

impl SystemFonts {
    pub fn try_new() -> Option<Self> {
        let mut name_map = FamilyNameMap::default();
        let mut generic_families = GenericFamilyMap::default();
        let mut source_map = SourcePathMap::default();
        let mut raw_families: HashMap<_, _> = Default::default();
        let mut fallback_map: HashMap<Script, FallbackFamilies> = Default::default();
        // First, parse the raw config files
        let mut config = Config::default();
        config::parse_config("/etc/fonts/fonts.conf".as_ref(), &mut config);
        if let Ok(xdg_config_home) = std::env::var("XDG_CONFIG_HOME") {
            config::parse_config(
                std::path::PathBuf::from(xdg_config_home)
                    .as_path()
                    .join("fontconfig/fonts.conf")
                    .as_path(),
                &mut config,
            );
        } else if let Ok(user_home) = std::env::var("HOME") {
            config::parse_config(
                std::path::PathBuf::from(user_home)
                    .as_path()
                    .join(".config/fontconfig/fonts.conf")
                    .as_path(),
                &mut config,
            );
        }

        // Extract all font/family metadata from the cache files
        cache::parse_caches(&config.cache_dirs, |font| {
            // Only accept OpenType fonts
            if let Some(ext) = font.path.extension().and_then(|ext| ext.to_str()) {
                if !["ttf", "otf", "ttc", "otc"].contains(&ext) {
                    return;
                }
            } else {
                return;
            }
            let [first_name, other_names @ ..] = font.family.as_slice() else {
                return;
            };
            let family_name = name_map.get_or_insert(strip_rbiz(first_name));
            let id = family_name.id();
            for other_name in other_names {
                name_map.add_alias(id, strip_rbiz(other_name));
            }
            let raw_family = raw_families.entry(id).or_insert_with(|| RawFamily {
                name: family_name,
                fonts: vec![],
            });
            let source = source_map.get_or_insert(&font.path);
            if raw_family
                .fonts
                .iter()
                .any(|raw_font| raw_font.source.id == source.id && raw_font.index == font.index)
            {
                return;
            }
            raw_family.fonts.push(RawFont {
                source,
                index: font.index,
                stretch: font.stretch,
                style: font.style,
                weight: font.weight,
                coverage: font.coverage.clone(),
            });
        });
        // Build the fallback map, dropping non-existent families
        for (lang, class, family) in &config.lang_maps {
            let Some(family_id) = name_map.get(strip_rbiz(family)).map(|f| f.id()) else {
                continue;
            };
            let class = *class;
            let Some(scripts) = lang_to_scripts(lang) else {
                continue;
            };
            for &script in scripts {
                let script = Script(*script);
                let key: FallbackKey = (script, lang.as_str()).into();
                let families = fallback_map.entry(script).or_default();
                if key.is_default() || key.locale().is_none() {
                    families.default.push((class, family_id));
                } else if let Some(locale) = key.locale() {
                    families.languages.push((locale, class, family_id));
                }
            }
        }
        // Build the generic map, also dropping non-existent families
        for family in GenericFamily::all() {
            let i = *family as usize;
            generic_families.append(
                *family,
                config.generics[i]
                    .iter()
                    .filter_map(|name| name_map.get(strip_rbiz(name)))
                    .map(|name| name.id()),
            );
        }
        let mut result = Self {
            name_map: Arc::new(name_map),
            generic_families: Arc::new(generic_families),
            raw_families,
            family_map: Default::default(),
            fallback_map,
        };
        result.load_additional_fallbacks();
        Some(result)
    }

    fn load_additional_fallbacks(&mut self) {
        // Check for missing scripts and extend the fallbacks based on coverage
        for (script, sample_text) in Script::all_samples() {
            if self.fallback_map.contains_key(script) {
                continue;
            }
            if let Some(family) = self.find_best_family(sample_text) {
                self.fallback_map
                    .entry(*script)
                    .or_default()
                    .default
                    .push((StyleClass::None, family));
            }
        }
    }

    fn find_best_family(&self, text: &str) -> Option<FamilyId> {
        for family in [GenericFamily::SansSerif, GenericFamily::Serif] {
            if let Some(family) = find_best_family(
                self.generic_families
                    .get(family)
                    .iter()
                    .filter_map(|id| self.raw_families.get(id)),
                text,
            ) {
                return Some(family);
            }
        }
        find_best_family(self.raw_families.values(), text)
    }
}

/// FontConfig seems to force RBIZ (regular, bold, italic, bold italic) when
/// categorizing fonts. This removes those suffixes from family names so that
/// we can match on all attributes.
fn strip_rbiz(name: &str) -> &str {
    const SUFFIXES: &[&str] = &[
        " Thin",
        " ExtraLight",
        " DemiLight",
        " Light",
        " Medium",
        " Black",
        " Light",
        " ExtraLight",
        " Medium",
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

fn find_best_family<'a>(
    raw_families: impl Iterator<Item = &'a RawFamily>,
    text: &str,
) -> Option<FamilyId> {
    let text_len = text.len();
    let mut best_id = None;
    let mut best_coverage = 0;
    for family in raw_families {
        let id = family.name.id();
        for font in &family.fonts {
            let coverage = font.coverage.compute_for_str(text);
            if coverage == text_len {
                return Some(id);
            }
            if coverage > best_coverage {
                best_id = Some(id);
                best_coverage = coverage;
            }
        }
    }
    best_id
}

struct RawFamily {
    name: FamilyName,
    fonts: Vec<RawFont>,
}

struct RawFont {
    source: SourceInfo,
    index: u32,
    stretch: Stretch,
    style: Style,
    weight: Weight,
    coverage: cache::Coverage,
}

#[derive(Default)]
struct Config {
    cache_dirs: Vec<PathBuf>,
    generics: [Vec<String>; 13],
    lang_maps: Vec<(String, StyleClass, String)>,
}

impl config::ParserSink for Config {
    fn alias(&mut self, family: &str, prefer: &[&str]) {
        if let Some(generic) = super::GenericFamily::parse(family) {
            self.generics[generic as usize].extend(prefer.iter().map(|s| s.to_string()));
        }
    }

    fn include_path(&mut self, _path: &std::path::Path) {}

    fn cache_path(&mut self, path: &std::path::Path) {
        self.cache_dirs.push(path.into());
    }

    fn lang_map(&mut self, lang: &str, from_family: Option<&str>, family: &str) {
        let class = match from_family {
            Some("sans-serif") => StyleClass::SansSerif,
            Some("serif") => StyleClass::Serif,
            Some("monospace") => StyleClass::Monospace,
            _ => StyleClass::None,
        };
        self.lang_maps.push((lang.into(), class, family.into()));
    }
}

fn lang_to_scripts(lang: &str) -> Option<&'static [&'static [u8; 4]]> {
    let ix = LANG_TO_SCRIPTS.binary_search_by(|x| x.0.cmp(lang)).ok()?;
    Some(LANG_TO_SCRIPTS.get(ix)?.1)
}

const LANG_TO_SCRIPTS: &[(&str, &[&[u8; 4]])] = &[
    ("am", &[b"Ethi"]),
    ("ar", &[b"Arab"]),
    ("as", &[b"Beng"]),
    ("az-ir", &[b"Arab"]),
    ("ber-ma", &[b"Tfng"]),
    ("bh", &[b"Deva"]),
    ("bho", &[b"Deva"]),
    ("bn", &[b"Beng"]),
    ("bo", &[b"Tibt"]),
    ("brx", &[b"Deva"]),
    ("byn", &[b"Ethi"]),
    ("chr", &[b"Cher"]),
    ("doi", &[b"Deva"]),
    ("dv", &[b"Thaa"]),
    ("dz", &[b"Tibt"]),
    ("el", &[b"Grek"]),
    ("fa", &[b"Arab"]),
    ("gez", &[b"Ethi"]),
    ("gu", &[b"Gujr"]),
    ("he", &[b"Hebr"]),
    ("hi", &[b"Deva"]),
    ("hne", &[b"Deva"]),
    ("hy", &[b"Armn"]),
    ("ii", &[b"Yiii"]),
    ("iu", &[b"Cans"]),
    ("ja", &[b"Hani", b"Kana", b"Hira"]),
    ("ka", &[b"Geor"]),
    ("km", &[b"Khmr"]),
    ("kn", &[b"Knda"]),
    ("ko", &[b"Hani", b"Hang"]),
    ("kok", &[b"Deva"]),
    ("ks", &[b"Arab"]),
    ("ku-iq", &[b"Arab"]),
    ("ku-ir", &[b"Arab"]),
    ("lah", &[b"Arab"]),
    ("lo", &[b"Laoo"]),
    ("mai", &[b"Deva"]),
    ("ml", &[b"Mlym"]),
    ("mn-cn", &[b"Mong"]),
    ("mni", &[b"Beng"]),
    ("mr", &[b"Deva"]),
    ("my", &[b"Mymr"]),
    ("ne", &[b"Deva"]),
    ("nqo", &[b"Nkoo"]),
    ("or", &[b"Orya"]),
    ("ota", &[b"Arab"]),
    ("pa", &[b"Guru"]),
    ("pa-pk", &[b"Arab"]),
    ("ps-af", &[b"Arab"]),
    ("ps-pk", &[b"Arab"]),
    ("sa", &[b"Deva"]),
    ("sat", &[b"Deva"]),
    ("sd", &[b"Arab"]),
    ("si", &[b"Sinh"]),
    ("sid", &[b"Ethi"]),
    ("syr", &[b"Syrc"]),
    ("ta", &[b"Taml"]),
    ("te", &[b"Telu"]),
    ("th", &[b"Thai"]),
    ("ti-er", &[b"Ethi"]),
    ("ti-et", &[b"Ethi"]),
    ("tig", &[b"Ethi"]),
    ("ug", &[b"Arab"]),
    ("ur", &[b"Arab"]),
    ("wal", &[b"Ethi"]),
    ("yi", &[b"Hebr"]),
    ("zh-cn", &[b"Hani"]),
    ("zh-hk", &[b"Hani"]),
    ("zh-mo", &[b"Hani"]),
    ("zh-sg", &[b"Hani"]),
    ("zh-tw", &[b"Hani"]),
];

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum StyleClass {
    None,
    SansSerif,
    Serif,
    Monospace,
}

impl StyleClass {
    fn rank(self, requested: Self) -> u32 {
        let from = if self == Self::None {
            Self::SansSerif
        } else {
            self
        };
        let requested = if requested == Self::None {
            Self::SansSerif
        } else {
            requested
        };
        match from {
            Self::SansSerif => match requested {
                Self::SansSerif => 3,
                Self::Serif => 2,
                _ => 1,
            },
            Self::Serif => match requested {
                Self::Serif => 3,
                Self::SansSerif => 2,
                _ => 1,
            },
            Self::Monospace => match requested {
                Self::Monospace => 3,
                Self::SansSerif => 2,
                _ => 1,
            },
            _ => 1,
        }
    }
}

#[derive(Default, Debug)]
pub struct FallbackFamilies {
    /// Default list of font families for the script.
    pub default: Vec<(StyleClass, FamilyId)>,
    /// Language specific font families for the script.
    pub languages: Vec<(&'static str, StyleClass, FamilyId)>,
}

impl FallbackFamilies {
    fn select_default(&self, style: StyleClass) -> Option<FamilyId> {
        let mut selected_rank = 0;
        let mut selected_ix = 0;
        for (i, family) in self.default.iter().enumerate() {
            let rank = family.0.rank(style);
            if rank > selected_rank {
                selected_rank = rank;
                selected_ix = i;
            }
        }
        if selected_rank != 0 {
            Some(self.default[selected_ix].1)
        } else {
            None
        }
    }

    fn select_lang(&self, lang: &str, style: StyleClass) -> Option<FamilyId> {
        let mut selected_rank = 0;
        let mut selected_ix = 0;
        for (i, family) in self
            .languages
            .iter()
            .enumerate()
            .filter(|(_, family)| family.0 == lang)
        {
            let rank = family.1.rank(style);
            if rank > selected_rank {
                selected_rank = rank;
                selected_ix = i;
            }
        }
        if selected_rank != 0 {
            Some(self.languages[selected_ix].2)
        } else {
            None
        }
    }
}
