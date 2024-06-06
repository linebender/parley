// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Support for working with Unicode scripts.

use core::{convert::TryInto, fmt};

/// Four byte tag representing a Unicode script.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Script(pub [u8; 4]);

impl Script {
    /// Returns a mapping of scripts to sample text.
    pub fn all_samples() -> &'static [(Script, &'static str)] {
        SCRIPT_SAMPLES
    }

    /// Returns a string containing sample characters for this script.
    pub fn sample(self) -> Option<&'static str> {
        let ix = SCRIPT_SAMPLES
            .binary_search_by(|entry| entry.0.cmp(&self))
            .ok()?;
        SCRIPT_SAMPLES.get(ix).map(|entry| entry.1)
    }

    /// Returns the associated [`icu_properties::Script`] value.
    #[cfg(feature = "icu_properties")]
    pub fn icu_script(self) -> Option<icu_properties::Script> {
        let mapper = icu_properties::Script::name_to_enum_mapper();
        let s = core::str::from_utf8(&self.0).ok()?;
        mapper.get_strict(s)
    }

    /// Returns the associated [`unicode_script::Script`] value.
    #[cfg(feature = "unicode_script")]
    pub fn unicode_script(self) -> Option<unicode_script::Script> {
        unicode_script::Script::from_short_name(core::str::from_utf8(&self.0).ok()?)
    }
}

impl fmt::Debug for Script {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", core::str::from_utf8(&self.0).unwrap_or_default())
    }
}

impl fmt::Display for Script {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", core::str::from_utf8(&self.0).unwrap_or_default())
    }
}

impl From<[u8; 4]> for Script {
    fn from(value: [u8; 4]) -> Self {
        Self(value)
    }
}

impl From<&[u8; 4]> for Script {
    fn from(value: &[u8; 4]) -> Self {
        Self(*value)
    }
}

impl From<&str> for Script {
    fn from(value: &str) -> Self {
        Self(value.as_bytes().try_into().unwrap_or_default())
    }
}

#[cfg(feature = "icu_properties")]
impl From<icu_properties::Script> for Script {
    fn from(value: icu_properties::Script) -> Self {
        Self(
            icu_properties::Script::enum_to_short_name_mapper()
                .get(value)
                .map(|name| *name.all_bytes())
                .unwrap_or_default(),
        )
    }
}

#[cfg(feature = "unicode_script")]
impl From<unicode_script::Script> for Script {
    fn from(value: unicode_script::Script) -> Self {
        Self(value.short_name().as_bytes().try_into().unwrap_or_default())
    }
}

