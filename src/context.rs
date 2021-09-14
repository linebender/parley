//! Context for layout.

use super::bidi;
use super::font::FontContext;
use super::layout::Layout;
use super::resolve::range::*;
use super::resolve::*;
use super::style::*;

use swash::shape::ShapeContext;
use swash::text::cluster::CharInfo;

use std::cell::{RefCell, RefMut};
use std::ops::{Deref, DerefMut, RangeBounds};
use std::rc::Rc;

/// Context for building a text layout.
pub struct LayoutContext<B: Brush = [u8; 4]> {
    bidi: bidi::BidiResolver,
    rcx: ResolveContext,
    styles: Vec<RangedStyle<B>>,
    rsb: RangedStyleBuilder<B>,
    info: Vec<(CharInfo, u16)>,
    scx: ShapeContext,
}

impl<B: Brush> LayoutContext<B> {
    pub fn new() -> Self {
        Self {
            bidi: bidi::BidiResolver::new(),
            rcx: ResolveContext::default(),
            styles: vec![],
            rsb: RangedStyleBuilder::default(),
            info: vec![],
            scx: ShapeContext::default(),
        }
    }

    pub fn ranged_builder<'a>(
        &'a mut self,
        fcx: &'a mut FontContext,
        text: &'a str,
    ) -> RangedLayoutBuilder<B, &'a str> {
        self.begin(text);
        fcx.cache.reset();
        RangedLayoutBuilder {
            text,
            lcx: MaybeShared::Borrowed(self),
            fcx: MaybeShared::Borrowed(fcx),
        }
    }

    fn begin(&mut self, text: &str) {
        self.rcx.clear();
        self.styles.clear();
        self.rsb.begin(text.len());
        self.info.clear();
        self.bidi.clear();
        let mut a = swash::text::analyze(text.chars());
        for x in a.by_ref() {
            self.info.push((CharInfo::new(x.0, x.1), 0));
        }
        if a.needs_bidi_resolution() {
            self.bidi.resolve(
                text.chars()
                    .zip(self.info.iter().map(|info| info.0.bidi_class())),
                None,
            );
        }
    }
}

#[doc(hidden)]
pub struct RcLayoutContext<B: Brush> {
    lcx: Rc<RefCell<LayoutContext<B>>>,
}

impl<B: Brush> RcLayoutContext<B> {
    pub fn new() -> Self {
        Self {
            lcx: Rc::new(RefCell::new(LayoutContext::new())),
        }
    }

    pub fn ranged_builder<T: TextSource>(
        &mut self,
        fcx: Rc<RefCell<FontContext>>,
        text: T,
    ) -> RangedLayoutBuilder<'static, B, T> {
        self.lcx.borrow_mut().begin("");
        RangedLayoutBuilder {
            text,
            lcx: MaybeShared::Shared(self.lcx.clone()),
            fcx: MaybeShared::Shared(fcx),
        }
    }
}

/// Builder for constructing a text layout with ranged attributes.
pub struct RangedLayoutBuilder<'a, B: Brush, T: TextSource> {
    text: T,
    lcx: MaybeShared<'a, LayoutContext<B>>,
    fcx: MaybeShared<'a, FontContext>,
}

impl<'a, B: Brush, T: TextSource> RangedLayoutBuilder<'a, B, T> {
    pub fn push_default(&mut self, property: Property<B>) {
        let mut lcx = self.lcx.borrow_mut();
        let mut fcx = self.fcx.borrow_mut();
        let resolved = lcx.rcx.resolve(&mut fcx, &property);
        lcx.rsb.push_default(resolved);
    }

    pub fn push(&mut self, property: Property<B>, range: impl RangeBounds<usize>) {
        let mut lcx = self.lcx.borrow_mut();
        let mut fcx = self.fcx.borrow_mut();
        let resolved = lcx.rcx.resolve(&mut fcx, &property);
        lcx.rsb.push(resolved, range);
    }

    pub fn build_into(&mut self, layout: &mut Layout<B>) {
        layout.data.clear();
        let mut lcx = self.lcx.borrow_mut();
        let lcx = &mut *lcx;
        let text = self.text.as_str();
        layout.data.has_bidi = !lcx.bidi.levels().is_empty();
        layout.data.base_level = lcx.bidi.base_level();
        layout.data.text_len = text.len();
        let mut fcx = self.fcx.borrow_mut();
        lcx.rsb.finish(&mut lcx.styles);
        let mut char_index = 0;
        for (i, style) in lcx.styles.iter().enumerate() {
            for _ in text[style.range.clone()].chars() {
                lcx.info[char_index].1 = i as u16;
                char_index += 1;
            }
        }
        use super::layout::{Decoration, Style};
        fn conv_deco<B: Brush>(
            deco: &ResolvedDecoration<B>,
            default_brush: &B,
        ) -> Option<Decoration<B>> {
            if deco.enabled {
                Some(Decoration {
                    brush: deco.brush.clone().unwrap_or_else(|| default_brush.clone()),
                    offset: deco.offset,
                    size: deco.size,
                })
            } else {
                None
            }
        }
        layout.data.styles.extend(lcx.styles.iter().map(|s| {
            let s = &s.style;
            Style {
                brush: s.brush.clone(),
                underline: conv_deco(&s.underline, &s.brush),
                strikethrough: conv_deco(&s.strikethrough, &s.brush),
            }
        }));
        super::shape::shape_text(
            &lcx.rcx,
            &mut fcx,
            &lcx.styles,
            &lcx.info,
            lcx.bidi.levels(),
            &mut lcx.scx,
            self.text.as_str(),
            layout,
        );
    }

    pub fn build(&mut self) -> Layout<B> {
        let mut layout = Layout::default();
        self.build_into(&mut layout);
        layout
    }
}

#[doc(hidden)]
pub trait TextSource {
    fn as_str(&self) -> &str;
}

impl<'a> TextSource for &'a str {
    fn as_str(&self) -> &str {
        *self
    }
}

enum MaybeShared<'a, T> {
    Shared(Rc<RefCell<T>>),
    Borrowed(&'a mut T),
}

impl<'a, T> MaybeShared<'a, T> {
    pub fn borrow_mut(&mut self) -> BorrowMut<T> {
        match self {
            Self::Shared(shared) => BorrowMut::Shared(shared.borrow_mut()),
            Self::Borrowed(borrowed) => BorrowMut::Borrowed(borrowed),
        }
    }
}

enum BorrowMut<'a, T> {
    Shared(RefMut<'a, T>),
    Borrowed(&'a mut T),
}

impl<'a, T> Deref for BorrowMut<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Shared(shared) => shared.deref(),
            Self::Borrowed(borrowed) => borrowed,
        }
    }
}

impl<'a, T> DerefMut for BorrowMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Shared(shared) => shared.deref_mut(),
            Self::Borrowed(borrowed) => borrowed,
        }
    }
}
