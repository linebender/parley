// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! # Parley Dev
//!
//! This crate provides utilities for developing Parley.

use std::path::{Path, PathBuf};

/// The directories that contain the font files.
pub fn font_dirs() -> impl Iterator<Item = PathBuf> {
    let assets_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts");
    [
        assets_dir.join("arimo_fonts"),
        assets_dir.join("roboto_fonts"),
        assets_dir.join("noto_fonts"),
    ]
    .into_iter()
}

/// The font families that are available in the assets/fonts directory.
pub const FONT_FAMILIES: &[&str] = &["Arimo", "Roboto", "Noto Kufi Arabic"];

/// A sample to be used for development.
#[derive(Debug)]
pub struct Sample {
    /// The name of the sample.
    pub name: &'static str,
    /// The text of the sample.
    pub text: &'static str,
}

/// A collection of text samples.
#[derive(Debug)]
pub struct TextSamples {
    /// The Arabic text sample.
    pub arabic: Sample,
    /// The Latin text sample.
    pub latin: Sample,
    /// The Japanese text sample.
    pub japanese: Sample,
}

impl TextSamples {
    /// Creates a new collection of text samples.
    pub const fn new() -> Self {
        let arabic = include_str!("../assets/text_samples/arabic.txt");
        let latin = include_str!("../assets/text_samples/latin.txt");
        let japanese = include_str!("../assets/text_samples/japanese.txt");
        Self {
            arabic: Sample {
                name: "arabic",
                text: arabic,
            },
            latin: Sample {
                name: "latin",
                text: latin,
            },
            japanese: Sample {
                name: "japanese",
                text: japanese,
            },
        }
    }
}

impl Default for TextSamples {
    fn default() -> Self {
        Self::new()
    }
}
