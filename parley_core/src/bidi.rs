// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Unicode bidirectional algorithm.

use alloc::vec::Vec;
use icu_properties::props::{BidiClass, BidiMirroringGlyph, BidiPairedBracketType};

/// Type alias for a bidirectional level.
pub type BidiLevel = u8;

/// Resolver for the Unicode bidirectional algorithm.
#[derive(Clone, Default)]
pub struct BidiResolver {
    base_level: BidiLevel,
    levels: Vec<BidiLevel>,
    initial_types: Vec<BidiClass>,
    types: Vec<BidiClass>,
    brackets: Vec<(usize, char, BidiMirroringGlyph)>,
    bracket_pairs: Vec<(usize, usize)>,
    runs: Vec<Run>,
    indices: Vec<usize>,
    flags: u16,
}

impl core::fmt::Debug for BidiResolver {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BidiResolver")
            .field("base_level", &self.base_level)
            .field("levels", &self.levels)
            .finish_non_exhaustive()
    }
}

impl BidiResolver {
    /// Creates a new resolver.
    pub fn new() -> Self {
        Self {
            base_level: 0,
            levels: Vec::new(),
            initial_types: Vec::new(),
            types: Vec::new(),
            brackets: Vec::new(),
            bracket_pairs: Vec::new(),
            runs: Vec::new(),
            indices: Vec::new(),
            flags: 0,
        }
    }

    /// Returns the base level of the text.
    pub fn base_level(&self) -> u8 {
        self.base_level
    }

    /// Returns the sequence of bidi levels corresponding to all characters in the
    /// paragraph.
    pub fn levels(&self) -> &[BidiLevel] {
        &self.levels
    }

    /// Clears the resolver state.
    pub fn clear(&mut self) {
        self.initial_types.clear();
        self.levels.clear();
        self.types.clear();
        self.brackets.clear();
        self.bracket_pairs.clear();
        self.flags = 0;
        self.base_level = 0;
    }

    /// Resolves a paragraph with the specified base direction and
    /// precomputed types.
    pub fn resolve(
        &mut self,
        chars: impl Iterator<Item = (char, (BidiClass, BidiMirroringGlyph))>,
        base_level: Option<u8>,
    ) {
        self.clear();
        let mut needs_bidi = false;
        let mut len = 0;
        for (i, (ch, (t, bracket))) in chars.enumerate() {
            self.initial_types.push(t);

            if bracket.paired_bracket_type != BidiPairedBracketType::None {
                self.brackets.push((i, ch, bracket));
            }

            needs_bidi = needs_bidi || mask(t) & BIDI_MASK != 0;
            len += 1;
        }
        self.base_level = match base_level {
            Some(level) => level & 1,
            _ => Self::default_level(&self.initial_types),
        };
        if !needs_bidi && self.base_level == 0 {
            self.flags |= 1;
            self.levels.resize(len, self.base_level);
            return;
        }
        self.types.extend_from_slice(&self.initial_types);
        self.resolve_levels();
        self.resolve_runs();
        //self.dump_sequences();
        for i in 0..self.runs.len() {
            if self.runs[i].in_sequence {
                continue;
            }
            self.types.truncate(len);
            self.indices.clear();
            let mut cur = i;
            let level = self.runs[i].level;
            let sos = self.runs[i].sos;
            let mut eos;
            loop {
                let run = &self.runs[cur];
                for i in run.start..run.end {
                    let ty = self.types[i];
                    if !is_removed_by_x9(ty) {
                        self.types.push(ty);
                        self.indices.push(i);
                    }
                }
                eos = run.eos;
                cur = match run.next {
                    Some(i) => i,
                    None => break,
                };
            }
            self.resolve_sequence(level, sos, eos, self.indices.len());
        }
        for i in 0..len {
            let t = self.initial_types[i];
            if t == BidiClass::SegmentSeparator || t == BidiClass::ParagraphSeparator {
                self.levels[i] = self.base_level;
                for j in (0..i).rev() {
                    let t = self.initial_types[j];
                    if is_removed_by_x9(t) {
                        continue;
                    } else if t == BidiClass::WhiteSpace
                        || is_isolate_initiator(t)
                        || t == BidiClass::PopDirectionalIsolate
                    {
                        self.levels[j] = self.base_level;
                    } else {
                        break;
                    }
                }
            } else if is_removed_by_x9(t) {
                if i == 0 {
                    self.levels[i] = self.base_level;
                } else {
                    self.levels[i] = self.levels[i - 1];
                }
                //self.levels[i] = 0xFF;
            }
        }
        for i in (0..len).rev() {
            let t = self.initial_types[i];
            if is_removed_by_x9(t) {
                continue;
            } else if t == BidiClass::WhiteSpace
                || is_isolate_initiator(t)
                || t == BidiClass::PopDirectionalIsolate
            {
                //self.levels[i] = self.base_level;
            } else {
                break;
            }
        }
    }

