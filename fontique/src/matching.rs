// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Implementation of the CSS font matching algorithm.

use core::ops::Deref;

use super::attributes::{DEFAULT_OBLIQUE_ANGLE, FontStyle, FontWeight, FontWidth};
use super::font::FontInfo;
use core::cmp::Ordering;
use smallvec::SmallVec;

#[derive(Copy, Clone)]
pub struct FontMatchingInfo {
    width: i32,
    style: FontStyle,
    weight: f32,
    has_slnt: bool,
}

pub fn match_font(
    fonts: impl IntoIterator<Item = impl Into<FontMatchingInfo>>,
    width: FontWidth,
    style: FontStyle,
    weight: FontWeight,
    synthesize_style: bool,
) -> Option<usize> {
    let set = CandidateFontSet::new(fonts);
    set.match_font_impl(width, style, weight, synthesize_style)
}

// Private implementation details

impl From<&FontInfo> for FontMatchingInfo {
    fn from(info: &FontInfo) -> Self {
        Self {
            width: (info.width().ratio() * 100.0) as i32,
            style: info.style(),
            weight: info.weight().value(),
            has_slnt: info.has_slant_axis(),
        }
    }
}

#[derive(Copy, Clone)]
struct CandidateFont {
    index: usize,
    info: FontMatchingInfo,
}
impl Deref for CandidateFont {
    type Target = FontMatchingInfo;
    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

struct CandidateFontSet(SmallVec<[CandidateFont; 16]>);

impl CandidateFontSet {
    fn new(fonts: impl IntoIterator<Item = impl Into<FontMatchingInfo>>) -> Self {
        let inner = fonts
            .into_iter()
            .enumerate()
            .map(|(index, info)| CandidateFont {
                index,
                info: info.into(),
            })
            .collect();
        Self(inner)
    }

    fn has_width(&self, width: i32) -> bool {
        self.0.iter().any(|f| f.width == width)
    }

    fn has_style(&self, style: FontStyle) -> bool {
        self.0.iter().any(|f| f.style == style)
    }

    fn has_variable_font_with_slnt_axis(&self) -> bool {
        self.0.iter().any(|f| f.has_slnt)
    }

    fn max_width_below(&self, width: i32) -> Option<i32> {
        self.0
            .iter()
            .filter(|f| f.width < width)
            .max_by_key(|f| f.width)
            .map(|f| f.width)
    }

    fn min_width_above(&self, width: i32) -> Option<i32> {
        self.0
            .iter()
            .filter(|f| f.width > width)
            .min_by_key(|f| f.width)
            .map(|f| f.width)
    }

    fn fonts_matching_weight(
        &self,
        predicate: impl Fn(f32) -> bool,
    ) -> impl Iterator<Item = &CandidateFont> {
        self.0.iter().filter(move |f| predicate(f.weight))
    }

    fn max_weight_matching(&self, predicate: impl Fn(f32) -> bool) -> Option<&CandidateFont> {
        self.fonts_matching_weight(predicate)
            .max_by(|x, y| x.weight.partial_cmp(&y.weight).unwrap_or(Ordering::Less))
    }

    fn min_weight_matching(&self, predicate: impl Fn(f32) -> bool) -> Option<&CandidateFont> {
        self.fonts_matching_weight(predicate)
            .min_by(|x, y| x.weight.partial_cmp(&y.weight).unwrap_or(Ordering::Less))
    }

    fn fonts_matching_oblique_angle(
        &self,
        predicate: impl Fn(f32) -> bool,
    ) -> impl Iterator<Item = (&CandidateFont, f32)> {
        self.0
            .iter()
            .filter_map(move |f| match f.style.oblique_angle() {
                Some(a) if predicate(a) => Some((f, a)),
                _ => None,
            })
    }

    fn min_oblique_angle_matching(&self, predicate: impl Fn(f32) -> bool) -> Option<FontStyle> {
        self.fonts_matching_oblique_angle(predicate)
            .min_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(Ordering::Less))
            .map(|f| f.0.style)
    }

