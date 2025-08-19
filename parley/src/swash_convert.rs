// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub(crate) fn script_to_fontique(script: swash::text::Script) -> fontique::Script {
    fontique::Script(*FONTIQUE_SCRIPT_TAGS.get(script as usize).unwrap_or(b"Zzzz"))
}

pub(crate) fn script_to_harfrust(script: swash::text::Script) -> harfrust::Script {
    *HARFRUST_SCRIPT_TAGS
        .get(script as usize)
        .unwrap_or(&harfrust::script::UNKNOWN)
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

const HARFRUST_SCRIPT_TAGS: [harfrust::Script; 157] = [
    harfrust::script::ADLAM,
    harfrust::script::CAUCASIAN_ALBANIAN,
    harfrust::script::AHOM,
    harfrust::script::ARABIC,
    harfrust::script::IMPERIAL_ARAMAIC,
    harfrust::script::ARMENIAN,
    harfrust::script::AVESTAN,
    harfrust::script::BALINESE,
    harfrust::script::BAMUM,
    harfrust::script::BASSA_VAH,
    harfrust::script::BATAK,
    harfrust::script::BENGALI,
    harfrust::script::BHAIKSUKI,
    harfrust::script::BOPOMOFO,
    harfrust::script::BRAHMI,
    harfrust::script::BRAILLE,
    harfrust::script::BUGINESE,
    harfrust::script::BUHID,
    harfrust::script::CHAKMA,
    harfrust::script::CANADIAN_SYLLABICS,
    harfrust::script::CARIAN,
    harfrust::script::CHAM,
    harfrust::script::CHEROKEE,
    harfrust::script::CHORASMIAN,
    harfrust::script::COPTIC,
    harfrust::script::CYPRIOT,
    harfrust::script::CYRILLIC,
    harfrust::script::DEVANAGARI,
    harfrust::script::DIVES_AKURU,
    harfrust::script::DOGRA,
    harfrust::script::DESERET,
    harfrust::script::DUPLOYAN,
    harfrust::script::EGYPTIAN_HIEROGLYPHS,
    harfrust::script::ELBASAN,
    harfrust::script::ELYMAIC,
    harfrust::script::ETHIOPIC,
    harfrust::script::GEORGIAN,
    harfrust::script::GLAGOLITIC,
    harfrust::script::GUNJALA_GONDI,
    harfrust::script::MASARAM_GONDI,
    harfrust::script::GOTHIC,
    harfrust::script::GRANTHA,
    harfrust::script::GREEK,
    harfrust::script::GUJARATI,
    harfrust::script::GURMUKHI,
    harfrust::script::HANGUL,
    harfrust::script::HAN,
    harfrust::script::HANUNOO,
    harfrust::script::HATRAN,
    harfrust::script::HEBREW,
    harfrust::script::HIRAGANA,
    harfrust::script::ANATOLIAN_HIEROGLYPHS,
    harfrust::script::PAHAWH_HMONG,
    harfrust::script::NYIAKENG_PUACHUE_HMONG,
    harfrust::script::OLD_HUNGARIAN,
    harfrust::script::OLD_ITALIC,
    harfrust::script::JAVANESE,
    harfrust::script::KAYAH_LI,
    harfrust::script::KATAKANA,
    harfrust::script::KHAROSHTHI,
    harfrust::script::KHMER,
    harfrust::script::KHOJKI,
    harfrust::script::KHITAN_SMALL_SCRIPT,
    harfrust::script::KANNADA,
    harfrust::script::KAITHI,
    harfrust::script::TAI_THAM,
    harfrust::script::LAO,
    harfrust::script::LATIN,
    harfrust::script::LEPCHA,
    harfrust::script::LIMBU,
    harfrust::script::LINEAR_A,
    harfrust::script::LINEAR_B,
    harfrust::script::LISU,
    harfrust::script::LYCIAN,
    harfrust::script::LYDIAN,
    harfrust::script::MAHAJANI,
    harfrust::script::MAKASAR,
    harfrust::script::MANDAIC,
    harfrust::script::MANICHAEAN,
    harfrust::script::MARCHEN,
    harfrust::script::MEDEFAIDRIN,
    harfrust::script::MENDE_KIKAKUI,
    harfrust::script::MEROITIC_CURSIVE,
    harfrust::script::MEROITIC_HIEROGLYPHS,
    harfrust::script::MALAYALAM,
    harfrust::script::MODI,
    harfrust::script::MONGOLIAN,
    harfrust::script::MRO,
    harfrust::script::MEETEI_MAYEK,
    harfrust::script::MULTANI,
    harfrust::script::MYANMAR,
    harfrust::script::NANDINAGARI,
    harfrust::script::OLD_NORTH_ARABIAN,
    harfrust::script::NABATAEAN,
    harfrust::script::NEWA,
    harfrust::script::NKO,
    harfrust::script::NUSHU,
    harfrust::script::OGHAM,
    harfrust::script::OL_CHIKI,
    harfrust::script::OLD_TURKIC,
    harfrust::script::ORIYA,
    harfrust::script::OSAGE,
    harfrust::script::OSMANYA,
    harfrust::script::PALMYRENE,
    harfrust::script::PAU_CIN_HAU,
    harfrust::script::OLD_PERMIC,
    harfrust::script::PHAGS_PA,
    harfrust::script::INSCRIPTIONAL_PAHLAVI,
    harfrust::script::PSALTER_PAHLAVI,
    harfrust::script::PHOENICIAN,
    harfrust::script::MIAO,
    harfrust::script::INSCRIPTIONAL_PARTHIAN,
    harfrust::script::REJANG,
    harfrust::script::HANIFI_ROHINGYA,
    harfrust::script::RUNIC,
    harfrust::script::SAMARITAN,
    harfrust::script::OLD_SOUTH_ARABIAN,
    harfrust::script::SAURASHTRA,
    harfrust::script::SIGNWRITING,
    harfrust::script::SHAVIAN,
    harfrust::script::SHARADA,
    harfrust::script::SIDDHAM,
    harfrust::script::KHUDAWADI,
    harfrust::script::SINHALA,
    harfrust::script::SOGDIAN,
    harfrust::script::OLD_SOGDIAN,
    harfrust::script::SORA_SOMPENG,
    harfrust::script::SOYOMBO,
    harfrust::script::SUNDANESE,
    harfrust::script::SYLOTI_NAGRI,
    harfrust::script::SYRIAC,
    harfrust::script::TAGBANWA,
    harfrust::script::TAKRI,
    harfrust::script::TAI_LE,
    harfrust::script::NEW_TAI_LUE,
    harfrust::script::TAMIL,
    harfrust::script::TANGUT,
    harfrust::script::TAI_VIET,
    harfrust::script::TELUGU,
    harfrust::script::TIFINAGH,
    harfrust::script::TAGALOG,
    harfrust::script::THAANA,
    harfrust::script::THAI,
    harfrust::script::TIBETAN,
    harfrust::script::TIRHUTA,
    harfrust::script::UGARITIC,
    harfrust::script::VAI,
    harfrust::script::WARANG_CITI,
    harfrust::script::WANCHO,
    harfrust::script::OLD_PERSIAN,
    harfrust::script::CUNEIFORM,
    harfrust::script::YEZIDI,
    harfrust::script::YI,
    harfrust::script::ZANABAZAR_SQUARE,
    harfrust::script::INHERITED,
    harfrust::script::COMMON,
    harfrust::script::UNKNOWN,
];