    fn default_level(types: &[BidiClass]) -> u8 {
        let mut isolates = 0;
        for ty in types {
            let ty = *ty;
            match ty {
                BidiClass::RightToLeftIsolate
                | BidiClass::LeftToRightIsolate
                | BidiClass::FirstStrongIsolate => isolates += 1,
                BidiClass::PopDirectionalIsolate if isolates > 0 => isolates -= 1,
                BidiClass::LeftToRight | BidiClass::RightToLeft | BidiClass::ArabicLetter
                    if isolates == 0 =>
                {
                    return if ty == BidiClass::LeftToRight { 0 } else { 1 };
                }
                _ => {}
            }
        }
        0
    }

    fn default_level_until_pdi(types: &[BidiClass]) -> u8 {
        let mut isolates = 0;
        for ty in types {
            let ty = *ty;
            match ty {
                BidiClass::RightToLeftIsolate
                | BidiClass::LeftToRightIsolate
                | BidiClass::FirstStrongIsolate => isolates += 1,
                BidiClass::PopDirectionalIsolate => {
                    if isolates > 0 {
                        isolates -= 1;
                    } else {
                        return 0;
                    }
                }
                BidiClass::LeftToRight | BidiClass::RightToLeft | BidiClass::ArabicLetter
                    if isolates == 0 =>
                {
                    return if ty == BidiClass::LeftToRight { 0 } else { 1 };
                }
                _ => {}
            }
        }
        0
    }

    fn resolve_levels(&mut self) {
        let base = self.base_level;
        let len = self.types.len();
        self.levels.clear();
        self.levels.resize(len, 0);
        let mut stack = Stack::new();
        let mut overflow_isolates = 0;
        let mut overflow_embedding = 0;
        let mut valid_isolates = 0;
        stack.push(base, BidiClass::OtherNeutral, false);
        for i in 0..len {
            let t = self.types[i];
            let tmask = mask(t);
            if tmask & EXPLICIT_MASK != 0 {
                let is_isolate = tmask & ISOLATE_MASK != 0;
                let is_rtl = if t == BidiClass::FirstStrongIsolate && i + 1 < len {
                    Self::default_level_until_pdi(&self.types[i + 1..]) == 1
                } else {
                    tmask & RTL_MASK != 0
                };
                if is_isolate {
                    self.levels[i] = stack.embedding_level();
                    let os = stack.override_status();
                    if os != BidiClass::OtherNeutral {
                        self.types[i] = os;
                    }
                }
                let new_level = if is_rtl {
                    (stack.embedding_level() + 1) | 1
                } else {
                    (stack.embedding_level() + 2) & !1
                };
                if new_level <= MAX_STACK as u8 && overflow_isolates == 0 && overflow_embedding == 0
                {
                    if is_isolate {
                        valid_isolates += 1;
                    }
                    stack.push(
                        new_level,
                        if t == BidiClass::LeftToRightOverride {
                            BidiClass::LeftToRight
                        } else if t == BidiClass::RightToLeftOverride {
                            BidiClass::RightToLeft
                        } else {
                            BidiClass::OtherNeutral
                        },
                        is_isolate,
                    );
                } else if is_isolate {
                    overflow_isolates += 1;
                } else if overflow_isolates == 0 {
                    overflow_embedding += 1;
                }
            } else if t == BidiClass::PopDirectionalIsolate {
                if overflow_isolates > 0 {
                    overflow_isolates -= 1;
                } else if valid_isolates == 0 {
                    // empty
                } else {
                    overflow_embedding = 0;
                    while !stack.isolate_status() {
                        stack.pop();
                    }
                    stack.pop();
                    valid_isolates -= 1;
                }
                self.levels[i] = stack.embedding_level();
                if stack.override_status() != BidiClass::OtherNeutral {
                    self.types[i] = stack.override_status();
                }
            } else if t == BidiClass::PopDirectionalFormat {
                self.levels[i] = stack.embedding_level();
                if overflow_isolates > 0 {
                    // empty
                } else if overflow_embedding > 0 {
                    overflow_embedding -= 1;
                } else if !stack.isolate_status() && stack.depth >= 2 {
                    stack.pop();
                }
            } else if t == BidiClass::ParagraphSeparator {
                stack.depth = 1;
                overflow_isolates = 0;
                overflow_embedding = 0;
                valid_isolates = 0;
                self.levels[i] = base;
            } else if t != BidiClass::BoundaryNeutral {
                self.levels[i] = stack.embedding_level();
                if stack.override_status() != BidiClass::OtherNeutral {
                    self.types[i] = stack.override_status();
                }
            }
        }
    }