    fn max_oblique_angle_matching(&self, predicate: impl Fn(f32) -> bool) -> Option<FontStyle> {
        self.fonts_matching_oblique_angle(predicate)
            .max_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(Ordering::Less))
            .map(|f| f.0.style)
    }

    fn fallback_style(&self, synthesize_style: bool, predicate: impl Fn(f32) -> bool) -> FontStyle {
        if synthesize_style {
            if self.has_style(FontStyle::Normal) {
                FontStyle::Normal
            } else {
                self.0[0].style
            }
        } else {
            // Choose an italic style
            if self.has_style(FontStyle::Italic) {
                FontStyle::Italic
            } else {
                // oblique values less than or equal to 0deg are checked in descending order
                self.max_oblique_angle_matching(predicate)
                    .unwrap_or(self.0[0].style)
            }
        }
    }

    // Method consumes self because it mutates it's containing collection
    // which means that calling this twice could not work
    fn match_font_impl(
        mut self,
        width: FontWidth,
        style: FontStyle,
        weight: FontWeight,
        synthesize_style: bool,
    ) -> Option<usize> {
        // Early return for case of 0 or 1 fonts where matching is trivial
        match self.0.len() {
            0 => return None,
            1 => return Some(0),
            _ => {}
        }

        let width = (width.ratio() * 100.0) as i32;
        let weight = weight.value();

        // font-width is tried first:
        let use_width = if !self.has_width(width) {
            // If the desired width value is less than or equal to 100% then...
            if width <= 100 {
                // Width values below the desired width value are checked in descending order followed by
                // width values above the desired width value in ascending order until a match is found.
                self.max_width_below(width)
                    .or_else(|| self.min_width_above(width))
                    .unwrap_or(width)
            }
            // Otherwise...
            else {
                // Width values above the desired width value are checked in ascending order followed by
                // width values below the desired width value in descending order until a match is found.
                self.min_width_above(width)
                    .or_else(|| self.max_width_below(width))
                    .unwrap_or(width)
            }
        } else {
            width
        };
        self.0.retain(|f| f.width == use_width);

        // NOTE: this code uses an oblique threshold of 14deg rather than
        // the current value of 11deg in the spec.
        // See: https://github.com/w3c/csswg-drafts/issues/2295
        const OBLIQUE_THRESHOLD: f32 = DEFAULT_OBLIQUE_ANGLE;

        // font-style is tried next:
        let mut _use_slnt = false;
        let use_style = if self.has_style(style) {
            style
        } else {
            // If the value of font-style is italic:
            if style == FontStyle::Italic {
                // oblique values greater than or equal to 14deg are checked in
                // ascending order
                self.min_oblique_angle_matching(|a| a >= OBLIQUE_THRESHOLD)
                    // followed by positive oblique values below 14deg in descending order
                    .or_else(|| {
                        self.max_oblique_angle_matching(|a| a > 0.0 && a < OBLIQUE_THRESHOLD)
                    })
                    // If no match is found, oblique values less than or equal to 0deg
                    // are checked in descending order until a match is found.
                    .or_else(|| self.max_oblique_angle_matching(|a| a < 0.0))
                    .unwrap_or(self.0[0].style)
            }
            // If the value of font-style is oblique...
            else if let Some(angle) = style.oblique_angle() {
                // and the requested angle is greater than or equal to 14deg
                if angle >= OBLIQUE_THRESHOLD {
                    // oblique values greater than or equal to angle are checked in
                    // ascending order
                    self.min_oblique_angle_matching(|a| a >= angle)
                        // followed by positive oblique values below angle in descending order
                        .or_else(|| self.max_oblique_angle_matching(|a| a > 0.0 && a < angle))
                        .unwrap_or_else(|| {
                            // If font-synthesis-style has the value auto, then for variable
                            // fonts with a slnt axis a match is created by selfting the slnt
                            // value with the specified oblique value; otherwise, a fallback
                            // match is produced by geometric shearing to the specified
                            // oblique value.
                            if synthesize_style && self.has_variable_font_with_slnt_axis() {
                                _use_slnt = true;
                                style
                            } else {
                                self.fallback_style(synthesize_style, |a| a <= 0.0)
                            }
                        })
                }
                // if the requested angle is greater than or equal to 0deg
                // and less than 14deg
                else if angle >= 0. {
                    // positive oblique values below angle in descending order
                    self.max_oblique_angle_matching(|a| a > 0.0 && a < angle)
                        // followed by oblique values greater than or equal to angle in
                        // ascending order
                        .or_else(|| self.min_oblique_angle_matching(|a| a >= angle))
                        .unwrap_or_else(|| {
                            // If font-synthesis-style has the value auto, then for variable
                            // fonts with a slnt axis a match is created by selfting the slnt
                            // value with the specified oblique value; otherwise, a fallback
                            // match is produced by geometric shearing to the specified
                            // oblique value.
                            if synthesize_style && self.has_variable_font_with_slnt_axis() {
                                _use_slnt = true;
                                style
                            } else {
                                self.fallback_style(synthesize_style, |a| a <= 0.0)
                            }
                        })
                }
                // -14deg < angle < 0deg
                else if angle > -OBLIQUE_THRESHOLD {
                    // negative oblique values above angle in ascending order
                    self.min_oblique_angle_matching(|a| a < 0. && a > angle)
                        // followed by oblique values less than or equal to angle in
                        // descending order
                        .or_else(|| self.max_oblique_angle_matching(|a| a <= angle))
                        .unwrap_or_else(|| {
                            // If font-synthesis-style has the value auto, then for variable
                            // fonts with a slnt axis a match is created by selfting the slnt
                            // value with the specified oblique value; otherwise, a fallback
                            // match is produced by geometric shearing to the specified
                            // oblique value.
                            if synthesize_style && self.has_variable_font_with_slnt_axis() {
                                _use_slnt = true;
                                style
                            } else {
                                self.fallback_style(synthesize_style, |a| a >= 0.0)
                            }
                        })
                }
                // angle < -14 deg
                else {
                    // oblique values less than or equal to angle are checked in
                    // descending order
                    self.max_oblique_angle_matching(|a| a >= angle)
                        // followed by negative oblique values above angle in ascending order
                        .or_else(|| self.min_oblique_angle_matching(|a| a < 0.0 && a > angle))
                        .unwrap_or_else(|| {
                            // If font-synthesis-style has the value auto, then for variable
                            // fonts with a slnt axis a match is created by selfting the slnt
                            // value with the specified oblique value; otherwise, a fallback
                            // match is produced by geometric shearing to the specified
                            // oblique value.
                            if synthesize_style && self.has_variable_font_with_slnt_axis() {
                                _use_slnt = true;
                                style
                            } else {
                                self.fallback_style(synthesize_style, |a| a >= 0.0)
                            }
                        })
                }
            }
            // If the value of font-style is normal...
            else {
                // oblique values greater than or equal to 0deg are checked in
                // ascending order
                self.min_oblique_angle_matching(|a| a >= 0.0)
                    // followed by italic fonts
                    .or_else(|| {
                        self.0
                            .iter()
                            .find(|f| f.style == FontStyle::Italic)
                            .map(|f| f.style)
                    })
                    // followed by oblique values less than 0deg in descending order
                    .or_else(|| self.max_oblique_angle_matching(|a| a < 0.0))
                    .unwrap_or(self.0[0].style)
            }
        };

        self.0.retain(|f| f.style == use_style);

        // font-weight is matched next:
        if let Some(index) = self.0.iter().position(|f| f.weight == weight) {
            Some(index)
        } else {
            // If the desired weight is inclusively between 400 and 500...
            if (400.0..=500.0).contains(&weight) {
                self
                    // weights greater than or equal to the target weight are checked in ascending
                    // order until 500 is hit and checked
                    .min_weight_matching(|w| w >= weight && w <= 500.0)
                    // followed by weights less than the target weight in descending order
                    .or_else(|| self.max_weight_matching(|w| w < weight))
                    // followed by weights greater than 500, until a match is found.
                    .or_else(|| self.min_weight_matching(|w| w > 500.0))
            }
            // If the desired weight is less than 400...
            else if weight < 400.0 {
                // weights less than or equal to the target weight are checked in descending order
                self.max_weight_matching(|w| w < weight)
                    // followed by weights greater than the target weight in ascending order
                    .or_else(|| self.min_weight_matching(|w| w > weight))
            }
            // If the desired weight is greater than 500...
            else {
                // weights greater than or equal to the target weight are checked in ascending
                self.min_weight_matching(|w| w >= weight)
                    // followed by weights less than the target weight in descending order
                    .or_else(|| self.max_weight_matching(|w| w < weight))
            }
            .map(|found| found.index)
        }
    }
}
