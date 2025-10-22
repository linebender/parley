use smallvec::SmallVec;

use crate::analysis::AnalysisDataSources;

/// The maximum number of characters in a single cluster.
const MAX_CLUSTER_SIZE: usize = 32;

#[derive(Debug)]
pub(crate) struct CharCluster {
    pub chars: SmallVec<[Char; 2]>,
    pub is_emoji: bool,
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

impl CharCluster {
    pub(crate) fn range(&self) -> SourceRange {
        SourceRange {
            start: self.start,
            end: self.end,
        }
    }
}

/// Source range of a cluster in code units.
#[derive(Copy, Clone)]
pub(crate) struct SourceRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct Char {
    /// The character.
    pub ch: char,
    /// Whether the character
    pub is_control_character: bool,
    /// True if the character should be considered when mapping glyphs.
    pub contributes_to_shaping: bool,
    /// Nominal glyph identifier.
    pub glyph_id: GlyphId,
    /// Indexes into the list of styles for the containing text run, to find the style applicable
    /// to this character.
    pub style_index: u16,
}

pub(crate) type GlyphId = u16;

/// Whitespace content of a cluster.
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Debug)]
#[repr(u8)]
pub(crate) enum Whitespace {
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
}

impl Whitespace {
    /// Returns true for space or no break space.
    pub(crate) fn is_space_or_nbsp(self) -> bool {
        matches!(self, Self::Space | Self::NoBreakSpace)
    }
}

/// Iterative status of mapping a character cluster to nominal glyph identifiers.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Status {
    /// Mapping should be skipped.
    Discard,
    /// The best mapping so far.
    Keep,
    /// Complete mapping.
    Complete,
}

impl CharCluster {
    pub(crate) fn new(
        chars: smallvec::SmallVec<[Char; 2]>,
        is_emoji: bool,
        len: u8,
        map_len: u8,
        start: u32,
        end: u32,
        force_normalize: bool,
    ) -> Self {
        CharCluster {
            chars,
            is_emoji,
            len,
            map_len,
            start,
            end,
            force_normalize,
            comp: Form::new(),
            decomp: Form::new(),
            form: FormKind::Original,
            best_ratio: 0.,
        }
    }

    /// Returns the primary style index for the cluster.
    pub(crate) fn style_index(&self) -> u16 {
        self.chars[0].style_index
    }

    fn decomposed(&mut self, analysis_data_sources: &AnalysisDataSources) -> Option<&[Char]> {
        match self.decomp.state {
            FormState::Invalid => None,
            FormState::None => {
                self.decomp.state = FormState::Invalid;

                // Create a string from the original characters to normalize
                let mut orig_str = String::with_capacity(self.len as usize * 4);
                for ch in &self.chars[..self.len as usize] {
                    orig_str.push(ch.ch);
                }

                // Normalize to NFD (decomposed) form
                let nfd_str = analysis_data_sources.decomposing_normalizer().normalize(&orig_str);

                // Copy the characters back to our form structure
                let mut i = 0;
                for c in nfd_str.chars() {
                    if i == MAX_CLUSTER_SIZE {
                        return None;
                    }

                    // Use the first character as a template for other properties
                    let mut copy = self.chars[0];
                    copy.ch = c;
                    if i >= self.decomp.chars.len() {
                        self.decomp.chars.push(copy);
                    } else {
                        self.decomp.chars[i] = copy;
                    }
                    i += 1;
                }

                if i == 0 {
                    return None;
                }

                self.decomp.len = i as u8;
                self.decomp.state = FormState::Valid;
                self.decomp.setup();
                Some(self.decomp.chars())
            }
            FormState::Valid => Some(self.decomp.chars()),
        }
    }

    fn composed(&mut self, analysis_data_sources: &AnalysisDataSources) -> Option<&[Char]> {
        match self.comp.state {
            FormState::Invalid => None,
            FormState::None => {
                // First, we need decomposed characters
                if self.decomposed(analysis_data_sources).map(|chars| chars.len()).unwrap_or(0) == 0 {
                    self.comp.state = FormState::Invalid;
                    return None;
                }

                self.comp.state = FormState::Invalid;

                // Create a string from the decomposed characters to normalize
                let mut decomp_str = String::with_capacity(self.decomp.len as usize * 4);
                for ch in &self.decomp.chars()[..self.decomp.len as usize] {
                    decomp_str.push(ch.ch);
                }

                // Normalize to NFC (composed) form
                let nfc_str = analysis_data_sources.composing_normalizer().normalize(&decomp_str);

                // Copy the characters back to our form structure
                let mut i = 0;
                for c in nfc_str.chars() {
                    if i >= MAX_CLUSTER_SIZE {
                        self.comp.state = FormState::Invalid;
                        return None;
                    }

                    // Use the first decomposed character as a template for other properties
                    let mut ch_copy = self.decomp.chars[0];
                    ch_copy.ch = c;
                    if i >= self.comp.chars.len() {
                        self.comp.chars.push(ch_copy);
                    } else {
                        self.comp.chars[i] = ch_copy;
                    }
                    i += 1;
                }

                if i == 0 {
                    return None;
                }

                self.comp.len = i as u8;
                self.comp.state = FormState::Valid;
                self.comp.setup();
                Some(self.comp.chars())
            }
            FormState::Valid => Some(self.comp.chars()),
        }
    }

    pub(crate) fn map(
        &mut self,
        f: impl Fn(char) -> GlyphId,
        analysis_data_sources: &AnalysisDataSources,
    ) -> Status {
        let len = self.len;
        if len == 0 {
            return Status::Complete;
        }
        let mut glyph_ids = [0u16; MAX_CLUSTER_SIZE];
        let prev_ratio = self.best_ratio;
        let mut ratio;
        if self.force_normalize && self.composed(analysis_data_sources).is_some() {
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
        if len > 1 && self.decomposed(analysis_data_sources).is_some() {
            ratio = self.decomp.map(&f, &mut glyph_ids, self.best_ratio);
            if ratio > self.best_ratio {
                self.best_ratio = ratio;
                self.form = FormKind::NFD;
                if ratio >= 1. {
                    return Status::Complete;
                }
            }
            if !self.force_normalize && self.composed(analysis_data_sources).is_some() {
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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
enum FormKind {
    Original,
    NFD,
    NFC,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum FormState {
    None,
    Valid,
    Invalid,
}

#[derive(Clone, Debug)]
struct Form {
    pub chars: SmallVec<[Char; 1]>,
    pub len: u8,
    pub map_len: u8,
    pub state: FormState,
}

impl Form {
    fn new() -> Self {
        Self {
            chars: smallvec::smallvec![DEFAULT_CHAR; 1],
            len: 0,
            map_len: 0,
            state: FormState::None,
        }
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
    style_index: 0,
};
