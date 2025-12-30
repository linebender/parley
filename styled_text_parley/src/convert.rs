// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::borrow::Cow;

use parley::setting::Tag as ParleyTag;
use parley::style::{
    FontFamily as ParleyFontFamily, FontFeature, FontSettings, FontStack as ParleyFontStack,
    FontVariation, GenericFamily as ParleyGenericFamily, LineHeight as ParleyLineHeight,
    OverflowWrap as ParleyOverflowWrap, TextWrapMode as ParleyTextWrapMode,
    WordBreak as ParleyWordBreak,
};
use text_style::{
    FontFamily, FontStack, FontStyle, FontWeight, FontWidth, GenericFamily, OverflowWrap, Setting,
    Tag, TextWrapMode, WordBreak,
};
use text_style_resolve::ComputedLineHeight;

pub(crate) fn to_parley_font_stack(stack: &FontStack) -> ParleyFontStack<'_> {
    ParleyFontStack::List(Cow::Owned(
        stack.iter().map(to_parley_font_family).collect(),
    ))
}

pub(crate) fn to_parley_font_weight(weight: FontWeight) -> parley::style::FontWeight {
    parley::style::FontWeight::new(weight.0)
}

pub(crate) fn to_parley_font_width(width: FontWidth) -> parley::style::FontWidth {
    parley::style::FontWidth::from_ratio(width.0)
}

pub(crate) fn to_parley_font_style(style: FontStyle) -> parley::style::FontStyle {
    match style {
        FontStyle::Normal => parley::style::FontStyle::Normal,
        FontStyle::Italic => parley::style::FontStyle::Italic,
        FontStyle::Oblique(angle) => parley::style::FontStyle::Oblique(angle),
        _ => {
            debug_assert!(false, "unhandled FontStyle variant");
            parley::style::FontStyle::Normal
        }
    }
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

pub(crate) fn to_parley_word_break(value: WordBreak) -> ParleyWordBreak {
    match value {
        WordBreak::Normal => ParleyWordBreak::Normal,
        WordBreak::BreakAll => ParleyWordBreak::BreakAll,
        WordBreak::KeepAll => ParleyWordBreak::KeepAll,
        _ => {
            debug_assert!(false, "unhandled WordBreak variant");
            ParleyWordBreak::Normal
        }
    }
}

pub(crate) fn to_parley_overflow_wrap(value: OverflowWrap) -> ParleyOverflowWrap {
    match value {
        OverflowWrap::Normal => ParleyOverflowWrap::Normal,
        OverflowWrap::Anywhere => ParleyOverflowWrap::Anywhere,
        OverflowWrap::BreakWord => ParleyOverflowWrap::BreakWord,
        _ => {
            debug_assert!(false, "unhandled OverflowWrap variant");
            ParleyOverflowWrap::Normal
        }
    }
}

pub(crate) fn to_parley_text_wrap_mode(value: TextWrapMode) -> ParleyTextWrapMode {
    match value {
        TextWrapMode::Wrap => ParleyTextWrapMode::Wrap,
        TextWrapMode::NoWrap => ParleyTextWrapMode::NoWrap,
        _ => {
            debug_assert!(false, "unhandled TextWrapMode variant");
            ParleyTextWrapMode::Wrap
        }
    }
}

pub(crate) fn to_parley_tag(tag: Tag) -> ParleyTag {
    let bytes = tag.to_bytes();
    ParleyTag::new(&bytes)
}

fn to_parley_font_family(family: &FontFamily) -> ParleyFontFamily<'_> {
    match family {
        FontFamily::Named(name) => ParleyFontFamily::Named(Cow::Borrowed(name.as_ref())),
        FontFamily::Generic(g) => ParleyFontFamily::Generic(to_parley_generic_family(*g)),
    }
}

fn to_parley_generic_family(family: GenericFamily) -> ParleyGenericFamily {
    match family {
        GenericFamily::Serif => ParleyGenericFamily::Serif,
        GenericFamily::SansSerif => ParleyGenericFamily::SansSerif,
        GenericFamily::Monospace => ParleyGenericFamily::Monospace,
        GenericFamily::Cursive => ParleyGenericFamily::Cursive,
        GenericFamily::Fantasy => ParleyGenericFamily::Fantasy,
        GenericFamily::SystemUi => ParleyGenericFamily::SystemUi,
        GenericFamily::Emoji => ParleyGenericFamily::Emoji,
        GenericFamily::Math => ParleyGenericFamily::Math,
        GenericFamily::Fangsong => ParleyGenericFamily::FangSong,
        _ => {
            debug_assert!(false, "unhandled GenericFamily variant");
            ParleyGenericFamily::SansSerif
        }
    }
}

fn to_parley_variation(setting: Setting<f32>) -> FontVariation {
    FontVariation {
        tag: to_parley_tag(setting.tag),
        value: setting.value,
    }
}

fn to_parley_feature(setting: Setting<u16>) -> FontFeature {
    FontFeature {
        tag: to_parley_tag(setting.tag),
        value: setting.value,
    }
}
