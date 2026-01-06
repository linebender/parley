// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Canonical text samples for style property testing.
//!
//! Each sample is designed to exercise specific text layout behaviors.
//! Tests should use these samples selectively based on what the property affects.

/// Simple Latin text - baseline for most tests
pub(crate) const LATIN: &str = "The quick brown fox jumps.";

/// Latin with multiple lines for testing line-related properties
pub(crate) const LATIN_MULTILINE: &str = "Line one.\nLine two.\nLine three.";

/// Arabic text for RTL and complex shaping
pub(crate) const ARABIC: &str = "مرحبا بالعالم";

/// Mixed bidirectional text
pub(crate) const MIXED_BIDI: &str = "Hello مرحبا World";

/// Text with common ligatures (fi, fl, ff, ffi, ffl)
pub(crate) const LIGATURES: &str = "fi fl ff ffi ffl office";

/// Text with spaces for word spacing tests
pub(crate) const SPACED: &str = "one two three four five";

