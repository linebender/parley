// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub fn script_to_fontique(script: swash::text::Script) -> fontique::Script {
    fontique::Script(*SCRIPT_TAGS.get(script as usize).unwrap_or(b"Zzzz"))
}

pub fn locale_to_fontique(locale: swash::text::Language) -> Option<fontique::Language> {
    let mut buf = [0u8; 16];
    let mut len = 0;
    for byte in locale.language().bytes() {
        buf[len] = byte;
        len += 1;
    }
    if let Some(subtag) = locale.script() {
        buf[len] = b'-';
        len += 1;
        for byte in subtag.bytes() {
            buf[len] = byte;
            len += 1;
        }
    }
    if let Some(subtag) = locale.region() {
        buf[len] = b'-';
        len += 1;
        for byte in subtag.bytes() {
            buf[len] = byte;
            len += 1;
        }
    }
    fontique::Language::try_from_bytes(&buf[..len]).ok()
}

pub fn synthesis_to_swash(synthesis: fontique::Synthesis) -> swash::Synthesis {
    swash::Synthesis::new(
        synthesis
            .variation_settings()
            .iter()
            .map(|setting| swash::Setting {
                tag: swash::tag_from_bytes(&setting.0.to_be_bytes()),
                value: setting.1,
            }),
        synthesis.embolden(),
        synthesis.skew().unwrap_or_default(),
    )
}

#[rustfmt::skip]
const SCRIPT_TAGS: [[u8; 4]; 157] = [
    *b"Adlm", *b"Aghb", *b"Ahom", *b"Arab", *b"Armi", *b"Armn", *b"Avst", *b"Bali", *b"Bamu",
    *b"Bass", *b"Batk", *b"Beng", *b"Bhks", *b"Bopo", *b"Brah", *b"Brai", *b"Bugi", *b"Buhd",
    *b"Cakm", *b"Cans", *b"Cari", *b"Cham", *b"Cher", *b"Chrs", *b"Copt", *b"Cprt", *b"Cyrl",
    *b"Deva", *b"Diak", *b"Dogr", *b"Dsrt", *b"Dupl", *b"Egyp", *b"Elba", *b"Elym", *b"Ethi",
    *b"Geor", *b"Glag", *b"Gong", *b"Gonm", *b"Goth", *b"Gran", *b"Grek", *b"Gujr", *b"Guru",
    *b"Hang", *b"Hani", *b"Hano", *b"Hatr", *b"Hebr", *b"Hira", *b"Hluw", *b"Hmng", *b"Hmnp",
    *b"Hung", *b"Ital", *b"Java", *b"Kali", *b"Kana", *b"Khar", *b"Khmr", *b"Khoj", *b"Kits",
    *b"Knda", *b"Kthi", *b"Lana", *b"Laoo", *b"Latn", *b"Lepc", *b"Limb", *b"Lina", *b"Linb",
    *b"Lisu", *b"Lyci", *b"Lydi", *b"Mahj", *b"Maka", *b"Mand", *b"Mani", *b"Marc", *b"Medf",
    *b"Mend", *b"Merc", *b"Mero", *b"Mlym", *b"Modi", *b"Mong", *b"Mroo", *b"Mtei", *b"Mult",
    *b"Mymr", *b"Nand", *b"Narb", *b"Nbat", *b"Newa", *b"Nkoo", *b"Nshu", *b"Ogam", *b"Olck",
    *b"Orkh", *b"Orya", *b"Osge", *b"Osma", *b"Palm", *b"Pauc", *b"Perm", *b"Phag", *b"Phli",
    *b"Phlp", *b"Phnx", *b"Plrd", *b"Prti", *b"Rjng", *b"Rohg", *b"Runr", *b"Samr", *b"Sarb",
    *b"Saur", *b"Sgnw", *b"Shaw", *b"Shrd", *b"Sidd", *b"Sind", *b"Sinh", *b"Sogd", *b"Sogo",
    *b"Sora", *b"Soyo", *b"Sund", *b"Sylo", *b"Syrc", *b"Tagb", *b"Takr", *b"Tale", *b"Talu",
    *b"Taml", *b"Tang", *b"Tavt", *b"Telu", *b"Tfng", *b"Tglg", *b"Thaa", *b"Thai", *b"Tibt",
    *b"Tirh", *b"Ugar", *b"Vaii", *b"Wara", *b"Wcho", *b"Xpeo", *b"Xsux", *b"Yezi", *b"Yiii",
    *b"Zanb", *b"Zinh", *b"Zyyy", *b"Zzzz",
];
