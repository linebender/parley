use super::font::FontFamilyHandle;
use core::fmt::Debug;
use fount::{FamilyId, GenericFamily, Locale};
use swash::text::Script;
use swash::{Attributes, Stretch, Style, Weight};
use super::{Brush, Color};

#[derive(Clone, Debug)]
pub struct SpanData<F: Clone + PartialEq + Debug, B: Brush> {
    pub family: FontFamilyHandle<F>,
    pub size: f64,
    pub stretch: Stretch,
    pub style: Style,
    pub weight: Weight,
    pub color: Color<B>,
    pub underline: bool,
    pub strikethrough: bool,
    pub start: usize,
    pub end: usize,
    pub count: usize,
}

impl<F: Clone + PartialEq + Debug, B: Brush> SpanData<F, B> {
    pub fn apply(&mut self, attr: &AttributeKind<F, B>) -> bool {
        match attr {
            AttributeKind::Family(family) => self.family = family.clone(),
            AttributeKind::Color(color) => self.color = color.clone(),
            AttributeKind::Size(size) => self.size = *size,
            AttributeKind::Stretch(stretch) => self.stretch = *stretch,
            AttributeKind::Style(style) => self.style = *style,
            AttributeKind::Weight(weight) => self.weight = *weight,
            AttributeKind::Underline(yes) => self.underline = *yes,
            AttributeKind::Strikethrough(yes) => self.strikethrough = *yes,
        }
        false
    }

    pub fn check(&self, attr: &AttributeKind<F, B>) -> bool {
        match attr {
            AttributeKind::Family(family) => self.family == *family,
            AttributeKind::Size(size) => self.size == *size,
            AttributeKind::Stretch(stretch) => self.stretch == *stretch,
            AttributeKind::Style(style) => self.style == *style,
            AttributeKind::Weight(weight) => self.weight == *weight,
            AttributeKind::Color(color) => self.color == *color,
            AttributeKind::Underline(yes) => self.underline == *yes,
            AttributeKind::Strikethrough(yes) => self.strikethrough == *yes,
        }
    }

    pub fn can_merge(&self, other: &Self) -> bool {
        self.family == other.family
            && self.size == other.size
            && self.stretch == other.stretch
            && self.style == other.style
            && self.weight == other.weight
            && self.color == other.color
            && self.underline == other.underline
            && self.strikethrough == other.strikethrough
    }

    pub fn attributes(&self) -> Attributes {
        Attributes::new(self.stretch, self.weight, self.style)
    }
}

