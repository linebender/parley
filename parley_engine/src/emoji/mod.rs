// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! This implementation is based on [emoji segmenter]'s Ragel grammar (Apache-2.0).
//!
//! And follow the [UTS51](Unicode Technical Standard #51).
//!
//! [emoji segmenter]: <https://github.com/google/emoji-segmenter>
//! [UTS51]: <https://www.unicode.org/reports/tr51/>

mod dfa;
mod types;

pub use dfa::EmojiDFA;
pub use types::{EmojiPresentationStyle, EmojiSegmentationCategory};
