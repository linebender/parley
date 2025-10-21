// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use icu_properties::props::Script;
use icu_provider::prelude::icu_locale_core::LanguageIdentifier;

pub(crate) fn script_to_fontique(script: Script) -> fontique::Script {
    fontique::Script(*FONTIQUE_SCRIPT_TAGS.get(script.to_icu4c_value() as usize).unwrap_or(b"Zzzz"))
}

pub(crate) fn script_to_harfrust(script: Script) -> harfrust::Script {
    harfrust::Script::from_iso15924_tag(harfrust::Tag::new(
        FONTIQUE_SCRIPT_TAGS.get(script.to_icu4c_value() as usize).unwrap_or(b"Zzzz"),
    ))
        .unwrap_or(harfrust::script::UNKNOWN)
}

pub(crate) fn locale_to_fontique(locale: LanguageIdentifier) -> Option<fontique::Language> {
    fontique::Language::try_from_utf8(locale.to_string().as_bytes()).ok()
}

#[rustfmt::skip]
const FONTIQUE_SCRIPT_TAGS: [[u8; 4]; 193] = [
    *b"Zyyy", *b"Zinh", *b"Arab", *b"Armn", *b"Beng", *b"Bopo", *b"Cher", *b"Copt", *b"Cyrl",
    *b"Dsrt", *b"Deva", *b"Ethi", *b"Geor", *b"Goth", *b"Grek", *b"Gujr", *b"Guru", *b"Hani",
    *b"Hang", *b"Hebr", *b"Hira", *b"Knda", *b"Kana", *b"Khmr", *b"Laoo", *b"Latn", *b"Mlym",
    *b"Mong", *b"Mymr", *b"Ogam", *b"Ital", *b"Orya", *b"Runr", *b"Sinh", *b"Syrc", *b"Taml",
    *b"Telu", *b"Thaa", *b"Thai", *b"Tibt", *b"Cans", *b"Yiii", *b"Tglg", *b"Hano", *b"Buhd",
    *b"Tagb", *b"Brai", *b"Cprt", *b"Limb", *b"Linb", *b"Osma", *b"Shaw", *b"Tale", *b"Ugar",
    *b"Zzzz", *b"Bugi", *b"Glag", *b"Khar", *b"Sylo", *b"Talu", *b"Tfng", *b"Xpeo", *b"Bali",
    *b"Batk", *b"Zzzz", *b"Brah", *b"Cham", *b"Zzzz", *b"Zzzz", *b"Zzzz", *b"Zzzz", *b"Egyp",
    *b"Zzzz", *b"Zzzz", *b"Zzzz", *b"Hmng", *b"Hung", *b"Zzzz", *b"Java", *b"Kali", *b"Zzzz",
    *b"Zzzz", *b"Lepc", *b"Lina", *b"Mand", *b"Zzzz", *b"Mero", *b"Nkoo", *b"Orkh", *b"Perm",
    *b"Phag", *b"Phnx", *b"Plrd", *b"Zzzz", *b"Zzzz", *b"Zzzz", *b"Zzzz", *b"Zzzz", *b"Zzzz",
    *b"Vaii", *b"Zzzz", *b"Xsux", *b"Zzzz", *b"Zzzz", *b"Cari", *b"Zzzz", *b"Lana", *b"Lyci",
    *b"Lydi", *b"Olck", *b"Rjng", *b"Saur", *b"Sgnw", *b"Sund", *b"Zzzz", *b"Mtei", *b"Armi",
    *b"Avst", *b"Cakm", *b"Zzzz", *b"Kthi", *b"Mani", *b"Phli", *b"Phlp", *b"Zzzz", *b"Prti",
    *b"Samr", *b"Tavt", *b"Zzzz", *b"Zzzz", *b"Bamu", *b"Lisu", *b"Zzzz", *b"Sarb", *b"Bass",
    *b"Dupl", *b"Elba", *b"Gran", *b"Zzzz", *b"Zzzz", *b"Mend", *b"Merc", *b"Narb", *b"Nbat",
    *b"Palm", *b"Sind", *b"Wara", *b"Zzzz", *b"Zzzz", *b"Mroo", *b"Nshu", *b"Shrd", *b"Sora",
    *b"Takr", *b"Tang", *b"Zzzz", *b"Hluw", *b"Khoj", *b"Tirh", *b"Aghb", *b"Mahj", *b"Ahom",
    *b"Hatr", *b"Modi", *b"Mult", *b"Pauc", *b"Sidd", *b"Adlm", *b"Bhks", *b"Marc", *b"Newa",
    *b"Osge", *b"Zzzz", *b"Zzzz", *b"Zzzz", *b"Gonm", *b"Soyo", *b"Zanb", *b"Dogr", *b"Gong",
    *b"Maka", *b"Medf", *b"Rohg", *b"Sogd", *b"Sogo", *b"Elym", *b"Hmnp", *b"Nand", *b"Wcho",
    *b"Chrs", *b"Diak", *b"Kits", *b"Yezi",
];