impl<F: Clone + PartialEq + Debug, B: Brush> Default for SpanData<F, B> {
    fn default() -> Self {
        Self {
            family: FontFamilyHandle::Default,
            size: 16.,
            stretch: Stretch::NORMAL,
            style: Style::Normal,
            weight: Weight::NORMAL,
            color: Color::Solid([0, 0, 0, 255]),
            underline: false,
            strikethrough: false,
            start: 0,
            end: 0,
            count: 0,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ItemData {
    pub script: Script,
    pub locale: Option<Locale>,
    pub size: f32,
    pub level: u8,
    pub start: usize,
    pub end: usize,
    pub count: usize,
}

#[derive(Clone, PartialEq)]
pub enum AttributeKind<F: Clone + PartialEq + Debug, B: Brush> {
    Family(FontFamilyHandle<F>),
    Style(Style),
    Weight(Weight),
    Stretch(Stretch),
    Size(f64),
    Color(Color<B>),
    Underline(bool),
    Strikethrough(bool),
}

#[derive(Clone)]
pub struct RangedAttribute<F: Clone + PartialEq + Debug, B: Brush> {
    pub attr: AttributeKind<F, B>,
    pub start: usize,
    pub end: usize,
}

pub fn normalize_spans<F: Clone + PartialEq + Debug, B: Brush>(
    attrs: &[RangedAttribute<F, B>],
    defaults: &SpanData<F, B>,
    spans: &mut Vec<SpanData<F, B>>,
) {
    spans.push(defaults.clone());
    for attr in attrs {
        if attr.start >= attr.end {
            continue;
        }
        let split_range = span_split_range(attr, &spans);
        let mut inserted = 0;
        if let Some(first) = split_range.first {
            let original_span = &mut spans[first];
            if !original_span.check(&attr.attr) {
                let mut new_span = original_span.clone();
                let original_end = original_span.end;
                original_span.end = attr.start;
                new_span.start = attr.start;
                new_span.apply(&attr.attr);
                if split_range.replace_len == 0 && split_range.last == Some(first) {
                    let mut new_end_span = original_span.clone();
                    new_end_span.start = attr.end;
                    new_end_span.end = original_end;
                    new_span.end = attr.end;
                    spans.splice(
                        first + 1..first + 1,
                        [new_span, new_end_span].iter().cloned(),
                    );
                    continue;
                } else {
                    spans.insert(first + 1, new_span);
                }
                inserted += 1;
            }
        }
        let replace_start = split_range.replace_start + inserted;
        let replace_end = replace_start + split_range.replace_len;
        for i in replace_start..replace_end {
            spans[i].apply(&attr.attr);
        }
        if let Some(mut last) = split_range.last {
            last += inserted;
            let original_span = &mut spans[last];
            if !original_span.check(&attr.attr) {
                let mut new_span = original_span.clone();
                original_span.start = attr.end;
                new_span.end = attr.end;
                new_span.apply(&attr.attr);
                spans.insert(last, new_span);
            }
        }
    }
    let mut prev_index = 0;
    let mut merged_count = 0;
    for i in 1..spans.len() {
        if spans[prev_index].can_merge(&spans[i]) {
            let end = spans[i].end;
            spans[prev_index].end = end;
            merged_count += 1;
        } else {
            prev_index += 1;
            if prev_index != i {
                let moved_span = spans[i].clone();
                spans[prev_index] = moved_span;
            }
        }
    }
    spans.truncate(spans.len() - merged_count);
}

pub fn itemize<F: Clone + PartialEq + Debug, B: Brush>(
    text: &str,
    spans: &mut [SpanData<F, B>],
    items: &mut Vec<ItemData>,
) {
    use swash::text::Codepoint as _;
    let mut span_index = 0;
    let mut span = &mut spans[0];
    let mut span_end = span.end;
    let mut cur_size = span.size;
    let mut size = cur_size;
    let mut cur_script = text
        .chars()
        .map(|ch| ch.script())
        .find(|&script| real_script(script))
        .unwrap_or(Script::Latin);
    let cur_level = 0;
    let mut start = 0;
    let mut end = 0;
    let mut count = 0;
    macro_rules! push_item {
        () => {
            if start != end {
                items.push(ItemData {
                    script: cur_script,
                    locale: None,
                    level: cur_level,
                    size: cur_size as f32,
                    start,
                    end,
                    count,
                });
            }
        };
    }
    for (i, ch) in text.char_indices() {
        if i >= span_end {
            span_index += 1;
            span = &mut spans[span_index];
            span_end = span.end;
            size = span.size;
        }
        span.count += 1;
        count += 1;
        let mut script = ch.script();
        if !real_script(script) {
            script = cur_script;
        }
        if cur_size != size || script != cur_script {
            push_item!();
            start = end;
            count = 0;
        }
        cur_script = script;
        cur_size = size;
        end += ch.len_utf8();
    }
    end = text.len();
    push_item!();
}

fn real_script(script: Script) -> bool {
    script != Script::Common && script != Script::Unknown && script != Script::Inherited
}

#[derive(Default)]
struct SpanSplitRange {
    first: Option<usize>,
    replace_start: usize,
    replace_len: usize,
    last: Option<usize>,
}

fn span_split_range<F: Clone + PartialEq + Debug, B: Brush>(
    attr: &RangedAttribute<F, B>,
    spans: &[SpanData<F, B>],
) -> SpanSplitRange {
    let mut range = SpanSplitRange::default();
    let start_span_index = match spans.binary_search_by(|span| span.start.cmp(&attr.start)) {
        Ok(index) => index,
        Err(index) => index.saturating_sub(1),
    };
    let mut end_span_index = spans.len() - 1;
    for (i, span) in spans[start_span_index..].iter().enumerate() {
        if span.end >= attr.end {
            end_span_index = i + start_span_index;
            break;
        }
    }
    let start_span = &spans[start_span_index];
    let end_span = &spans[end_span_index];
    if start_span.start < attr.start {
        range.first = Some(start_span_index);
        range.replace_start = start_span_index + 1;
    } else {
        range.replace_start = start_span_index;
    }
    if end_span.end > attr.end {
        range.last = Some(end_span_index);
        range.replace_len = end_span_index.saturating_sub(range.replace_start);
    } else {
        range.replace_len = (end_span_index + 1).saturating_sub(range.replace_start);
    }
    range
}