pub const SCRIPT_SAMPLES: &[(Script, &str)] = &[
    (Script(*b"Adlm"), "𞤀𞤁𞤂𞤃𞤄𞤅𞤆𞤇𞤈𞤉𞤊𞤋𞤌𞤍𞤎𞤏"),
    (Script(*b"Aghb"), "𐔰𐔱𐔲𐔳𐔴𐔵𐔶𐔷𐔸𐔹𐔺𐔻𐔼𐔽𐔾𐔿"),
    (Script(*b"Ahom"), "𑜀𑜁𑜂𑜃𑜄𑜅𑜆𑜇𑜈𑜉𑜊𑜋𑜌𑜍𑜎𑜏"),
    (Script(*b"Arab"), "\u{600}\u{601}\u{602}\u{603}\u{604}؆؇؈؉؊؋؍؎؏\u{610}\u{611}"),
    (Script(*b"Armi"), "𐡀𐡁𐡂𐡃𐡄𐡅𐡆𐡇𐡈𐡉𐡊𐡋𐡌𐡍𐡎𐡏"),
    (Script(*b"Armn"), "ԱԲԳԴԵԶԷԸԹԺԻԼԽԾԿՀ"),
    (Script(*b"Avst"), "𐬀𐬁𐬂𐬃𐬄𐬅𐬆𐬇𐬈𐬉𐬊𐬋𐬌𐬍𐬎𐬏"),
    (Script(*b"Bali"), "\u{1b00}\u{1b01}\u{1b02}\u{1b03}ᬄᬅᬆᬇᬈᬉᬊᬋᬌᬍᬎᬏ"),
    (Script(*b"Bamu"), "ꚠꚡꚢꚣꚤꚥꚦꚧꚨꚩꚪꚫꚬꚭꚮꚯ"),
    (Script(*b"Bass"), "𖫐𖫑𖫒𖫓𖫔𖫕𖫖𖫗𖫘𖫙𖫚𖫛𖫜𖫝𖫞𖫟"),
    (Script(*b"Batk"), "ᯀᯁᯂᯃᯄᯅᯆᯇᯈᯉᯊᯋᯌᯍᯎᯏ"),
    (Script(*b"Beng"), "ঀ\u{981}ংঃঅআইঈউঊঋঌএঐওঔ"),
    (Script(*b"Bhks"), "𑰀𑰁𑰂𑰃𑰄𑰅𑰆𑰇𑰈𑰊𑰋𑰌𑰍𑰎𑰏𑰐"),
    (Script(*b"Bopo"), "˪˫ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒ"),
    (Script(*b"Brah"), "𑀀\u{11001}𑀂𑀃𑀄𑀅𑀆𑀇𑀈𑀉𑀊𑀋𑀌𑀍𑀎𑀏"),
    (Script(*b"Brai"), "⠀⠁⠂⠃⠄⠅⠆⠇⠈⠉⠊⠋⠌⠍⠎⠏"),
    (Script(*b"Bugi"), "ᨀᨁᨂᨃᨄᨅᨆᨇᨈᨉᨊᨋᨌᨍᨎᨏ"),
    (Script(*b"Buhd"), "ᝀᝁᝂᝃᝄᝅᝆᝇᝈᝉᝊᝋᝌᝍᝎᝏ"),
    (Script(*b"Cakm"), "\u{11100}\u{11101}\u{11102}𑄃𑄄𑄅𑄆𑄇𑄈𑄉𑄊𑄋𑄌𑄍𑄎𑄏"),
    (Script(*b"Cans"), "᐀ᐁᐂᐃᐄᐅᐆᐇᐈᐉᐊᐋᐌᐍᐎᐏ"),
    (Script(*b"Cari"), "𐊠𐊡𐊢𐊣𐊤𐊥𐊦𐊧𐊨𐊩𐊪𐊫𐊬𐊭𐊮𐊯"),
    (Script(*b"Cham"), "ꨀꨁꨂꨃꨄꨅꨆꨇꨈꨉꨊꨋꨌꨍꨎꨏ"),
    (Script(*b"Cher"), "ᎠᎡᎢᎣᎤᎥᎦᎧᎨᎩᎪᎫᎬᎭᎮᎯ"),
    (Script(*b"Chrs"), "𐾰𐾱𐾲𐾳𐾴𐾵𐾶𐾷𐾸𐾹𐾺𐾻𐾼𐾽𐾾𐾿"),
    (Script(*b"Copt"), "ϢϣϤϥϦϧϨϩϪϫϬϭϮϯⲀⲁ"),
    (Script(*b"Cpmn"), "𒾐𒾑𒾒𒾓𒾔𒾕𒾖𒾗𒾘𒾙𒾚𒾛𒾜𒾝𒾞𒾟"),
    (Script(*b"Cprt"), "𐠀𐠁𐠂𐠃𐠄𐠅𐠈𐠊𐠋𐠌𐠍𐠎𐠏𐠐𐠑𐠒"),
    (Script(*b"Cyrl"), "ЀЁЂЃЄЅІЇЈЉЊЋЌЍЎЏ"),
    (Script(*b"Deva"), "\u{900}\u{901}\u{902}ःऄअआइईउऊऋऌऍऎए"),
    (Script(*b"Diak"), "𑤀𑤁𑤂𑤃𑤄𑤅𑤆𑤉𑤌𑤍𑤎𑤏𑤐𑤑𑤒𑤓"),
    (Script(*b"Dogr"), "𑠀𑠁𑠂𑠃𑠄𑠅𑠆𑠇𑠈𑠉𑠊𑠋𑠌𑠍𑠎𑠏"),
    (Script(*b"Dsrt"), "𐐀𐐁𐐂𐐃𐐄𐐅𐐆𐐇𐐈𐐉𐐊𐐋𐐌𐐍𐐎𐐏"),
    (Script(*b"Dupl"), "𛰀𛰁𛰂𛰃𛰄𛰅𛰆𛰇𛰈𛰉𛰊𛰋𛰌𛰍𛰎𛰏"),
    (Script(*b"Egyp"), "𓀀𓀁𓀂𓀃𓀄𓀅𓀆𓀇𓀈𓀉𓀊𓀋𓀌𓀍𓀎𓀏"),
    (Script(*b"Elba"), "𐔀𐔁𐔂𐔃𐔄𐔅𐔆𐔇𐔈𐔉𐔊𐔋𐔌𐔍𐔎𐔏"),
    (Script(*b"Elym"), "𐿠𐿡𐿢𐿣𐿤𐿥𐿦𐿧𐿨𐿩𐿪𐿫𐿬𐿭𐿮𐿯"),
    (Script(*b"Ethi"), "ሀሁሂሃሄህሆሇለሉሊላሌልሎሏ"),
    (Script(*b"Geor"), "ႠႡႢႣႤႥႦႧႨႩႪႫႬႭႮႯ"),
    (Script(*b"Glag"), "ⰀⰁⰂⰃⰄⰅⰆⰇⰈⰉⰊⰋⰌⰍⰎⰏ"),
    (Script(*b"Gong"), "𑵠𑵡𑵢𑵣𑵤𑵥𑵧𑵨𑵪𑵫𑵬𑵭𑵮𑵯𑵰𑵱"),
    (Script(*b"Gonm"), "𑴀𑴁𑴂𑴃𑴄𑴅𑴆𑴈𑴉𑴋𑴌𑴍𑴎𑴏𑴐𑴑"),
    (Script(*b"Goth"), "𐌰𐌱𐌲𐌳𐌴𐌵𐌶𐌷𐌸𐌹𐌺𐌻𐌼𐌽𐌾𐌿"),
    (Script(*b"Gran"), "\u{11300}\u{11301}𑌂𑌃𑌅𑌆𑌇𑌈𑌉𑌊𑌋𑌌𑌏𑌐𑌓𑌔"),
    (Script(*b"Grek"), "ͰͱͲͳ͵ͶͷͺͻͼͽͿ΄ΆΈΉ"),
    (Script(*b"Gujr"), "\u{a81}\u{a82}ઃઅઆઇઈઉઊઋઌઍએઐઑઓ"),
    (Script(*b"Guru"), "\u{a01}\u{a02}ਃਅਆਇਈਉਊਏਐਓਔਕਖਗ"),
    (Script(*b"Hang"), "가"),
    (Script(*b"Hani"), "今"),
    (Script(*b"Hano"), "ᜠᜡᜢᜣᜤᜥᜦᜧᜨᜩᜪᜫᜬᜭᜮᜯ"),
    (Script(*b"Hatr"), "𐣠𐣡𐣢𐣣𐣤𐣥𐣦𐣧𐣨𐣩𐣪𐣫𐣬𐣭𐣮𐣯"),
    (Script(*b"Hebr"), "\u{591}\u{592}\u{593}\u{594}\u{595}\u{596}\u{597}\u{598}\u{599}\u{59a}\u{59b}\u{59c}\u{59d}\u{59e}\u{59f}\u{5a0}"),
    (Script(*b"Hira"), "ぁあぃいぅうぇえぉおかがきぎくぐ"),
    (Script(*b"Hluw"), "𔐀𔐁𔐂𔐃𔐄𔐅𔐆𔐇𔐈𔐉𔐊𔐋𔐌𔐍𔐎𔐏"),
    (Script(*b"Hmng"), "𖬀𖬁𖬂𖬃𖬄𖬅𖬆𖬇𖬈𖬉𖬊𖬋𖬌𖬍𖬎𖬏"),
    (Script(*b"Hmnp"), "𞄀𞄁𞄂𞄃𞄄𞄅𞄆𞄇𞄈𞄉𞄊𞄋𞄌𞄍𞄎𞄏"),
    (Script(*b"Hung"), "𐲀𐲁𐲂𐲃𐲄𐲅𐲆𐲇𐲈𐲉𐲊𐲋𐲌𐲍𐲎𐲏"),
    (Script(*b"Ital"), "𐌀𐌁𐌂𐌃𐌄𐌅𐌆𐌇𐌈𐌉𐌊𐌋𐌌𐌍𐌎𐌏"),
    (Script(*b"Java"), "\u{a980}\u{a981}\u{a982}ꦃꦄꦅꦆꦇꦈꦉꦊꦋꦌꦍꦎꦏ"),
    (Script(*b"Kali"), "꤀꤁꤂꤃꤄꤅꤆꤇꤈꤉ꤊꤋꤌꤍꤎꤏ"),
    (Script(*b"Kana"), "ァアィイゥウェエォオカガキギクグ"),
    (Script(*b"Kawi"), "\u{11f00}\u{11f01}𑼂𑼃𑼄𑼅𑼆𑼇𑼈𑼉𑼊𑼋𑼌𑼍𑼎𑼏"),
    (Script(*b"Khar"), "𐨀\u{10a01}\u{10a02}\u{10a03}\u{10a05}\u{10a06}\u{10a0c}\u{10a0d}\u{10a0e}\u{10a0f}𐨐𐨑𐨒𐨓𐨕𐨖"),
    (Script(*b"Khmr"), "កខគឃងចឆជឈញដឋឌឍណត"),
    (Script(*b"Khoj"), "𑈀𑈁𑈂𑈃𑈄𑈅𑈆𑈇𑈈𑈉𑈊𑈋𑈌𑈍𑈎𑈏"),
    (Script(*b"Kits"), "\u{16fe4}𘬀𘬁𘬂𘬃𘬄𘬅𘬆𘬇𘬈𘬉𘬊𘬋𘬌𘬍𘬎"),
    (Script(*b"Knda"), "ಀ\u{c81}ಂಃ಄ಅಆಇಈಉಊಋಌಎಏಐ"),
    (Script(*b"Kthi"), "\u{11080}\u{11081}𑂂𑂃𑂄𑂅𑂆𑂇𑂈𑂉𑂊𑂋𑂌𑂍𑂎𑂏"),
    (Script(*b"Lana"), "ᨠᨡᨢᨣᨤᨥᨦᨧᨨᨩᨪᨫᨬᨭᨮᨯ"),
    (Script(*b"Laoo"), "ກຂຄຆງຈຉຊຌຍຎຏຐຑຒຓ"),
    (Script(*b"Latn"), "abcdefgABCDEFG"),
    (Script(*b"Lepc"), "ᰀᰁᰂᰃᰄᰅᰆᰇᰈᰉᰊᰋᰌᰍᰎᰏ"),
    (Script(*b"Limb"), "ᤀᤁᤂᤃᤄᤅᤆᤇᤈᤉᤊᤋᤌᤍᤎᤏ"),
    (Script(*b"Lina"), "𐘀𐘁𐘂𐘃𐘄𐘅𐘆𐘇𐘈𐘉𐘊𐘋𐘌𐘍𐘎𐘏"),
    (Script(*b"Linb"), "𐀀𐀁𐀂𐀃𐀄𐀅𐀆𐀇𐀈𐀉𐀊𐀋𐀍𐀎𐀏𐀐"),
    (Script(*b"Lisu"), "ꓐꓑꓒꓓꓔꓕꓖꓗꓘꓙꓚꓛꓜꓝꓞꓟ"),
    (Script(*b"Lyci"), "𐊀𐊁𐊂𐊃𐊄𐊅𐊆𐊇𐊈𐊉𐊊𐊋𐊌𐊍𐊎𐊏"),
    (Script(*b"Lydi"), "𐤠𐤡𐤢𐤣𐤤𐤥𐤦𐤧𐤨𐤩𐤪𐤫𐤬𐤭𐤮𐤯"),
    (Script(*b"Mahj"), "𑅐𑅑𑅒𑅓𑅔𑅕𑅖𑅗𑅘𑅙𑅚𑅛𑅜𑅝𑅞𑅟"),
    (Script(*b"Maka"), "𑻠𑻡𑻢𑻣𑻤𑻥𑻦𑻧𑻨𑻩𑻪𑻫𑻬𑻭𑻮𑻯"),
    (Script(*b"Mand"), "ࡀࡁࡂࡃࡄࡅࡆࡇࡈࡉࡊࡋࡌࡍࡎࡏ"),
    (Script(*b"Mani"), "𐫀𐫁𐫂𐫃𐫄𐫅𐫆𐫇𐫈𐫉𐫊𐫋𐫌𐫍𐫎𐫏"),
    (Script(*b"Marc"), "𑱰𑱱𑱲𑱳𑱴𑱵𑱶𑱷𑱸𑱹𑱺𑱻𑱼𑱽𑱾𑱿"),
    (Script(*b"Medf"), "𖹀𖹁𖹂𖹃𖹄𖹅𖹆𖹇𖹈𖹉𖹊𖹋𖹌𖹍𖹎𖹏"),
    (Script(*b"Mend"), "𞠀𞠁𞠂𞠃𞠄𞠅𞠆𞠇𞠈𞠉𞠊𞠋𞠌𞠍𞠎𞠏"),
    (Script(*b"Merc"), "𐦠𐦡𐦢𐦣𐦤𐦥𐦦𐦧𐦨𐦩𐦪𐦫𐦬𐦭𐦮𐦯"),
    (Script(*b"Mero"), "𐦀𐦁𐦂𐦃𐦄𐦅𐦆𐦇𐦈𐦉𐦊𐦋𐦌𐦍𐦎𐦏"),
    (Script(*b"Mlym"), "\u{d00}\u{d01}ംഃഄഅആഇഈഉഊഋഌഎഏഐ"),
    (Script(*b"Modi"), "𑘀𑘁𑘂𑘃𑘄𑘅𑘆𑘇𑘈𑘉𑘊𑘋𑘌𑘍𑘎𑘏"),
    (Script(*b"Mong"), "᠀᠁᠄᠆᠇᠈᠉᠊\u{180b}\u{180c}\u{180d}\u{180e}\u{180f}᠐᠑᠒"),
    (Script(*b"Mroo"), "𖩀𖩁𖩂𖩃𖩄𖩅𖩆𖩇𖩈𖩉𖩊𖩋𖩌𖩍𖩎𖩏"),
    (Script(*b"Mtei"), "ꫠꫡꫢꫣꫤꫥꫦꫧꫨꫩꫪꫫ\u{aaec}\u{aaed}ꫮꫯ"),
    (Script(*b"Mult"), "𑊀𑊁𑊂𑊃𑊄𑊅𑊆𑊈𑊊𑊋𑊌𑊍𑊏𑊐𑊑𑊒"),
    (Script(*b"Mymr"), "ကခဂဃငစဆဇဈဉညဋဌဍဎဏ"),
    (Script(*b"Nagm"), "𞓐𞓑𞓒𞓓𞓔𞓕𞓖𞓗𞓘𞓙𞓚𞓛𞓜𞓝𞓞𞓟"),
    (Script(*b"Nand"), "𑦠𑦡𑦢𑦣𑦤𑦥𑦦𑦧𑦪𑦫𑦬𑦭𑦮𑦯𑦰𑦱"),
    (Script(*b"Narb"), "𐪀𐪁𐪂𐪃𐪄𐪅𐪆𐪇𐪈𐪉𐪊𐪋𐪌𐪍𐪎𐪏"),
    (Script(*b"Nbat"), "𐢀𐢁𐢂𐢃𐢄𐢅𐢆𐢇𐢈𐢉𐢊𐢋𐢌𐢍𐢎𐢏"),
    (Script(*b"Newa"), "𑐀𑐁𑐂𑐃𑐄𑐅𑐆𑐇𑐈𑐉𑐊𑐋𑐌𑐍𑐎𑐏"),
    (Script(*b"Nkoo"), "߀߁߂߃߄߅߆߇߈߉ߊߋߌߍߎߏ"),
    (Script(*b"Nshu"), "𖿡𛅰𛅱𛅲𛅳𛅴𛅵𛅶𛅷𛅸𛅹𛅺𛅻𛅼𛅽𛅾"),
    (Script(*b"Ogam"), "\u{1680}ᚁᚂᚃᚄᚅᚆᚇᚈᚉᚊᚋᚌᚍᚎᚏ"),
    (Script(*b"Olck"), "᱐᱑᱒᱓᱔᱕᱖᱗᱘᱙ᱚᱛᱜᱝᱞᱟ"),
    (Script(*b"Orkh"), "𐰀𐰁𐰂𐰃𐰄𐰅𐰆𐰇𐰈𐰉𐰊𐰋𐰌𐰍𐰎𐰏"),
    (Script(*b"Orya"), "\u{b01}ଂଃଅଆଇଈଉଊଋଌଏଐଓଔକ"),
    (Script(*b"Osge"), "𐒰𐒱𐒲𐒳𐒴𐒵𐒶𐒷𐒸𐒹𐒺𐒻𐒼𐒽𐒾𐒿"),
    (Script(*b"Osma"), "𐒀𐒁𐒂𐒃𐒄𐒅𐒆𐒇𐒈𐒉𐒊𐒋𐒌𐒍𐒎𐒏"),
    (Script(*b"Ougr"), "𐽰𐽱𐽲𐽳𐽴𐽵𐽶𐽷𐽸𐽹𐽺𐽻𐽼𐽽𐽾𐽿"),
    (Script(*b"Palm"), "𐡠𐡡𐡢𐡣𐡤𐡥𐡦𐡧𐡨𐡩𐡪𐡫𐡬𐡭𐡮𐡯"),
    (Script(*b"Pauc"), "𑫀𑫁𑫂𑫃𑫄𑫅𑫆𑫇𑫈𑫉𑫊𑫋𑫌𑫍𑫎𑫏"),
    (Script(*b"Perm"), "𐍐𐍑𐍒𐍓𐍔𐍕𐍖𐍗𐍘𐍙𐍚𐍛𐍜𐍝𐍞𐍟"),
    (Script(*b"Phag"), "ꡀꡁꡂꡃꡄꡅꡆꡇꡈꡉꡊꡋꡌꡍꡎꡏ"),
    (Script(*b"Phli"), "𐭠𐭡𐭢𐭣𐭤𐭥𐭦𐭧𐭨𐭩𐭪𐭫𐭬𐭭𐭮𐭯"),
    (Script(*b"Phlp"), "𐮀𐮁𐮂𐮃𐮄𐮅𐮆𐮇𐮈𐮉𐮊𐮋𐮌𐮍𐮎𐮏"),
    (Script(*b"Phnx"), "𐤀𐤁𐤂𐤃𐤄𐤅𐤆𐤇𐤈𐤉𐤊𐤋𐤌𐤍𐤎𐤏"),
    (Script(*b"Plrd"), "𖼀𖼁𖼂𖼃𖼄𖼅𖼆𖼇𖼈𖼉𖼊𖼋𖼌𖼍𖼎𖼏"),
    (Script(*b"Prti"), "𐭀𐭁𐭂𐭃𐭄𐭅𐭆𐭇𐭈𐭉𐭊𐭋𐭌𐭍𐭎𐭏"),
    (Script(*b"Rjng"), "ꤰꤱꤲꤳꤴꤵꤶꤷꤸꤹꤺꤻꤼꤽꤾꤿ"),
    (Script(*b"Rohg"), "𐴀𐴁𐴂𐴃𐴄𐴅𐴆𐴇𐴈𐴉𐴊𐴋𐴌𐴍𐴎𐴏"),
    (Script(*b"Runr"), "ᚠᚡᚢᚣᚤᚥᚦᚧᚨᚩᚪᚫᚬᚭᚮᚯ"),
    (Script(*b"Samr"), "ࠀࠁࠂࠃࠄࠅࠆࠇࠈࠉࠊࠋࠌࠍࠎࠏ"),
    (Script(*b"Sarb"), "𐩠𐩡𐩢𐩣𐩤𐩥𐩦𐩧𐩨𐩩𐩪𐩫𐩬𐩭𐩮𐩯"),
    (Script(*b"Saur"), "ꢀꢁꢂꢃꢄꢅꢆꢇꢈꢉꢊꢋꢌꢍꢎꢏ"),
    (Script(*b"Sgnw"), "𝠀𝠁𝠂𝠃𝠄𝠅𝠆𝠇𝠈𝠉𝠊𝠋𝠌𝠍𝠎𝠏"),
    (Script(*b"Shaw"), "𐑐𐑑𐑒𐑓𐑔𐑕𐑖𐑗𐑘𐑙𐑚𐑛𐑜𐑝𐑞𐑟"),
    (Script(*b"Shrd"), "\u{11180}\u{11181}𑆂𑆃𑆄𑆅𑆆𑆇𑆈𑆉𑆊𑆋𑆌𑆍𑆎𑆏"),
    (Script(*b"Sidd"), "𑖀𑖁𑖂𑖃𑖄𑖅𑖆𑖇𑖈𑖉𑖊𑖋𑖌𑖍𑖎𑖏"),
    (Script(*b"Sind"), "𑊰𑊱𑊲𑊳𑊴𑊵𑊶𑊷𑊸𑊹𑊺𑊻𑊼𑊽𑊾𑊿"),
    (Script(*b"Sinh"), "\u{d81}ංඃඅආඇඈඉඊඋඌඍඎඏඐඑ"),
    (Script(*b"Sogd"), "𐼰𐼱𐼲𐼳𐼴𐼵𐼶𐼷𐼸𐼹𐼺𐼻𐼼𐼽𐼾𐼿"),
    (Script(*b"Sogo"), "𐼀𐼁𐼂𐼃𐼄𐼅𐼆𐼇𐼈𐼉𐼊𐼋𐼌𐼍𐼎𐼏"),
    (Script(*b"Sora"), "𑃐𑃑𑃒𑃓𑃔𑃕𑃖𑃗𑃘𑃙𑃚𑃛𑃜𑃝𑃞𑃟"),
    (Script(*b"Soyo"), "𑩐\u{11a51}\u{11a52}\u{11a53}\u{11a54}\u{11a55}\u{11a56}𑩗𑩘\u{11a59}\u{11a5a}\u{11a5b}𑩜𑩝𑩞𑩟"),
    (Script(*b"Sund"), "\u{1b80}\u{1b81}ᮂᮃᮄᮅᮆᮇᮈᮉᮊᮋᮌᮍᮎᮏ"),
    (Script(*b"Sylo"), "ꠀꠁ\u{a802}ꠃꠄꠅ\u{a806}ꠇꠈꠉꠊ\u{a80b}ꠌꠍꠎꠏ"),
    (Script(*b"Syrc"), "܀܁܂܃܄܅܆܇܈܉܊܋܌܍\u{70f}ܐ"),
    (Script(*b"Tagb"), "ᝠᝡᝢᝣᝤᝥᝦᝧᝨᝩᝪᝫᝬᝮᝯᝰ"),
    (Script(*b"Takr"), "𑚀𑚁𑚂𑚃𑚄𑚅𑚆𑚇𑚈𑚉𑚊𑚋𑚌𑚍𑚎𑚏"),
    (Script(*b"Tale"), "ᥐᥑᥒᥓᥔᥕᥖᥗᥘᥙᥚᥛᥜᥝᥞᥟ"),
    (Script(*b"Talu"), "ᦀᦁᦂᦃᦄᦅᦆᦇᦈᦉᦊᦋᦌᦍᦎᦏ"),
    (Script(*b"Taml"), "\u{b82}ஃஅஆஇஈஉஊஎஏஐஒஓஔகங"),
    (Script(*b"Tang"), "𖿠𗀀𗀁𗀂𗀃𗀄𗀅𗀆𗀇𗀈𗀉𗀊𗀋𗀌𗀍𗀎"),
    (Script(*b"Tavt"), "ꪀꪁꪂꪃꪄꪅꪆꪇꪈꪉꪊꪋꪌꪍꪎꪏ"),
    (Script(*b"Telu"), "\u{c00}ఁంః\u{c04}అఆఇఈఉఊఋఌఎఏఐ"),
    (Script(*b"Tfng"), "ⴰⴱⴲⴳⴴⴵⴶⴷⴸⴹⴺⴻⴼⴽⴾⴿ"),
    (Script(*b"Tglg"), "ᜀᜁᜂᜃᜄᜅᜆᜇᜈᜉᜊᜋᜌᜍᜎᜏ"),
    (Script(*b"Thaa"), "ހށނރބޅކއވމފދތލގޏ"),
    (Script(*b"Thai"), "กขฃคฅฆงจฉชซฌญฎฏฐ"),
    (Script(*b"Tibt"), "ༀ༁༂༃༄༅༆༇༈༉༊་༌།༎༏"),
    (Script(*b"Tirh"), "𑒀𑒁𑒂𑒃𑒄𑒅𑒆𑒇𑒈𑒉𑒊𑒋𑒌𑒍𑒎𑒏"),
    (Script(*b"Tnsa"), "𖩰𖩱𖩲𖩳𖩴𖩵𖩶𖩷𖩸𖩹𖩺𖩻𖩼𖩽𖩾𖩿"),
    (Script(*b"Toto"), "𞊐𞊑𞊒𞊓𞊔𞊕𞊖𞊗𞊘𞊙𞊚𞊛𞊜𞊝𞊞𞊟"),
    (Script(*b"Ugar"), "𐎀𐎁𐎂𐎃𐎄𐎅𐎆𐎇𐎈𐎉𐎊𐎋𐎌𐎍𐎎𐎏"),
    (Script(*b"Vaii"), "ꔀꔁꔂꔃꔄꔅꔆꔇꔈꔉꔊꔋꔌꔍꔎꔏ"),
    (Script(*b"Vith"), "𐕰𐕱𐕲𐕳𐕴𐕵𐕶𐕷𐕸𐕹𐕺𐕼𐕽𐕾𐕿𐖀"),
    (Script(*b"Wara"), "𑢠𑢡𑢢𑢣𑢤𑢥𑢦𑢧𑢨𑢩𑢪𑢫𑢬𑢭𑢮𑢯"),
    (Script(*b"Wcho"), "𞋀𞋁𞋂𞋃𞋄𞋅𞋆𞋇𞋈𞋉𞋊𞋋𞋌𞋍𞋎𞋏"),
    (Script(*b"Xpeo"), "𐎠𐎡𐎢𐎣𐎤𐎥𐎦𐎧𐎨𐎩𐎪𐎫𐎬𐎭𐎮𐎯"),
    (Script(*b"Xsux"), "𒀀𒀁𒀂𒀃𒀄𒀅𒀆𒀇𒀈𒀉𒀊𒀋𒀌𒀍𒀎𒀏"),
    (Script(*b"Yezi"), "𐺀𐺁𐺂𐺃𐺄𐺅𐺆𐺇𐺈𐺉𐺊𐺋𐺌𐺍𐺎𐺏"),
    (Script(*b"Yiii"), "ꀀꀁꀂꀃꀄꀅꀆꀇꀈꀉꀊꀋꀌꀍꀎꀏ"),
    (Script(*b"Zanb"), "𑨀\u{11a01}\u{11a02}\u{11a03}\u{11a04}\u{11a05}\u{11a06}\u{11a07}\u{11a08}\u{11a09}\u{11a0a}𑨋𑨌𑨍𑨎𑨏"),    
];
