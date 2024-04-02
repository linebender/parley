//! Implementation of the CSS font matching algorithm.

use super::attributes::{Stretch, Style, Weight};
use super::font::FontInfo;
use smallvec::SmallVec;

const DEFAULT_OBLIQUE_ANGLE: f32 = 14.0;

pub fn match_font(
    set: &[FontInfo],
    stretch: Stretch,
    style: Style,
    weight: Weight,
    synthesize_style: bool,
) -> Option<usize> {
    const OBLIQUE_THRESHOLD: f32 = DEFAULT_OBLIQUE_ANGLE;
    match set.len() {
        0 => return None,
        1 => return Some(0),
        _ => {}
    }
    #[derive(Copy, Clone)]
    struct Candidate {
        index: usize,
        stretch: i32,
        style: Style,
        weight: f32,
        has_slnt: bool,
    }
    let mut set: SmallVec<[Candidate; 16]> = set
        .iter()
        .enumerate()
        .map(|(i, font)| Candidate {
            index: i,
            stretch: (font.stretch().ratio() * 100.0) as i32,
            style: font.style(),
            weight: font.weight().value(),
            has_slnt: font.has_slant_axis(),
        })
        .collect();
    let stretch = (stretch.ratio() * 100.0) as i32;
    let weight = weight.value();
    // font-stretch is tried first:
    let mut use_stretch = set[0].stretch;
    if !set.iter().any(|f| f.stretch == stretch) {
        // If the desired stretch value is less than or equal to 100%...
        if stretch <= 100 {
            // stretch values below the desired stretch value are checked in
            // descending order...
            if let Some(found) = set
                .iter()
                .filter(|f| f.stretch < stretch)
                .max_by_key(|f| f.stretch)
            {
                use_stretch = found.stretch;
            }
            // followed by stretch values above the desired stretch value in
            // ascending order until a match is found.
            else if let Some(found) = set
                .iter()
                .filter(|f| f.stretch > stretch)
                .min_by_key(|f| f.stretch)
            {
                use_stretch = found.stretch;
            }
        }
        // Otherwise, ...
        else {
            // stretch values above the desired stretch value are checked in
            // ascending order...
            if let Some(found) = set
                .iter()
                .filter(|f| f.stretch > stretch)
                .min_by_key(|f| f.stretch)
            {
                use_stretch = found.stretch;
            }
            // followed by stretch values below the desired stretch value in
            // descending order until a match is found.
            else if let Some(found) = set
                .iter()
                .filter(|f| f.stretch < stretch)
                .max_by_key(|f| f.stretch)
            {
                use_stretch = found.stretch;
            }
        }
    } else {
        use_stretch = stretch;
    }
    set.retain(|f| f.stretch == use_stretch);
    use core::cmp::Ordering::*;
    let oblique_fonts = set.iter().filter_map(|f| oblique_style(f.style));
    // font-style is tried next:
    // NOTE: this code uses an oblique threshold of 14deg rather than
    // the current value of 20deg in the spec.
    // See: https://github.com/w3c/csswg-drafts/issues/2295
    let mut use_style = style;
    let mut _use_slnt = false;
    if !set.iter().any(|f| f.style == use_style) {
        // If the value of font-style is italic:
        if style == Style::Italic {
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
        else if let Some(angle) = oblique_angle(style) {
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
                            use_style = if set.iter().any(|f| f.style == Style::Normal) {
                                Style::Normal
                            } else {
                                set[0].style
                            };
                        }
                    } else {
                        // Choose an italic style
                        if set.iter().any(|f| f.style == Style::Italic) {
                            use_style = Style::Italic
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
                            use_style = if set.iter().any(|f| f.style == Style::Normal) {
                                Style::Normal
                            } else {
                                set[0].style
                            };
                        }
                    } else {
                        // Choose an italic style
                        if set.iter().any(|f| f.style == Style::Italic) {
                            use_style = Style::Italic
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
                            use_style = if set.iter().any(|f| f.style == Style::Normal) {
                                Style::Normal
                            } else {
                                set[0].style
                            };
                        }
                    } else {
                        // Choose an italic style
                        if set.iter().any(|f| f.style == Style::Italic) {
                            use_style = Style::Italic
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
                            use_style = if set.iter().any(|f| f.style == Style::Normal) {
                                Style::Normal
                            } else {
                                set[0].style
                            };
                        }
                    } else {
                        // Choose an italic style
                        if set.iter().any(|f| f.style == Style::Italic) {
                            use_style = Style::Italic
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
            else if let Some(found) = set.iter().find(|f| f.style == Style::Italic) {
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
    if let Some(f) = set.iter().find(|f| f.weight == weight) {
        return Some(f.index);
    } else {
        // If the desired weight is inclusively between 400 and 500...
        if (400.0..=500.0).contains(&weight) {
            // weights greater than or equal to the target weight are checked in ascending
            // order until 500 is hit and checked
            if let Some(found) = set
                .iter()
                .enumerate()
                .filter(|f| f.1.weight >= weight && f.1.weight <= 500.0)
                .min_by(|x, y| x.1.weight.partial_cmp(&y.1.weight).unwrap_or(Less))
            {
                return Some(found.1.index);
            }
            // followed by weights less than the target weight in descending order
            if let Some(found) = set
                .iter()
                .enumerate()
                .filter(|f| f.1.weight < weight)
                .max_by(|x, y| x.1.weight.partial_cmp(&y.1.weight).unwrap_or(Less))
            {
                return Some(found.1.index);
            }
            // followed by weights greater than 500, until a match is found.
            if let Some(found) = set
                .iter()
                .enumerate()
                .filter(|f| f.1.weight > 500.0)
                .min_by(|x, y| x.1.weight.partial_cmp(&y.1.weight).unwrap_or(Less))
            {
                return Some(found.1.index);
            }
        }
        // If the desired weight is less than 400...
        else if weight < 400.0 {
            // weights less than or equal to the target weight are checked in descending
            if let Some(found) = set
                .iter()
                .filter(|f| f.weight <= weight)
                .max_by(|x, y| x.weight.partial_cmp(&y.weight).unwrap_or(Less))
            {
                return Some(found.index);
            }
            // followed by weights greater than the target weight in ascending order
            if let Some(found) = set
                .iter()
                .filter(|f| f.weight > weight)
                .min_by(|x, y| x.weight.partial_cmp(&y.weight).unwrap_or(Less))
            {
                return Some(found.index);
            }
        }
        // If the desired weight is greater than 500...
        else {
            // weights greater than or equal to the target weight are checked in ascending
            if let Some(found) = set
                .iter()
                .filter(|f| f.weight >= weight)
                .min_by(|x, y| x.weight.partial_cmp(&y.weight).unwrap_or(Less))
            {
                return Some(found.index);
            }
            // followed by weights less than the target weight in descending order
            if let Some(found) = set
                .iter()
                .filter(|f| f.weight < weight)
                .max_by(|x, y| x.weight.partial_cmp(&y.weight).unwrap_or(Less))
            {
                return Some(found.index);
            }
        }
    }
    None
}

fn oblique_angle(style: Style) -> Option<f32> {
    match style {
        Style::Oblique(angle) => Some(angle.unwrap_or(DEFAULT_OBLIQUE_ANGLE)),
        _ => None,
    }
}

fn oblique_style(style: Style) -> Option<(Style, f32)> {
    match style {
        Style::Oblique(angle) => Some((style, angle.unwrap_or(DEFAULT_OBLIQUE_ANGLE))),
        _ => None,
    }
}
