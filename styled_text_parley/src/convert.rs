// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::borrow::Cow;

use parley::style::{
    FontFamily as ParleyFontFamily, FontFeature, FontSettings, FontStack as ParleyFontStack,
    FontVariation, LineHeight as ParleyLineHeight,
};
use text_style::{FontFamily, FontStack, Setting};
use text_style_resolve::ComputedLineHeight;

pub(crate) fn to_parley_font_stack(stack: &FontStack) -> ParleyFontStack<'_> {
    ParleyFontStack::List(Cow::Owned(
        stack.iter().map(to_parley_font_family).collect(),
    ))
}

pub(crate) fn to_parley_variations(settings: &[Setting<f32>]) -> FontSettings<'_, FontVariation> {
    FontSettings::List(Cow::Owned(
        settings.iter().copied().map(to_parley_variation).collect(),
    ))
}

pub(crate) fn to_parley_features(settings: &[Setting<u16>]) -> FontSettings<'_, FontFeature> {
    FontSettings::List(Cow::Owned(
        settings.iter().copied().map(to_parley_feature).collect(),
    ))
}

pub(crate) fn to_parley_line_height(line_height: ComputedLineHeight) -> ParleyLineHeight {
    match line_height {
        ComputedLineHeight::MetricsRelative(x) => ParleyLineHeight::MetricsRelative(x),
        ComputedLineHeight::FontSizeRelative(x) => ParleyLineHeight::FontSizeRelative(x),
        ComputedLineHeight::Px(px) => ParleyLineHeight::Absolute(px),
        _ => {
            debug_assert!(false, "unhandled ComputedLineHeight variant");
            ParleyLineHeight::default()
        }
    }
}

fn to_parley_font_family(family: &FontFamily) -> ParleyFontFamily<'_> {
    match family {
        FontFamily::Named(name) => ParleyFontFamily::Named(Cow::Borrowed(name.as_ref())),
        FontFamily::Generic(g) => ParleyFontFamily::Generic(*g),
    }
}

fn to_parley_variation(setting: Setting<f32>) -> FontVariation {
    FontVariation {
        tag: setting.tag,
        value: setting.value,
    }
}

fn to_parley_feature(setting: Setting<u16>) -> FontFeature {
    FontFeature {
        tag: setting.tag,
        value: setting.value,
    }
}