    fn resolve_runs(&mut self) {
        let len = self.types.len();
        self.runs.clear();
        let mut start = 0;
        while start < len {
            if !is_removed_by_x9(self.types[start]) {
                break;
            }
            start += 1;
        }
        if start == len {
            return;
        }
        let mut level = self.levels[start];
        let mut offset = 0;
        for i in start + 1..len {
            if is_removed_by_x9(self.types[i]) {
                continue;
            }
            if self.levels[i] != level {
                self.runs.push(Run::new(level, offset, i));
                offset = i;
                level = self.levels[i];
            }
        }
        if offset < len {
            self.runs.push(Run::new(level, offset, len));
        }
        for run in &mut self.runs {
            while run.start < run.end {
                if is_removed_by_x9(self.types[run.start]) {
                    run.start += 1;
                } else {
                    break;
                }
            }
            while run.end > run.start {
                if is_removed_by_x9(self.types[run.end - 1]) {
                    run.end -= 1;
                } else {
                    break;
                }
            }
            if run.start == run.end {
                continue;
            }
            if self.types[run.start] == BidiClass::PopDirectionalIsolate {
                run.starts_with_pdi = true;
            }
            let mut prev_level = self.base_level;
            for i in (0..run.start).rev() {
                if !is_removed_by_x9(self.types[i]) {
                    prev_level = self.levels[i];
                    break;
                }
            }
            run.sos = type_from_level(prev_level.max(run.level));
            if is_isolate_initiator(self.initial_types[run.end - 1]) {
                run.ends_with_isolate = true;
                run.eos = type_from_level(self.base_level.max(run.level));
            } else {
                let mut next_level = self.base_level;
                for i in run.end..len {
                    if !is_removed_by_x9(self.types[i]) {
                        next_level = self.levels[i];
                        break;
                    }
                }
                run.eos = type_from_level(next_level.max(run.level));
            }
        }
        for i in 0..self.runs.len() {
            if self.runs[i].ends_with_isolate {
                let level = self.runs[i].level;
                for j in i + 1..self.runs.len() {
                    if self.runs[j].starts_with_pdi && self.runs[j].level == level {
                        self.runs[i].next = Some(j);
                        self.runs[j].in_sequence = true;
                        break;
                    }
                }
            }
        }
    }

