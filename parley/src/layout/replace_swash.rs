use crate::icu_working;

/// The maximum number of characters in a single cluster.
pub const MAX_CLUSTER_SIZE: usize = 32;

pub(crate) struct CharCluster {
    pub info: ClusterInfo,
    pub chars: Vec<Char>, // [Char; MAX_CLUSTER_SIZE], TODO(conor)
    pub style_index: u16,
    len: u8,
    map_len: u8,
    start: u32,
    end: u32,
    force_normalize: bool,
    comp: Form,
    decomp: Form,
    form: FormKind,
    best_ratio: f32,
}

pub(crate) struct ClusterInfo {
    // Not used strictly in Parley:
    // - is_broken()
    // - emoji()
    // - is_boundary()
    // - boundary() - fully replaced by Parley's internal ClusterInfo type
    // - set_space_from_char()
    // - set_broken
    // - set_emoji
    // - set_space
    // - merge_boundary

    // Used in Parley:
    // - is_emoji() (emoji font selection per-cluster, editing
    // - is_whitespace() - line breaking algorithm (greedy.rs)
    // - whitespace() - data.rs, cluster.rs, alignment.rs (carries with it Swash's Whitespace type)
        // this isn't using Swash's implementation after all?
    pub is_emoji: bool,
    //whitespace: Whitespace,
}

impl ClusterInfo {
    /*fn is_whitespace(&self) -> bool {
        !matches!(self.whitespace, Whitespace::None)
    }*/
}

#[derive(Copy, Clone)]
pub(crate) struct Char {
    /// The character.
    pub ch: char,
    /// Offset of the character in code units.
    pub offset: u32,
    /// Shaping class of the character.
    //pub shape_class: ShapeClass,
    pub is_control_character: bool,
    /// True if the character should be considered when mapping glyphs.
    pub contributes_to_shaping: bool,
    /// Nominal glyph identifier.
    pub glyph_id: GlyphId,
    /// Arbitrary user data.
    pub data: UserData,

    // Only used by Swash shaping:
    // Joining type of the character.
    //pub joining_type: JoiningType,
    // True if the character is ignorable.
    //pub ignorable: bool,
}

/*pub(crate) struct Token {
    /// The character.
    pub ch: char,
    /// Offset of the character in code units.
    pub offset: u32,
    /// Length of the character in code units.
    pub len: u8,
    /// Character information.
    pub info: icu_working::CharInfo,
    /// Arbitrary user data.
    pub data: UserData,
}*/

pub type GlyphId = u16;

pub type UserData = u32;

/// Whitespace content of a cluster.
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Whitespace {
    /// Not a space.
    None = 0,
    /// Standard space.
    Space = 1,
    /// Non-breaking space (U+00A0).
    NoBreakSpace = 2,
    /// Horizontal tab.
    Tab = 3,
    /// Newline (CR, LF, or CRLF).
    Newline = 4,
    /// Other space.
    Other = 5,
}

impl Whitespace {
    /// Returns true for space or no break space.
    pub fn is_space_or_nbsp(self) -> bool {
        matches!(self, Self::Space | Self::NoBreakSpace)
    }

    /*#[inline]
    fn from_raw(bits: u16) -> Self {
        match bits & 0b111 {
            0 => Self::None,
            1 => Self::Space,
            2 => Self::NoBreakSpace,
            3 => Self::Tab,
            4 => Self::Newline,
            5 => Self::Other,
            _ => Self::None,
        }
    }*/
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
pub enum JoiningType {
    U = 0,
    L = 1,
    R = 2,
    D = 3,
    Alaph = 4,
    DalathRish = 5,
    T = 6,
}

/// Iterative status of mapping a character cluster to nominal glyph identifiers.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Status {
    /// Mapping should be skipped.
    Discard,
    /// The best mapping so far.
    Keep,
    /// Complete mapping.
    Complete,
}

/*#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ShapeClass {
    /// Reph form.
    Reph,
    /// Pre-base form.
    Pref,
    /// Myanmar three character prefix.
    Kinzi,
    /// Base character.
    Base,
    /// Mark character.
    Mark,
    /// Halant modifier.
    Halant,
    /// Medial consonant Ra.
    MedialRa,
    /// Pre-base vowel modifier.
    VMPre,
    /// Pre-base dependent vowel.
    VPre,
    /// Below base dependent vowel.
    VBlw,
    /// Anusvara class.
    Anusvara,
    /// Zero width joiner.
    Zwj,
    /// Zero width non-joiner.
    Zwnj,
    /// Control character.
    Control,
    /// Variation selector.
    Vs,
    /// Other character.
    Other,
}

impl Default for ShapeClass {
    fn default() -> Self {
        Self::Base
    }
}*/

