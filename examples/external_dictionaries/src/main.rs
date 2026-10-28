// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Loads a dictionary file and uses it to lay out text.
//!
//! See the docs on [`parley::DictionaryProvider`] for more details.

use std::{error::Error, io};

use icu_provider::buf::AsDeserializingBufferProvider;
use icu_provider_blob::BlobDataProvider;
use parley::{FontContext, Layout, LayoutContext};

mod cache;
use cache::CachingDictionary;

fn main() -> Result<(), Box<dyn Error>> {
    let bytes = read_dictionary()?;
    let blob = BlobDataProvider::try_new_from_blob(bytes.into_boxed_slice())?;
    // Load once, then reuse the cache for every layout.
    let dictionaries = CachingDictionary::try_new(&blob.as_deserializing())?;

    // "The weather is very nice today" in Thai, written without spaces between words.
    // Word boundaries: วันนี้ | อากาศ | ดี | มาก.
    let text = "วันนี้อากาศดีมาก";
    let mut fonts = FontContext::new();
    let mut context = LayoutContext::new();
    let mut builder = context.ranged_builder(&mut fonts, text, 1., true);
    // Try commenting out this line to see what happens!
    builder.set_dictionary_provider(Some(&dictionaries));
    let mut layout: Layout<()> = builder.build(text);
    let max_width = 50.;
    layout.break_all_lines(Some(max_width));

    println!("Source text: {text}");
    println!("Lines at max width: {max_width}:");
    for (index, line) in layout.lines().enumerate() {
        println!("  {}: \"{}\"", index + 1, &text[line.text_range()]);
    }
    Ok(())
}

fn read_dictionary() -> Result<Vec<u8>, io::Error> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../_output/dictionaries.postcard"
    );
    std::fs::read(path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            io::Error::new(
                error.kind(),
                "dictionary file missing; run `cargo run -p external_dictionaries --features generate --bin generate_dictionaries` first",
            )
        } else {
            error
        }
    })
}
