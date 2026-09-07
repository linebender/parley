// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Some whitespace-related utilities.

use parley_engine::shape::Whitespace;

/// Whether this is whitespace that is allowed to hang past the line.
///
/// Following [CSS Text 4 § 4.3.2][css-hanging], non-breaking spaces don't hang.
///
/// [css-hanging]: https://www.w3.org/TR/css-text-4/#white-space-phase-2
///
// Note: CSS Text 4 also includes "other space separators" here, but we don't include them (yet?).
// See https://github.com/linebender/parley/pull/762#discussion_r3923722770.
#[inline(always)]
pub(crate) const fn whitespace_can_hang(whitespace: Whitespace) -> bool {
    matches!(
        whitespace,
        Whitespace::Space | Whitespace::Tab | Whitespace::Newline
    )
}