    #[expect(clippy::needless_range_loop, reason = "Deferred")]
    fn resolve_sequence(&mut self, level: u8, sos: BidiClass, eos: BidiClass, len: usize) {
        if len == 0 {
            return;
        }
        const W1_MASK: u32 = mask(BidiClass::LeftToRightIsolate)
            | mask(BidiClass::RightToLeftIsolate)
            | mask(BidiClass::FirstStrongIsolate)
            | mask(BidiClass::PopDirectionalIsolate);
        const W2_MASK: u32 = mask(BidiClass::LeftToRight)
            | mask(BidiClass::RightToLeft)
            | mask(BidiClass::ArabicLetter);
        const W4_MASK: u32 = mask(BidiClass::EuropeanSeparator) | mask(BidiClass::CommonSeparator);
        let mut prev = sos;
        let mut prev_strong = prev;
        let types = &mut self.types[self.initial_types.len()..];
        for i in 0..len {
            let mut t = types[i];
            let tmask = mask(t);
            if t == BidiClass::NonspacingMark {
                // W1
                types[i] = prev;
            } else {
                if tmask & W1_MASK != 0 {
                    prev = BidiClass::OtherNeutral;
                    continue;
                }
                if t == BidiClass::EuropeanNumber {
                    // W2
                    if prev_strong == BidiClass::ArabicLetter {
                        t = BidiClass::ArabicNumber;
                        types[i] = t;
                    }
                } else if tmask & W2_MASK != 0 {
                    prev_strong = t;
                    // W3
                    if t == BidiClass::ArabicLetter {
                        t = BidiClass::RightToLeft;
                        types[i] = t;
                    }
                } else if tmask & W4_MASK != 0 && i < (len - 1) {
                    // W4
                    let mut next = types[i + 1];
                    if next == BidiClass::EuropeanNumber && prev_strong == BidiClass::ArabicLetter {
                        next = BidiClass::ArabicNumber;
                    }
                    if prev == BidiClass::EuropeanNumber && next == BidiClass::EuropeanNumber {
                        t = BidiClass::EuropeanNumber;
                        types[i] = t;
                    } else if t == BidiClass::CommonSeparator
                        && prev == BidiClass::ArabicNumber
                        && next == BidiClass::ArabicNumber
                    {
                        t = BidiClass::ArabicNumber;
                        types[i] = t;
                    }
                }
                prev = t;
            }
        }
        // W5
        let mut i = 0;
        while i < len {
            if types[i] == BidiClass::EuropeanTerminator {
                let limit = find_limit(types, i, BidiClass::EuropeanTerminator);
                let mut t = if i == 0 { sos } else { types[i - 1] };
                if t != BidiClass::EuropeanNumber {
                    t = if limit == len { eos } else { types[limit] };
                }
                if t == BidiClass::EuropeanNumber {
                    for j in i..limit {
                        types[j] = BidiClass::EuropeanNumber;
                    }
                }
                i = limit;
            }
            i += 1;
        }
        // W6, W7
        const W6_MASK: u32 = mask(BidiClass::EuropeanSeparator)
            | mask(BidiClass::EuropeanTerminator)
            | mask(BidiClass::CommonSeparator);
        prev_strong = sos;
        for i in 0..len {
            let t = types[i];
            if mask(t) & W6_MASK != 0 {
                // W6
                types[i] = BidiClass::OtherNeutral;
            } else if t == BidiClass::EuropeanNumber {
                // W7
                if prev_strong == BidiClass::LeftToRight {
                    types[i] = BidiClass::LeftToRight;
                }
            } else if t == BidiClass::LeftToRight || t == BidiClass::RightToLeft {
                prev_strong = t;
            }
        }
        // N0
        if !self.brackets.is_empty() {
            let base_brackets = self.bracket_pairs.len();
            let mut bracket_stack = BracketStack::new();
            for i in 0..len {
                if types[i] != BidiClass::OtherNeutral {
                    continue;
                }
                let index = self.indices[i];
                if let Ok(index) = self.brackets.binary_search_by(|x| x.0.cmp(&index)) {
                    let (_, ch, bracket) = self.brackets[index];
                    match bracket.paired_bracket_type {
                        BidiPairedBracketType::Open => {
                            if bracket_stack.depth == MAX_BRACKET_STACK {
                                break;
                            }
                            bracket_stack.push(i, bracket.mirroring_glyph.unwrap());
                        }
                        BidiPairedBracketType::Close => {
                            if let Some(open) = bracket_stack.find_and_pop(ch) {
                                self.bracket_pairs.push((open, i));
                            }
                        }
                        _ => {}
                    }
                }
            }
            if self.bracket_pairs.len() > base_brackets {
                let embed_dir = if level & 1 != 0 {
                    BidiClass::RightToLeft
                } else {
                    BidiClass::LeftToRight
                };
                let bracket_pairs = &mut self.bracket_pairs[base_brackets..];
                bracket_pairs.sort_unstable_by_key(|a| a.0);
                for pair in bracket_pairs {
                    let mut pair_dir = BidiClass::OtherNeutral;
                    for i in pair.0 + 1..pair.1 {
                        let dir = match types[i] {
                            BidiClass::EuropeanNumber
                            | BidiClass::ArabicNumber
                            | BidiClass::ArabicLetter
                            | BidiClass::RightToLeft => BidiClass::RightToLeft,
                            BidiClass::LeftToRight => BidiClass::LeftToRight,
                            _ => BidiClass::OtherNeutral,
                        };
                        if dir == BidiClass::OtherNeutral {
                            continue;
                        }
                        pair_dir = dir;
                        if dir == embed_dir {
                            break;
                        }
                    }
                    if pair_dir == BidiClass::OtherNeutral {
                        pair.0 = self.indices[pair.0];
                        pair.1 = self.indices[pair.1];
                        continue;
                    }
                    if pair_dir != embed_dir {
                        pair_dir = sos;
                        for i in (0..pair.0).rev() {
                            let dir = match types[i] {
                                BidiClass::EuropeanNumber
                                | BidiClass::ArabicNumber
                                | BidiClass::ArabicLetter
                                | BidiClass::RightToLeft => BidiClass::RightToLeft,
                                BidiClass::LeftToRight => BidiClass::LeftToRight,
                                _ => BidiClass::OtherNeutral,
                            };
                            if dir != BidiClass::OtherNeutral {
                                pair_dir = dir;
                                break;
                            }
                        }
                        if pair_dir == embed_dir || pair_dir == BidiClass::OtherNeutral {
                            pair_dir = embed_dir;
                        }
                    }
                    types[pair.0] = pair_dir;
                    types[pair.1] = pair_dir;
                    for i in pair.0 + 1..pair.1 {
                        let index = self.indices[i];
                        if self.initial_types[index] == BidiClass::NonspacingMark {
                            types[i] = pair_dir;
                        } else {
                            break;
                        }
                    }
                    for i in pair.1 + 1..len {
                        let index = self.indices[i];
                        if self.initial_types[index] == BidiClass::NonspacingMark {
                            types[i] = pair_dir;
                        } else {
                            break;
                        }
                    }
                    pair.0 = self.indices[pair.0];
                    pair.1 = self.indices[pair.1];
                }
            }
        }
        // N1, N2
        const N_MASK: u32 = mask(BidiClass::ParagraphSeparator)
            | mask(BidiClass::SegmentSeparator)
            | mask(BidiClass::WhiteSpace)
            | mask(BidiClass::OtherNeutral)
            | mask(BidiClass::RightToLeftIsolate)
            | mask(BidiClass::LeftToRightIsolate)
            | mask(BidiClass::FirstStrongIsolate)
            | mask(BidiClass::PopDirectionalIsolate);
        let mut i = 0;
        while i < len {
            let t = types[i];
            if mask(t) & N_MASK != 0 {
                let offset = i;
                let limit = find_limit_by_mask(types, offset, N_MASK);
                let mut leading;
                let mut trailing;
                if offset == 0 {
                    leading = sos;
                } else {
                    leading = types[offset - 1];
                    if leading == BidiClass::ArabicNumber || leading == BidiClass::EuropeanNumber {
                        leading = BidiClass::RightToLeft;
                    }
                }
                if limit == len {
                    trailing = eos;
                } else {
                    trailing = types[limit];
                    if trailing == BidiClass::ArabicNumber || trailing == BidiClass::EuropeanNumber
                    {
                        trailing = BidiClass::RightToLeft;
                    }
                }
                let resolved = if leading == trailing {
                    // N1
                    leading
                } else {
                    // N2
                    if level & 1 != 0 {
                        BidiClass::RightToLeft
                    } else {
                        BidiClass::LeftToRight
                    }
                };
                for j in offset..limit {
                    types[j] = resolved;
                }
                i = limit - 1;
            }
            i += 1;
        }
        // Implicit levels
        if level & 1 == 0 {
            // I1
            for i in 0..len {
                let index = self.indices[i];
                let t = types[i];
                if t == BidiClass::RightToLeft {
                    self.levels[index] = level + 1;
                } else if t != BidiClass::LeftToRight {
                    self.levels[index] = level + 2;
                } else {
                    self.levels[index] = level;
                }
            }
        } else {
            // I2
            for i in 0..len {
                let index = self.indices[i];
                let t = types[i];
                if t != BidiClass::RightToLeft {
                    self.levels[index] = level + 1;
                } else {
                    self.levels[index] = level;
                }
            }
        }
    }
}

