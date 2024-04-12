use std::{path::Path, str::FromStr, sync::Arc};

use hashbrown::HashMap;
use roxmltree::Document;

use super::{
    scan, FallbackKey, FamilyId, FamilyInfo, FamilyNameMap, GenericFamily, GenericFamilyMap,
};

// TODO: Use actual generic families here, where available, when fonts.xml is properly parsed.
//       system-ui should map to `variant="compact"` in some scripts during fallback resolution.
const DEFAULT_GENERIC_FAMILIES: &[(GenericFamily, &[&str])] = &[
    (
        GenericFamily::SansSerif,
        &["Roboto Flex", "Roboto", "Noto Sans"],
    ),
    (GenericFamily::Serif, &["Noto Serif"]),
    (GenericFamily::Monospace, &["monospace"]),
    (GenericFamily::Cursive, &["Dancing Script"]),
    (GenericFamily::Fantasy, &["Noto Serif"]),
    (
        GenericFamily::SystemUi,
        &["Roboto Flex", "Roboto", "Noto Sans"],
    ),
    (GenericFamily::Emoji, &["Noto Color Emoji"]),
    (GenericFamily::Math, &["Noto Sans Math", "Noto Sans"]),
];

pub struct SystemFonts {
    pub name_map: Arc<FamilyNameMap>,
    pub generic_families: Arc<GenericFamilyMap>,
    family_map: HashMap<FamilyId, FamilyInfo>,
}

impl SystemFonts {
    pub fn new() -> Self {
        let android_root: String = std::env::var("ANDROID_ROOT").unwrap_or("/system".to_string());

        let scan::ScannedCollection {
            family_names: mut name_map,
            families: family_map,
            ..
        } = scan::ScannedCollection::from_paths(Path::new(&android_root).join("fonts").to_str(), 8);
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

        // Try to get generic info from fonts.xml
        if let Ok(s) = std::fs::read_to_string(Path::new(&android_root).join("etc/fonts.xml")) {
            if let Ok(doc) = Document::parse(s.clone().as_str()) {
                let root = doc.root_element();
                if root.tag_name().name() == "familyset"
                    || root
                        .attribute("version")
                        .is_some_and(|v| usize::from_str(v).is_ok_and(|x| x >= 21))
                {
                    for child in root.children() {
                        match child.tag_name().name() {
                            "alias" => {
                                if let Some((name, to)) =
                                    child.attribute("name").zip(child.attribute("to"))
                                {
                                    if child.attribute("weight").is_some() {
                                        // weight aliases are an Android quirk and are not in­
                                        // teresting for use cases other than Android legacy.
                                        continue;
                                    }
                                    let to_n = name_map.get_or_insert(to);
                                    name_map.add_alias(to_n.id(), name);
                                }
                            }
                            "family" => {
                                if let Some(name) = child.attribute("name") {
                                    let f = name_map.get_or_insert(name);
                                    let _id = f.id();
                                    for _child in child.children() {
                                        // TODO: map using postScriptName when available other­
                                        //       wise use the file name, and perhaps if necess­
                                        //       ary (e.g. if it's a collection), do something
                                        //       smarter, or something dumb that meets expecta­
                                        //       tions on Android.
                                    }
                                } else if let Some(_langs) = child
                                    .attribute("lang")
                                    .map(|s| s.split(',').collect::<Vec<&str>>())
                                {
                                    // TODO: implement language fallback "family" elements
                                }
                                // TODO: interpret variant="compact" without fallbackFor as a
                                //       fallback for system-ui, as falling back to a
                                //       variant="elegant" for system-ui can mess up a layout
                                //       in some scripts.
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        Self {
            name_map: Arc::new(name_map),
            generic_families: Arc::new(generic_families),
            family_map,
        }
    }

    pub fn family(&mut self, id: FamilyId) -> Option<FamilyInfo> {
        self.family_map.get(&id).cloned()
    }

    pub fn fallback(&mut self, _key: impl Into<FallbackKey>) -> Option<FamilyId> {
        // FIXME: This is a stub
        Some(self.generic_families.get(GenericFamily::SansSerif)[0])
    }
}
