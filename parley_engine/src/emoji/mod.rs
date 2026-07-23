// Copyright 2026 Christian Hansen
// SPDX-License-Identifier: MIT
// <https://github.com/chansen/c-emoji>
//
// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Port of [c-emoji] (MIT) and follow the [UTS51](Unicode Technical Standard #51).
//!
//! [c-emoji]: <https://github.com/chansen/c-emoji>
//! [UTS51]: <https://www.unicode.org/reports/tr51/>

mod dfa;
mod types;

pub use dfa::EmojiDFA;
pub use types::{EmojiPresentationStyle, EmojiSegmentationCategory};
