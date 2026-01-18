// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use parley::LineHeight as ParleyLineHeight;
use styled_text::ComputedLineHeight;

#[inline]
pub(crate) fn to_parley_line_height(line_height: ComputedLineHeight) -> ParleyLineHeight {
    match line_height {
        ComputedLineHeight::MetricsRelative(x) => ParleyLineHeight::MetricsRelative(x),
        ComputedLineHeight::FontSizeRelative(x) => ParleyLineHeight::FontSizeRelative(x),
        ComputedLineHeight::Px(px) => ParleyLineHeight::Absolute(px),
    }
}
