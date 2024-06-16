// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{Stretch, Style, Weight};
use fontconfig_cache_parser::*;
use std::io::Read;
use std::path::PathBuf;

impl Stretch {
    fn from_fc(width: i32) -> Self {
        match width {
            63 => Self::EXTRA_CONDENSED,
            87 => Self::SEMI_CONDENSED,
            113 => Self::SEMI_EXPANDED,
            _ => Self::from_ratio(width as f32 / 100.0),
        }
    }
}

impl Style {
    fn from_fc(slant: i32) -> Self {
        match slant {
            100 => Self::Italic,
            110 => Self::Oblique(None),
            _ => Self::Normal,
        }
    }
}

impl Weight {
    fn from_fc(weight: i32) -> Self {
        const MAP: &[(i32, i32)] = &[
            (0, 0),
            (100, 0),
            (200, 40),
            (300, 50),
            (350, 55),
            (380, 75),
            (400, 80),
            (500, 100),
            (600, 180),
            (700, 200),
            (800, 205),
            (900, 210),
            (1000, 215),
        ];
        for (i, (ot, fc)) in MAP.iter().skip(1).enumerate() {
            if weight == *fc {
                return Self::new(*ot as f32);
            }
            if weight < *fc {
                let weight = weight as f32;
                let fc_a = MAP[i - 1].1 as f32;
                let fc_b = *fc as f32;
                let ot_a = MAP[i - 1].1 as f32;
                let ot_b = *ot as f32;
                let t = (fc_a - fc_b) / (weight - fc_a);
                return Self::new(ot_a + (ot_b - ot_a) * t);
            }
        }
        Self::new(1000.0)
    }
}

#[derive(Default)]
pub struct CachedFont {
    pub family: Vec<String>,
    pub path: PathBuf,
    pub index: u32,
    pub stretch: Stretch,
    pub style: Style,
    pub weight: Weight,
    pub coverage: Coverage,
}

impl CachedFont {
    fn clear(&mut self) {
        self.family.clear();
        self.path.clear();
        self.index = 0;
        self.coverage.clear();
    }
}

pub fn parse_caches(paths: &[PathBuf], mut f: impl FnMut(&CachedFont)) {
    let mut buffer = vec![];
    let mut name_free_list = vec![];
    let mut cached_font = CachedFont::default();
    for path in paths {
        let Ok(dir) = path.canonicalize().and_then(std::fs::read_dir) else {
            return;
        };
        for path in dir.filter_map(|entry| entry.ok()).map(|entry| entry.path()) {
            buffer.clear();
            let Ok(file_size) = path.metadata() else {
                continue;
            };
            buffer.resize(file_size.len() as usize, 0);
            let Ok(mut file) = std::fs::OpenOptions::new().read(true).open(&path) else {
                continue;
            };
            let Ok(_) = file.read(&mut buffer) else {
                continue;
            };
            let Ok(set) = Cache::from_bytes(&buffer).and_then(|cache| cache.set()) else {
                continue;
            };
            let Ok(fonts) = set.fonts() else { continue };
            for font in fonts.flatten() {
                if parse_font(&font, &mut name_free_list, &mut cached_font).is_some() {
                    f(&cached_font);
                }
            }
        }
    }
}

fn parse_font(
    pattern: &Pattern,
    name_free_list: &mut Vec<String>,
    font: &mut CachedFont,
) -> Option<()> {
    name_free_list.append(&mut font.family);
    font.clear();
    for elt in pattern.elts().ok()? {
        let Ok(obj) = elt.object() else {
            continue;
        };
        match obj {
            Object::Family => {
                for val in elt.values().ok()? {
                    let val = val.ok()?;
                    if let Value::String(s) = val {
                        let mut name = name_free_list.pop().unwrap_or_default();
                        name.clear();
                        name.push_str(core::str::from_utf8(s.str().ok()?).ok()?);
                        font.family.push(name);
                    }
                }
            }
            Object::File => {
                for val in elt.values().ok()? {
                    let val = val.ok()?;
                    if let Value::String(s) = val {
                        font.path.clear();
                        font.path.push(core::str::from_utf8(s.str().ok()?).ok()?);
                        if font.path.extension() == Some(std::ffi::OsStr::new("t1")) {
                            return None;
                        }
                    }
                }
            }
            Object::Slant => {
                for val in elt.values().ok()? {
                    if let Value::Int(i) = val.ok()? {
                        font.style = Style::from_fc(i as _);
                    }
                }
            }
            Object::Weight => {
                for val in elt.values().ok()? {
                    if let Value::Int(i) = val.ok()? {
                        font.weight = Weight::from_fc(i as _);
                    }
                }
            }
            Object::Width => {
                for val in elt.values().ok()? {
                    if let Value::Int(i) = val.ok()? {
                        font.stretch = Stretch::from_fc(i as _);
                    }
                }
            }
            Object::Index => {
                for val in elt.values().ok()? {
                    let val = val.ok()?;
                    if let Value::Int(i) = val {
                        font.index = i as u32;
                        // Ignore named instances
                        if font.index >> 16 != 0 {
                            return None;
                        }
                    }
                }
            }
            Object::CharSet => {
                for val in elt.values().ok()? {
                    let val = val.ok()?;
                    if let Value::CharSet(set) = val {
                        font.coverage.clear();
                        font.coverage
                            .numbers
                            .extend_from_slice(set.numbers().ok()?.as_slice().ok()?);
                        for leaf in set.leaves().ok()? {
                            let leaf = leaf.ok()?;
                            font.coverage.leaves.push(leaf);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if !font.family.is_empty() && !font.path.as_os_str().is_empty() {
        Some(())
    } else {
        None
    }
}

#[derive(Clone, Default)]
pub struct Coverage {
    numbers: Vec<u16>,
    leaves: Vec<CharSetLeaf>,
}

impl Coverage {
    pub fn compute_for_str(&self, s: &str) -> usize {
        s.chars()
            .map(|ch| self.contains(ch as _).unwrap_or(false) as usize)
            .sum()
    }

    pub fn contains(&self, ch: u32) -> Option<bool> {
        let hi = ((ch >> 8) & 0xffff) as u16;
        match self.numbers.binary_search(&hi) {
            // The unwrap will succeed because numbers and leaves have the same length.
            Ok(idx) => {
                let leaf = self.leaves.get(idx)?;
                let lo = (ch & 0xff) as u8;
                Some(leaf.contains_byte(lo))
            }
            Err(_) => Some(false),
        }
    }

    fn clear(&mut self) {
        self.numbers.clear();
        self.leaves.clear();
    }
}