/// Returns a default bidi type for a level.
pub(crate) fn type_from_level(level: BidiLevel) -> BidiClass {
    if level & 1 == 0 {
        BidiClass::LeftToRight
    } else {
        BidiClass::RightToLeft
    }
}

/// Computes an ordering for a sequence of bidi runs based on levels.
pub(crate) fn _reorder<F>(order: &mut [usize], levels: F)
where
    F: Fn(usize) -> BidiLevel,
{
    let mut max_level = 0;
    let mut lowest_odd_level = 255;
    for (i, o) in order.iter_mut().enumerate() {
        *o = i;
        let level = levels(i);
        if level > max_level {
            max_level = level;
        }
        if level & 1 != 0 && level < lowest_odd_level {
            lowest_odd_level = level;
        }
    }
    let len = order.len();
    for level in (lowest_odd_level..=max_level).rev() {
        let mut i = 0;
        while i < len {
            if levels(i) >= level {
                let mut end = i + 1;
                while end < len && levels(end) >= level {
                    end += 1;
                }
                let mut j = i;
                let mut k = end - 1;
                while j < k {
                    order.swap(j, k);
                    j += 1;
                    k -= 1;
                }
                i = end;
            }
            i += 1;
        }
    }
}

/// Returns whether the character needs bidirectional resolution.
#[inline(always)]
pub fn needs_bidi_resolution(bidi_class: BidiClass) -> bool {
    mask(bidi_class) & BIDI_MASK != 0
}

