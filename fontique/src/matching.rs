// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Implementation of the CSS font matching algorithm.

use core::ops::{Deref, DerefMut};

use super::attributes::{DEFAULT_OBLIQUE_ANGLE, FontStyle, FontWeight, FontWidth};
use super::font::FontInfo;
use smallvec::SmallVec;

#[derive(Copy, Clone)]
pub struct FontMatchingInfo {
    width: i32,
    style: FontStyle,
    weight: f32,
    has_slnt: bool,
}

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

pub fn match_font(
    fonts: impl IntoIterator<Item = impl Into<FontMatchingInfo>>,
    width: FontWidth,
    style: FontStyle,
    weight: FontWeight,
    synthesize_style: bool,
) -> Option<usize> {
    use core::cmp::Ordering::Less;
    const OBLIQUE_THRESHOLD: f32 = DEFAULT_OBLIQUE_ANGLE;

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
    impl Deref for CandidateFontSet {
        type Target = SmallVec<[CandidateFont; 16]>;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl DerefMut for CandidateFontSet {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

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

        fn max_weight_matching(&self, predicate: impl Fn(f32) -> bool) -> Option<&CandidateFont> {
            use core::cmp::Ordering::Less;
            self.0
                .iter()
                .filter(|f| predicate(f.weight))
                .max_by(|x, y| x.weight.partial_cmp(&y.weight).unwrap_or(Less))
        }

        fn min_weight_matching(&self, predicate: impl Fn(f32) -> bool) -> Option<&CandidateFont> {
            use core::cmp::Ordering::Less;
            self.0
                .iter()
                .filter(|f| predicate(f.weight))
                .min_by(|x, y| x.weight.partial_cmp(&y.weight).unwrap_or(Less))
        }
    }

    let mut set = CandidateFontSet::new(fonts);

    // Early return for case of 0 or 1 fonts where matching is trivial
    match set.len() {
        0 => return None,
        1 => return Some(0),
        _ => {}
    }

    let width = (width.ratio() * 100.0) as i32;
    let weight = weight.value();

    // font-width is tried first:
    let use_width = if !set.has_width(width) {
        // If the desired width value is less than or equal to 100% then...
        if width <= 100 {
            // Width values below the desired width value are checked in descending order followed by
            // width values above the desired width value in ascending order until a match is found.
            set.max_width_below(width)
                .or_else(|| set.min_width_above(width))
                .unwrap_or(width)
        }
        // Otherwise...
        else {
            // Width values above the desired width value are checked in ascending order followed by
            // width values below the desired width value in descending order until a match is found.
            set.min_width_above(width)
                .or_else(|| set.max_width_below(width))
                .unwrap_or(width)
        }
    } else {
        width
    };
    set.retain(|f| f.width == use_width);

    let oblique_fonts = set.iter().filter_map(|f| oblique_style(f.style));
    // font-style is tried next:
    // NOTE: this code uses an oblique threshold of 14deg rather than
    // the current value of 20deg in the spec.
    // See: https://github.com/w3c/csswg-drafts/issues/2295
    let mut use_style = style;
    let mut _use_slnt = false;
    if !set.iter().any(|f| f.style == use_style) {
        // If the value of font-style is italic:
        if style == FontStyle::Italic {
            // oblique values greater than or equal to 14deg are checked in
            // ascending order
            if let Some(found) = oblique_fonts
                .clone()
                .filter(|(_, a)| *a >= OBLIQUE_THRESHOLD)
                .min_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(Less))
            {
                use_style = found.0;
            }
            // followed by positive oblique values below 14deg in descending order
            else if let Some(found) = oblique_fonts
                .clone()
                .filter(|(_, a)| *a > 0. && *a < OBLIQUE_THRESHOLD)
                .max_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(Less))
            {
                use_style = found.0;
            }
            // If no match is found, oblique values less than or equal to 0deg
            // are checked in descending order until a match is found.
            else if let Some(found) = oblique_fonts
                .clone()
                .filter(|(_, a)| *a < 0.)
                .max_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(Less))
            {
                use_style = found.0;
            } else {
                use_style = set[0].style;
            }
        }
        // If the value of font-style is oblique...
        else if let Some(angle) = style.oblique_angle() {
            // and the requested angle is greater than or equal to 14deg
            if angle >= OBLIQUE_THRESHOLD {
                // oblique values greater than or equal to angle are checked in
                // ascending order
                if let Some(found) = oblique_fonts
                    .clone()
                    .filter(|(_, a)| *a >= angle)
                    .min_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(Less))
                {
                    use_style = found.0;
                }
                // followed by positive oblique values below angle in descending order
                else if let Some(found) = oblique_fonts
                    .clone()
                    .filter(|(_, a)| *a > 0. && *a < angle)
                    .max_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(Less))
                {
                    use_style = found.0;
                } else {
                    // If font-synthesis-style has the value auto, then for variable
                    // fonts with a slnt axis a match is created by setting the slnt
                    // value with the specified oblique value; otherwise, a fallback
                    // match is produced by geometric shearing to the specified
                    // oblique value.
                    if synthesize_style {
                        if set.iter().any(|f| f.has_slnt) {
                            _use_slnt = true;
                        } else {
                            use_style = if set.iter().any(|f| f.style == FontStyle::Normal) {
                                FontStyle::Normal
                            } else {
                                set[0].style
                            };
                        }
                    } else {
                        // Choose an italic style
                        if set.iter().any(|f| f.style == FontStyle::Italic) {
                            use_style = FontStyle::Italic;
                        }
                        // oblique values less than or equal to 0deg are checked in descending order
                        else if let Some(found) = oblique_fonts
                            .clone()
                            .filter(|(_, a)| *a <= 0.)
                            .max_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(Less))
                        {
                            use_style = found.0;
                        } else {
                            use_style = set[0].style;
                        }
                    }
                }
            }
            // if the requested angle is greater than or equal to 0deg
            // and less than 14deg
            else if angle >= 0. {
                // positive oblique values below angle in descending order
                if let Some(found) = oblique_fonts
                    .clone()
                    .filter(|(_, a)| *a > 0. && *a < angle)
                    .max_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(Less))
                {
                    use_style = found.0;
                }
                // followed by oblique values greater than or equal to angle in
                // ascending order
                else if let Some(found) = oblique_fonts
                    .clone()
                    .filter(|(_, a)| *a >= angle)
                    .min_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(Less))
                {
                    use_style = found.0;
                } else {
                    // If font-synthesis-style has the value auto, then for variable
                    // fonts with a slnt axis a match is created by setting the slnt
                    // value with the specified oblique value; otherwise, a fallback
                    // match is produced by geometric shearing to the specified
                    // oblique value.
                    if synthesize_style {
                        if set.iter().any(|f| f.has_slnt) {
                            _use_slnt = true;
                        } else {
                            use_style = if set.iter().any(|f| f.style == FontStyle::Normal) {
                                FontStyle::Normal
                            } else {
                                set[0].style
                            };
                        }
                    } else {
                        // Choose an italic style
                        if set.iter().any(|f| f.style == FontStyle::Italic) {
                            use_style = FontStyle::Italic;
                        }
                        // oblique values less than or equal to 0deg are checked in descending order
                        else if let Some(found) = oblique_fonts
                            .clone()
                            .filter(|(_, a)| *a <= 0.)
                            .max_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(Less))
                        {
                            use_style = found.0;
                        } else {
                            use_style = set[0].style;
                        }
                    }
                }
            }
            // -14deg < angle < 0deg
            else if angle > -OBLIQUE_THRESHOLD {
                // negative oblique values above angle in ascending order
                if let Some(found) = oblique_fonts
                    .clone()
                    .filter(|(_, a)| *a < 0. && *a > angle)
                    .min_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(Less))
                {
                    use_style = found.0;
                }
                // followed by oblique values less than or equal to angle in
                // descending order
                else if let Some(found) = oblique_fonts
                    .clone()
                    .filter(|(_, a)| *a <= angle)
                    .max_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(Less))
                {
                    use_style = found.0;
                } else {
                    // If font-synthesis-style has the value auto, then for variable
                    // fonts with a slnt axis a match is created by setting the slnt
                    // value with the specified oblique value; otherwise, a fallback
                    // match is produced by geometric shearing to the specified
                    // oblique value.
                    if synthesize_style {
                        if set.iter().any(|f| f.has_slnt) {
                            _use_slnt = true;
                        } else {
                            use_style = if set.iter().any(|f| f.style == FontStyle::Normal) {
                                FontStyle::Normal
                            } else {
                                set[0].style
                            };
                        }
                    } else {
                        // Choose an italic style
                        if set.iter().any(|f| f.style == FontStyle::Italic) {
                            use_style = FontStyle::Italic;
                        }
                        // oblique values greater than or equal to 0deg are checked in ascending order
                        else if let Some(found) = oblique_fonts
                            .clone()
                            .filter(|(_, a)| *a >= 0.)
                            .min_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(Less))
                        {
                            use_style = found.0;
                        } else {
                            use_style = set[0].style;
                        }
                    }
                }
            }
            // angle < -14 deg
            else {
                // oblique values less than or equal to angle are checked in
                // descending order
                if let Some(found) = oblique_fonts
                    .clone()
                    .filter(|(_, a)| *a <= angle)
                    .max_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(Less))
                {
                    use_style = found.0;
                }
                // followed by negative oblique values above angle in ascending order
                else if let Some(found) = oblique_fonts
                    .clone()
                    .filter(|(_, a)| *a < 0. && *a > angle)
                    .min_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(Less))
                {
                    use_style = found.0;
                } else {
                    // If font-synthesis-style has the value auto, then for variable
                    // fonts with a slnt axis a match is created by setting the slnt
                    // value with the specified oblique value; otherwise, a fallback
                    // match is produced by geometric shearing to the specified
                    // oblique value.
                    if synthesize_style {
                        if set.iter().any(|f| f.has_slnt) {
                            _use_slnt = true;
                        } else {
                            use_style = if set.iter().any(|f| f.style == FontStyle::Normal) {
                                FontStyle::Normal
                            } else {
                                set[0].style
                            };
                        }
                    } else {
                        // Choose an italic style
                        if set.iter().any(|f| f.style == FontStyle::Italic) {
                            use_style = FontStyle::Italic;
                        }
                        // oblique values greater than or equal to 0deg are checked in ascending order
                        else if let Some(found) = oblique_fonts
                            .clone()
                            .filter(|(_, a)| *a >= 0.)
                            .min_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(Less))
                        {
                            use_style = found.0;
                        } else {
                            use_style = set[0].style;
                        }
                    }
                }
            }
        }
        // If the value of font-style is normal...
        else {
            // oblique values greater than or equal to 0deg are checked in
            // ascending order
            if let Some(found) = oblique_fonts
                .clone()
                .filter(|(_, a)| *a >= 0.)
                .min_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(Less))
            {
                use_style = found.0;
            }
            // followed by italic fonts
            else if let Some(found) = set.iter().find(|f| f.style == FontStyle::Italic) {
                use_style = found.style;
            }
            // followed by oblique values less than 0deg in descending order
            else if let Some(found) = oblique_fonts
                .clone()
                .filter(|(_, a)| *a < 0.)
                .max_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(Less))
            {
                use_style = found.0;
            } else {
                use_style = set[0].style;
            }
        }
    }
    set.retain(|f| f.style == use_style);

    // font-weight is matched next:
    if let Some(index) = set.iter().position(|f| f.weight == weight) {
        Some(index)
    } else {
        // If the desired weight is inclusively between 400 and 500...
        if (400.0..=500.0).contains(&weight) {
            set
                // weights greater than or equal to the target weight are checked in ascending
                // order until 500 is hit and checked
                .min_weight_matching(|w| w >= weight && w <= 500.0)
                // followed by weights less than the target weight in descending order
                .or_else(|| set.max_weight_matching(|w| w < weight))
                // followed by weights greater than 500, until a match is found.
                .or_else(|| set.min_weight_matching(|w| w > 500.0))
        }
        // If the desired weight is less than 400...
        else if weight < 400.0 {
            // weights less than or equal to the target weight are checked in descending order
            set.max_weight_matching(|w| w < weight)
                // followed by weights greater than the target weight in ascending order
                .or_else(|| set.min_weight_matching(|w| w > weight))
        }
        // If the desired weight is greater than 500...
        else {
            // weights greater than or equal to the target weight are checked in ascending
            set.min_weight_matching(|w| w >= weight)
                // followed by weights less than the target weight in descending order
                .or_else(|| set.max_weight_matching(|w| w < weight))
        }
        .map(|found| found.index)
    }
}

fn oblique_style(style: FontStyle) -> Option<(FontStyle, f32)> {
    match style {
        FontStyle::Oblique(angle) => Some((style, angle.unwrap_or(DEFAULT_OBLIQUE_ANGLE))),
        _ => None,
    }
}