impl CharCluster {
    pub(crate) fn new(info: ClusterInfo, chars: Vec<Char>) -> Self {
        CharCluster {
            info,
            chars,
            style_index: 0,
            len: 0,
            map_len: 0,
            start: 0, // TODO(conor) + end
            end: 0,
            force_normalize: false, // TODO(conor) ?
            comp: Form::new(),
            decomp: Form::new(),
            form: FormKind::Original,
            best_ratio: 0.,
        }
    }

    fn composed(&mut self) -> Option<&[Char]> {
        unimplemented!();
    }

    fn decomposed(&mut self) -> Option<&[Char]> {
        unimplemented!();
    }

    pub fn map(&mut self, f: impl Fn(char) -> GlyphId) -> Status {
        let len = self.len;
        if len == 0 {
            return Status::Complete;
        }
        let mut glyph_ids = [0u16; MAX_CLUSTER_SIZE];
        let prev_ratio = self.best_ratio;
        let mut ratio;
        if self.force_normalize && self.composed().is_some() {
            ratio = self.comp.map(&f, &mut glyph_ids, self.best_ratio);
            if ratio > self.best_ratio {
                self.best_ratio = ratio;
                self.form = FormKind::NFC;
                if ratio >= 1. {
                    return Status::Complete;
                }
            }
        }
        ratio = Mapper {
            chars: &mut self.chars[..self.len as usize],
            map_len: self.map_len.max(1),
        }
            .map(&f, &mut glyph_ids, self.best_ratio);
        if ratio > self.best_ratio {
            self.best_ratio = ratio;
            self.form = FormKind::Original;
            if ratio >= 1. {
                return Status::Complete;
            }
        }
        if len > 1 && self.decomposed().is_some() {
            ratio = self.decomp.map(&f, &mut glyph_ids, self.best_ratio);
            if ratio > self.best_ratio {
                self.best_ratio = ratio;
                self.form = FormKind::NFD;
                if ratio >= 1. {
                    return Status::Complete;
                }
            }
            if !self.force_normalize && self.composed().is_some() {
                ratio = self.comp.map(&f, &mut glyph_ids, self.best_ratio);
                if ratio > self.best_ratio {
                    self.best_ratio = ratio;
                    self.form = FormKind::NFC;
                    if ratio >= 1. {
                        return Status::Complete;
                    }
                }
            }
        }
        if self.best_ratio > prev_ratio {
            Status::Keep
        } else {
            Status::Discard
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
enum FormKind {
    Original,
    NFD,
    NFC,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum FormState {
    None,
    Valid,
    Invalid,
}

#[derive(Copy, Clone)]
struct Form {
    pub chars: [Char; MAX_CLUSTER_SIZE],
    pub len: u8,
    pub map_len: u8,
    pub state: FormState,
}

impl Form {
    fn new() -> Self {
        Self {
            chars: [DEFAULT_CHAR; MAX_CLUSTER_SIZE],
            len: 0,
            map_len: 0,
            state: FormState::None,
        }
    }

    fn clear(&mut self) {
        self.len = 0;
        self.map_len = 0;
        self.state = FormState::None;
    }

    fn chars(&self) -> &[Char] {
        &self.chars[..self.len as usize]
    }

    fn setup(&mut self) {
        self.map_len = (self
            .chars()
            .iter()
            .filter(|c| !c.is_control_character)
            .count() as u8)
            .max(1);
    }

    fn map(
        &mut self,
        f: &impl Fn(char) -> u16,
        glyphs: &mut [u16; MAX_CLUSTER_SIZE],
        best_ratio: f32,
    ) -> f32 {
        Mapper {
            chars: &mut self.chars[..self.len as usize],
            map_len: self.map_len,
        }
            .map(f, glyphs, best_ratio)
    }
}

struct Mapper<'a> {
    chars: &'a mut [Char],
    map_len: u8,
}

impl<'a> Mapper<'a> {
    fn map(
        &mut self,
        f: &impl Fn(char) -> u16,
        glyphs: &mut [u16; MAX_CLUSTER_SIZE],
        best_ratio: f32,
    ) -> f32 {
        if self.map_len == 0 {
            return 1.;
        }
        let mut mapped = 0;
        for (c, g) in self.chars.iter().zip(glyphs.iter_mut()) {
            if !c.contributes_to_shaping {
                *g = f(c.ch);
                if self.map_len == 1 {
                    mapped += 1;
                }
            } else {
                let gid = f(c.ch);
                *g = gid;
                if gid != 0 {
                    mapped += 1;
                }
            }
        }
        let ratio = mapped as f32 / self.map_len as f32;
        if ratio > best_ratio {
            for (ch, glyph) in self.chars.iter_mut().zip(glyphs) {
                ch.glyph_id = *glyph;
            }
        }
        ratio
    }
}

const DEFAULT_CHAR: Char = Char {
    ch: ' ',
    is_control_character: false,
    contributes_to_shaping: true,
    glyph_id: 0,
    data: 0,
    offset: 0,
};