const OVERRIDE_MASK: u32 = mask(BidiClass::RightToLeftEmbedding)
    | mask(BidiClass::LeftToRightEmbedding)
    | mask(BidiClass::RightToLeftOverride)
    | mask(BidiClass::LeftToRightOverride);
const ISOLATE_MASK: u32 = mask(BidiClass::RightToLeftIsolate)
    | mask(BidiClass::LeftToRightIsolate)
    | mask(BidiClass::FirstStrongIsolate);
const EXPLICIT_MASK: u32 = OVERRIDE_MASK | ISOLATE_MASK;
const RTL_MASK: u32 = mask(BidiClass::RightToLeftEmbedding)
    | mask(BidiClass::RightToLeftOverride)
    | mask(BidiClass::RightToLeftIsolate);
const REMOVED_BY_X9_MASK: u32 =
    OVERRIDE_MASK | mask(BidiClass::PopDirectionalFormat) | mask(BidiClass::BoundaryNeutral);
const BIDI_MASK: u32 = EXPLICIT_MASK
    | mask(BidiClass::RightToLeft)
    | mask(BidiClass::ArabicLetter)
    | mask(BidiClass::ArabicNumber);
const _RESET_MASK: u32 =
    ISOLATE_MASK | mask(BidiClass::PopDirectionalIsolate) | mask(BidiClass::WhiteSpace);

