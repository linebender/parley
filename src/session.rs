use super::font::*;

pub struct LayoutSession<'a, C: FontCollection> {
    fonts: &'a C,
}
