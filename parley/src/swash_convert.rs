// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

// TODO(conor) Rename to icu_convert

pub(crate) fn script_to_fontique(script: icu_properties::props::Script) -> fontique::Script {
    fontique::Script(*FONTIQUE_SCRIPT_TAGS.get(script_icu_to_swash(script) as usize).unwrap_or(b"Zzzz"))
}

pub(crate) fn script_icu_to_swash(script: icu_properties::props::Script) -> swash::text::Script {
    script_from_u8(script.to_icu4c_value() as u8).unwrap()
}

pub(crate) fn script_icu_to_harfrust(script: icu_properties::props::Script) -> harfrust::Script {
    harfrust::Script::from_iso15924_tag(harfrust::Tag::new(
        FONTIQUE_SCRIPT_TAGS.get(script_icu_to_swash(script) as usize).unwrap_or(b"Zzzz"),
    ))
    .unwrap_or(harfrust::script::UNKNOWN)
}

pub(crate) fn locale_to_fontique(locale: swash::text::Language) -> Option<fontique::Language> {
    let mut buf = [0_u8; 16];
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

pub(crate) fn locale_icu_to_fontique(locale: icu::locale::LanguageIdentifier) -> Option<fontique::Language> {
    let mut buf = [0_u8; 16];
    let mut len = 0;
    for byte in locale.language.into_raw() {
        buf[len] = byte;
        len += 1;
    }
    if let Some(subtag) = locale.script {
        buf[len] = b'-';
        len += 1;
        for byte in subtag.into_raw() {
            buf[len] = byte;
            len += 1;
        }
    }
    if let Some(subtag) = locale.region {
        buf[len] = b'-';
        len += 1;
        // TODO(conor) icu has 3 byte regions, Swash only had 2
        for byte in subtag.into_raw() {
            buf[len] = byte;
            len += 1;
        }
    }
    fontique::Language::try_from_bytes(&buf[..len]).ok()
}

// TODO(conor) This should map from icu::Script::to_icu4c() values -> Fontique script tags
#[rustfmt::skip]
const FONTIQUE_SCRIPT_TAGS: [[u8; 4]; 157] = [
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

// TODO(conor) shouldn't need this with reworked FONTIQUE_SCRIPT_TAGS
fn script_from_u8(value: u8) -> Option<swash::text::Script> {
    match value {
        0 => Some(swash::text::Script::Common),
        1 => Some(swash::text::Script::Inherited),
        2 => Some(swash::text::Script::Arabic),
        3 => Some(swash::text::Script::Armenian),
        4 => Some(swash::text::Script::Bengali),
        5 => Some(swash::text::Script::Bopomofo),
        6 => Some(swash::text::Script::Cherokee),
        7 => Some(swash::text::Script::Coptic),
        8 => Some(swash::text::Script::Cyrillic),
        9 => Some(swash::text::Script::Deseret),
        10 => Some(swash::text::Script::Devanagari),
        11 => Some(swash::text::Script::Ethiopic),
        12 => Some(swash::text::Script::Georgian),
        13 => Some(swash::text::Script::Gothic),
        14 => Some(swash::text::Script::Greek),
        15 => Some(swash::text::Script::Gujarati),
        16 => Some(swash::text::Script::Gurmukhi),
        17 => Some(swash::text::Script::Han),
        18 => Some(swash::text::Script::Hangul),
        19 => Some(swash::text::Script::Hebrew),
        20 => Some(swash::text::Script::Hiragana),
        21 => Some(swash::text::Script::Kannada),
        22 => Some(swash::text::Script::Katakana),
        23 => Some(swash::text::Script::Khmer),
        24 => Some(swash::text::Script::Lao),
        25 => Some(swash::text::Script::Latin),
        26 => Some(swash::text::Script::Malayalam),
        27 => Some(swash::text::Script::Mongolian),
        28 => Some(swash::text::Script::Myanmar),
        29 => Some(swash::text::Script::Ogham),
        30 => Some(swash::text::Script::OldItalic),
        31 => Some(swash::text::Script::Oriya),
        32 => Some(swash::text::Script::Runic),
        33 => Some(swash::text::Script::Sinhala),
        34 => Some(swash::text::Script::Syriac),
        35 => Some(swash::text::Script::Tamil),
        36 => Some(swash::text::Script::Telugu),
        37 => Some(swash::text::Script::Thaana),
        38 => Some(swash::text::Script::Thai),
        39 => Some(swash::text::Script::Tibetan),
        40 => Some(swash::text::Script::CanadianAboriginal),
        41 => Some(swash::text::Script::Yi),
        42 => Some(swash::text::Script::Tagalog),
        43 => Some(swash::text::Script::Hanunoo),
        44 => Some(swash::text::Script::Buhid),
        45 => Some(swash::text::Script::Tagbanwa),
        46 => Some(swash::text::Script::Braille),
        47 => Some(swash::text::Script::Cypriot),
        48 => Some(swash::text::Script::Limbu),
        49 => Some(swash::text::Script::LinearB),
        50 => Some(swash::text::Script::Osmanya),
        51 => Some(swash::text::Script::Shavian),
        52 => Some(swash::text::Script::TaiLe),
        53 => Some(swash::text::Script::Ugaritic),
        55 => Some(swash::text::Script::Buginese),
        56 => Some(swash::text::Script::Glagolitic),
        57 => Some(swash::text::Script::Kharoshthi),
        58 => Some(swash::text::Script::SylotiNagri),
        59 => Some(swash::text::Script::NewTaiLue),
        60 => Some(swash::text::Script::Tifinagh),
        61 => Some(swash::text::Script::OldPersian),
        62 => Some(swash::text::Script::Balinese),
        63 => Some(swash::text::Script::Batak),
        65 => Some(swash::text::Script::Brahmi),
        66 => Some(swash::text::Script::Cham),
        71 => Some(swash::text::Script::EgyptianHieroglyphs),
        75 => Some(swash::text::Script::PahawhHmong),
        76 => Some(swash::text::Script::OldHungarian),
        78 => Some(swash::text::Script::Javanese),
        79 => Some(swash::text::Script::KayahLi),
        82 => Some(swash::text::Script::Lepcha),
        83 => Some(swash::text::Script::LinearA),
        84 => Some(swash::text::Script::Mandaic),
        86 => Some(swash::text::Script::MeroiticHieroglyphs),
        87 => Some(swash::text::Script::Nko),
        88 => Some(swash::text::Script::OldTurkic),
        89 => Some(swash::text::Script::OldPermic),
        90 => Some(swash::text::Script::PhagsPa),
        91 => Some(swash::text::Script::Phoenician),
        92 => Some(swash::text::Script::Miao),
        99 => Some(swash::text::Script::Vai),
        101 => Some(swash::text::Script::Cuneiform),
        103 => Some(swash::text::Script::Unknown),
        104 => Some(swash::text::Script::Carian),
        106 => Some(swash::text::Script::TaiTham),
        107 => Some(swash::text::Script::Lycian),
        108 => Some(swash::text::Script::Lydian),
        109 => Some(swash::text::Script::OlChiki),
        110 => Some(swash::text::Script::Rejang),
        111 => Some(swash::text::Script::Saurashtra),
        112 => Some(swash::text::Script::SignWriting),
        113 => Some(swash::text::Script::Sundanese),
        115 => Some(swash::text::Script::MeeteiMayek),
        116 => Some(swash::text::Script::ImperialAramaic),
        117 => Some(swash::text::Script::Avestan),
        118 => Some(swash::text::Script::Chakma),
        120 => Some(swash::text::Script::Kaithi),
        121 => Some(swash::text::Script::Manichaean),
        122 => Some(swash::text::Script::InscriptionalPahlavi),
        123 => Some(swash::text::Script::PsalterPahlavi),
        125 => Some(swash::text::Script::InscriptionalParthian),
        126 => Some(swash::text::Script::Samaritan),
        127 => Some(swash::text::Script::TaiViet),
        130 => Some(swash::text::Script::Bamum),
        131 => Some(swash::text::Script::Lisu),
        133 => Some(swash::text::Script::OldSouthArabian),
        134 => Some(swash::text::Script::BassaVah),
        135 => Some(swash::text::Script::Duployan),
        136 => Some(swash::text::Script::Elbasan),
        137 => Some(swash::text::Script::Grantha),
        140 => Some(swash::text::Script::MendeKikakui),
        141 => Some(swash::text::Script::MeroiticCursive),
        142 => Some(swash::text::Script::OldNorthArabian),
        143 => Some(swash::text::Script::Nabataean),
        144 => Some(swash::text::Script::Palmyrene),
        145 => Some(swash::text::Script::Khudawadi),
        146 => Some(swash::text::Script::WarangCiti),
        149 => Some(swash::text::Script::Mro),
        150 => Some(swash::text::Script::Nushu),
        151 => Some(swash::text::Script::Sharada),
        152 => Some(swash::text::Script::SoraSompeng),
        153 => Some(swash::text::Script::Takri),
        154 => Some(swash::text::Script::Tangut),
        156 => Some(swash::text::Script::AnatolianHieroglyphs),
        157 => Some(swash::text::Script::Khojki),
        158 => Some(swash::text::Script::Tirhuta),
        159 => Some(swash::text::Script::CaucasianAlbanian),
        160 => Some(swash::text::Script::Mahajani),
        161 => Some(swash::text::Script::Ahom),
        162 => Some(swash::text::Script::Hatran),
        163 => Some(swash::text::Script::Modi),
        164 => Some(swash::text::Script::Multani),
        165 => Some(swash::text::Script::PauCinHau),
        166 => Some(swash::text::Script::Siddham),
        167 => Some(swash::text::Script::Adlam),
        168 => Some(swash::text::Script::Bhaiksuki),
        169 => Some(swash::text::Script::Marchen),
        170 => Some(swash::text::Script::Newa),
        171 => Some(swash::text::Script::Osage),
        175 => Some(swash::text::Script::MasaramGondi),
        176 => Some(swash::text::Script::Soyombo),
        177 => Some(swash::text::Script::ZanabazarSquare),
        178 => Some(swash::text::Script::Dogra),
        179 => Some(swash::text::Script::GunjalaGondi),
        180 => Some(swash::text::Script::Makasar),
        181 => Some(swash::text::Script::Medefaidrin),
        182 => Some(swash::text::Script::HanifiRohingya),
        183 => Some(swash::text::Script::Sogdian),
        184 => Some(swash::text::Script::OldSogdian),
        185 => Some(swash::text::Script::Elymaic),
        186 => Some(swash::text::Script::NyiakengPuachueHmong),
        187 => Some(swash::text::Script::Nandinagari),
        188 => Some(swash::text::Script::Wancho),
        189 => Some(swash::text::Script::Chorasmian),
        190 => Some(swash::text::Script::DivesAkuru),
        191 => Some(swash::text::Script::KhitanSmallScript),
        192 => Some(swash::text::Script::Yezidi),
        // 193 => Some(swash::text::Script::Cypro),
        // 194 => Some(swash::text::Script::OldUyghur),
        // 195 => Some(swash::text::Script::Tangsa),
        // 196 => Some(swash::text::Script::Toto),
        // 197 => Some(swash::text::Script::Vithkuqi),
        // 198 => Some(swash::text::Script::Kawi),
        // 199 => Some(swash::text::Script::NagMundari),
        // 200 => Some(swash::text::Script::Nastaliq),
        _ => None,
    }
}