fn is_isolate_initiator(ty: BidiClass) -> bool {
    mask(ty) & ISOLATE_MASK != 0
}

pub(crate) fn is_removed_by_x9(ty: BidiClass) -> bool {
    mask(ty) & REMOVED_BY_X9_MASK != 0
}

pub(crate) fn _is_reset(ty: BidiClass) -> bool {
    mask(ty) & _RESET_MASK != 0
}

fn find_limit(types: &[BidiClass], offset: usize, ty: BidiClass) -> usize {
    let mut len = offset;
    for &t in &types[offset..] {
        if t != ty {
            break;
        }
        len += 1;
    }
    len
}

fn find_limit_by_mask(types: &[BidiClass], offset: usize, mask: u32) -> usize {
    let mut len = offset;
    for &t in &types[offset..] {
        if self::mask(t) & mask == 0 {
            break;
        }
        len += 1;
    }
    len
}

#[derive(Clone)]
struct Run {
    level: u8,
    ends_with_isolate: bool,
    starts_with_pdi: bool,
    sos: BidiClass,
    eos: BidiClass,
    start: usize,
    end: usize,
    in_sequence: bool,
    next: Option<usize>,
}

impl Run {
    fn new(level: u8, start: usize, end: usize) -> Self {
        Self {
            level,
            ends_with_isolate: false,
            starts_with_pdi: false,
            sos: BidiClass::OtherNeutral,
            eos: BidiClass::OtherNeutral,
            start,
            end,
            in_sequence: false,
            next: None,
        }
    }
}

const MAX_STACK: usize = 125;

struct Stack {
    embedding_level: [u8; MAX_STACK + 1],
    override_status: [BidiClass; MAX_STACK + 1],
    isolate_status: [bool; MAX_STACK + 1],
    depth: usize,
}

impl Stack {
    fn new() -> Self {
        Self {
            depth: 0,
            embedding_level: [0; MAX_STACK + 1],
            override_status: [BidiClass::OtherNeutral; MAX_STACK + 1],
            isolate_status: [false; MAX_STACK + 1],
        }
    }

    fn push(&mut self, level: u8, override_status: BidiClass, isolate_status: bool) {
        let d = self.depth;
        self.embedding_level[d] = level;
        self.override_status[d] = override_status;
        self.isolate_status[d] = isolate_status;
        self.depth += 1;
    }

    fn pop(&mut self) {
        if self.depth > 1 {
            self.depth -= 1;
        }
    }

    fn embedding_level(&self) -> u8 {
        self.embedding_level[self.depth - 1]
    }

    fn override_status(&self) -> BidiClass {
        self.override_status[self.depth - 1]
    }

    fn isolate_status(&self) -> bool {
        self.isolate_status[self.depth - 1]
    }
}

const MAX_BRACKET_STACK: usize = 63;

struct BracketStack {
    openers: [(usize, char); MAX_BRACKET_STACK],
    depth: usize,
}

impl BracketStack {
    fn new() -> Self {
        Self {
            openers: [(0, '\0'); MAX_BRACKET_STACK],
            depth: 0,
        }
    }

    fn push(&mut self, offset: usize, closer: char) {
        self.openers[self.depth] = (offset, closer);
        self.depth += 1;
    }

    fn find_and_pop(&mut self, closer: char) -> Option<usize> {
        if self.depth == 0 {
            return None;
        }
        for i in (0..self.depth).rev() {
            let c = self.openers[i].1;
            if c == closer
                || (c == '\u{232A}' && closer == '\u{3009}')
                || (c == '\u{3009}' && closer == '\u{232A}')
            {
                self.depth = i;
                return Some(self.openers[i].0);
            }
        }
        None
    }
}

const fn mask(t: BidiClass) -> u32 {
    1 << (t.to_icu4c_value() as u32)
}

/// Reorders a single line's elements into visual left-to-right display order from their bidi
/// levels, applying UAX #9 rule L2.
///
/// See <https://www.unicode.org/reports/tr9/#L2>.
///
/// Analysis and shaping store everything in *logical* (source) order; turning that into visual
/// left-to-right order is a per-line decision, because it must run after line breaking (rule L1
/// resets levels at line edges, see <https://www.unicode.org/reports/tr9/#L1>). UAX #9 says:
///
/// > **L2.** From the highest level found in the text to the lowest odd level on each line,
/// > including intermediate levels not actually present in the text, reverse any contiguous
/// > sequence of characters that are at that level or higher.
///
/// `items` holds the elements of a single line in logical order, and `level` returns each element's
/// bidi level (e.g. [`Run::bidi_level`](crate::Run::bidi_level)). The levels must already reflect
/// any rule L1 resets the caller performs (e.g. trailing whitespace reset to the base level). The
/// slice is reordered in place, left to right from the inline start (left, for a left-to-right base
/// direction) to the inline end.
///
/// # Example
///
/// You can use this to reorder runs, or to generate an index array creating a visual order mapping.
///
/// ```
/// use parley_core::reorder_visual;
/// // Base left-to-right with an embedded right-to-left word ("hello WORLD!",
/// // where WORLD is RTL). One bidi level per element, in logical order:
/// let levels = [0u8, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 0];
/// let mut order: Vec<usize> = (0..levels.len()).collect();
/// reorder_visual(&mut order, |&i| levels[i]);
/// // The five RTL elements are reversed; everything else keeps its place.
/// assert_eq!(order, [0, 1, 2, 3, 4, 5, 10, 9, 8, 7, 6, 11]);
/// ```
pub fn reorder_visual<T>(items: &mut [T], level: impl Fn(&T) -> u8) {
    let mut max_level = 0;
    let mut lowest_odd_level = u8::MAX;
    for item in items.iter() {
        let l = level(item);
        max_level = max_level.max(l);
        if l & 1 != 0 {
            lowest_odd_level = lowest_odd_level.min(l);
        }
    }
    // With no odd level (e.g. pure left-to-right) the range below is empty and
    // nothing is reversed, leaving the elements in their original order.
    let len = items.len();
    for threshold in (lowest_odd_level..=max_level).rev() {
        let mut i = 0;
        while i < len {
            if level(&items[i]) >= threshold {
                // Reverse the maximal run of elements at this level or higher.
                let mut end = i + 1;
                while end < len && level(&items[end]) >= threshold {
                    end += 1;
                }
                items[i..end].reverse();
                i = end;
            } else {
                i += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::reorder_visual;
    use alloc::vec::Vec;

    #[test]
    fn reorders_by_level() {
        // (line levels in logical order, expected visual order).
        let cases: &[(&[u8], &[usize])] = &[
            // Empty line.
            (&[], &[]),
            // Pure left-to-right: no odd level, so nothing is reversed.
            (&[0, 0, 0, 0], &[0, 1, 2, 3]),
            // "hello WORLD!" (WORLD is RTL): only the level-1 span is reversed.
            (&[0, 0, 1, 1, 1, 0], &[0, 1, 4, 3, 2, 5]),
            // Entirely right-to-left.
            (&[1, 1, 1], &[2, 1, 0]),
            // Base LTR (0), an RTL word (1) wrapping an LTR number (2): the
            // number keeps its internal order while the RTL word is reversed.
            (&[0, 1, 1, 2, 2, 1], &[0, 5, 3, 4, 2, 1]),
        ];
        // Recover the visual order as a permutation by reordering an index array,
        // reusing the buffer across cases.
        let mut order = Vec::new();
        for &(levels, expected) in cases {
            order.clear();
            order.extend(0..levels.len());
            reorder_visual(&mut order, |&i| levels[i]);
            assert_eq!(order.as_slice(), expected, "levels: {levels:?}");
        }
    }
}
