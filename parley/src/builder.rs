// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Context for layout.

use super::FontContext;
use super::context::LayoutContext;
use super::style::{Brush, StyleProperty, TextStyle};

use super::layout::Layout;

use alloc::string::String;
use core::ops::RangeBounds;
use parlance::BaseDirection;
use parley_engine::break_overrides::LineBreakOverrideFn;
use parley_engine::{AnalysisDataSources, DictionaryProvider, InlineObject};

use crate::inline_box::{FloatedBox, InlineBox, LayoutInlineBox};
use crate::resolve::{ResolvedStyle, StyleRun};
use crate::util::resolve_range;

#[derive(Clone, Copy)]
pub(crate) struct BuilderOptions<'a> {
    scale: f32,
    quantize: bool,
    base_direction: BaseDirection,
    line_break_override: Option<&'a LineBreakOverrideFn>,
    dictionary_provider: Option<&'a dyn DictionaryProvider>,
}

impl BuilderOptions<'_> {
    pub(crate) fn new(scale: f32, quantize: bool) -> Self {
        Self {
            scale,
            quantize,
            base_direction: BaseDirection::Auto,
            line_break_override: None,
            dictionary_provider: None,
        }
    }
}

/// Builder for constructing a text layout with ranged attributes.
///
/// This builder does not collapse whitespace. For whitespace collapsing, use the [`TreeBuilder`]
#[must_use]
pub struct RangedBuilder<'a, B: Brush> {
    pub(crate) options: BuilderOptions<'a>,
    pub(crate) lcx: &'a mut LayoutContext<B>,
    pub(crate) fcx: &'a mut FontContext,
}

impl<'b, B: Brush> RangedBuilder<'b, B> {
    pub fn push_default<'a>(&mut self, property: impl Into<StyleProperty<'a, B>>) {
        let resolved =
            self.lcx
                .rcx
                .resolve_property(self.fcx, &property.into(), self.options.scale);
        self.lcx.ranged_style_builder.push_default(resolved);
    }

    pub fn push<'a>(
        &mut self,
        property: impl Into<StyleProperty<'a, B>>,
        range: impl RangeBounds<usize>,
    ) {
        let resolved =
            self.lcx
                .rcx
                .resolve_property(self.fcx, &property.into(), self.options.scale);
        self.lcx.ranged_style_builder.push(resolved, range);
    }

    /// Forces a shaping boundary at `byte_index`.
    ///
    /// Arabic joining forms will be maintained across the boundary (`اَلْفُصْ‍|‍حَىٰ`
    /// rather than `اَلْفُصْ|حَىٰ` for a boundary in اَلْفُصْ‍حَىٰ).
    /// However, other shaping features, including but not limited to ligatures
    /// and kerning, will not reach across the boundary.
    ///
    /// Currently, this also introduces a grapheme boundary.
    /// Splits may be pushed in any order; duplicates are currently ignored.
    ///
    /// # Panics
    ///
    /// Building the layout panics if `byte_index` is not a UTF-8 character boundary of the text.
    pub fn push_shaping_split(&mut self, byte_index: usize) {
        self.lcx.shaping_splits.push(byte_index);
    }

    /// Adds an inline box.
    ///
    /// The inline box must have a matching U+FFFC OBJECT REPLACEMENT CHARACTER at its
    /// [`index`](InlineBox::index) in the text.
    ///
    /// # Panics
    ///
    /// Building the layout panics if the inline box's index isn't that of a U+FFFC character in the
    /// text, or if two inline boxes share a single U+FFFC character.
    pub fn push_inline_box(&mut self, inline_box: InlineBox) {
        self.lcx
            .inline_boxes
            .push(LayoutInlineBox::new(inline_box, 0));
    }

    /// Adds a floated box.
    pub fn push_floated_box(&mut self, floated_box: FloatedBox) {
        self.lcx.floated_boxes.push(floated_box);
    }

    /// Sets the paragraph's base direction.
    ///
    /// The default is [`BaseDirection::Auto`], which infers the direction from the text.
    pub fn set_base_direction(&mut self, base_direction: BaseDirection) {
        self.options.base_direction = base_direction;
    }

    /// Set the callback which will be called as a first provider of line breaking decisions.
    ///
    /// See [`LineBreakOverrideFn`] for more details.
    pub fn set_line_break_override(&mut self, overrides: Option<&'b LineBreakOverrideFn>) {
        self.options.line_break_override = overrides;
    }

    /// Set the provider for dictionary data used for word and line segmentation.
    ///
    /// See [`DictionaryProvider`] for more details.
    ///
    /// # Panics
    ///
    /// Building this layout may panic if the provider returns an error other than `IdentifierNotFound`.
    pub fn set_dictionary_provider(&mut self, provider: Option<&'b dyn DictionaryProvider>) {
        self.options.dictionary_provider = provider;
    }

    pub fn build_into(self, layout: &mut Layout<B>, text: impl AsRef<str>) {
        // Apply RangedStyleBuilder styles directly to style-table/style-run state.
        self.lcx
            .ranged_style_builder
            .finish(&mut self.lcx.style_table, &mut self.lcx.style_runs);

        // Call generic layout builder method
        build_into_layout(layout, text.as_ref(), self.lcx, self.fcx, self.options);
    }

    pub fn build(self, text: impl AsRef<str>) -> Layout<B> {
        let mut layout = Layout::default();
        self.build_into(&mut layout, text);
        layout
    }
}

/// Builder for constructing a text layout from a style table and
/// indexed style runs.
///
/// This builder does not collapse whitespace. For whitespace collapsing, use the [`TreeBuilder`]
#[must_use]
pub struct StyleRunBuilder<'a, B: Brush> {
    pub(crate) options: BuilderOptions<'a>,
    pub(crate) lcx: &'a mut LayoutContext<B>,
    pub(crate) fcx: &'a mut FontContext,
    pub(crate) cursor: usize,
}

impl<'b, B: Brush> StyleRunBuilder<'b, B> {
    /// Reserves additional capacity for styles and runs.
    ///
    /// This is an optional optimization for callers that know counts
    /// up front; call it before pushing styles and runs to reduce
    /// reallocations.
    pub fn reserve(&mut self, additional_styles: usize, additional_runs: usize) {
        self.lcx.style_table.reserve(additional_styles);
        self.lcx.style_runs.reserve(additional_runs);
    }

    /// Adds a style to the shared style table and returns its index.
    ///
    /// The first style pushed (index `0`) is the root style. Lines are at least as tall as its line
    /// height, and its [`TextStyle::vertical_align`] is ignored. Later styles are nested in
    /// `parent`, which their `vertical_align` resolves against. If `parent` is `None`, the style is
    /// nested in the root.
    ///
    /// # Panics
    ///
    /// Panics if `parent` is `Some(i)` where `i` is not the index of a previously pushed style,
    /// or if more than `u16::MAX` styles have been pushed.
    pub fn push_style<'family, 'settings>(
        &mut self,
        style: TextStyle<'family, 'settings, B>,
        parent: Option<u16>,
    ) -> u16 {
        let mut resolved =
            self.lcx
                .rcx
                .resolve_entire_style_set(self.fcx, &style, self.options.scale);
        if let Some(parent_style_index) = parent {
            assert!(
                usize::from(parent_style_index) < self.lcx.style_table.len(),
                "StyleRunBuilder expects a parent style that was previously added"
            );
            resolved.parent = parent_style_index;
        }
        let style_index = self.lcx.style_table.len();
        assert!(style_index <= u16::MAX as usize, "too many styles");
        self.lcx.style_table.push(resolved);
        style_index as u16
    }

    /// Adds a style derived from a previously pushed style `base_style_index` by applying
    /// `properties`, and returns its index.
    ///
    /// The new style starts as a copy of `base_style_index` (including its
    /// [`TextStyle::vertical_align`]) and is nested in `parent`, which its `vertical_align`
    /// resolves against (matching [`Self::push_style`]: `None` nests it in the root, `Some(i)`
    /// nests it in style `i`). Pass `Some(base_style_index)` to nest inside `base_style_index`
    /// (override [`StyleProperty::VerticalAlign`] to [`crate::VerticalAlign::BASELINE`] if you do
    /// not want `base_style_index`'s shift to apply again), or pass `base_style_index`'s own parent
    /// to create a sibling span (for example, a wrappable span for a collapsed space following a
    /// `nowrap` span).
    ///
    /// If `parent` matches `base_style_index`'s parent and none of `properties` differ from
    /// `base_style_index`, `base_style_index` is returned without adding a duplicate entry.
    ///
    /// # Panics
    ///
    /// Panics if `base_style_index` or `parent` (when `Some`) is not the index of a previously
    /// pushed style, or if more than `u16::MAX` styles have been pushed.
    pub fn push_modified_style<'a>(
        &mut self,
        base_style_index: u16,
        parent: Option<u16>,
        properties: impl IntoIterator<Item = StyleProperty<'a, B>>,
    ) -> u16 {
        assert!(
            usize::from(base_style_index) < self.lcx.style_table.len(),
            "StyleRunBuilder expects a base style that was previously added"
        );
        let parent_style_index = match parent {
            Some(parent_style_index) => {
                assert!(
                    usize::from(parent_style_index) < self.lcx.style_table.len(),
                    "StyleRunBuilder expects a parent style that was previously added"
                );
                parent_style_index
            }
            None => 0,
        };
        let mut modified =
            if self.lcx.style_table[usize::from(base_style_index)].parent != parent_style_index {
                let mut cloned = self.lcx.style_table[usize::from(base_style_index)].clone();
                cloned.parent = parent_style_index;
                Some(cloned)
            } else {
                None
            };
        for property in properties {
            let resolved_prop =
                self.lcx
                    .rcx
                    .resolve_property(self.fcx, &property, self.options.scale);
            let current = modified
                .as_ref()
                .unwrap_or(&self.lcx.style_table[usize::from(base_style_index)]);
            if !current.check(&resolved_prop) {
                modified
                    .get_or_insert_with(|| {
                        self.lcx.style_table[usize::from(base_style_index)].clone()
                    })
                    .apply(resolved_prop);
            }
        }
        let Some(resolved) = modified else {
            return base_style_index;
        };
        let style_index = self.lcx.style_table.len();
        assert!(style_index <= u16::MAX as usize, "too many styles");
        self.lcx.style_table.push(resolved);
        style_index as u16
    }

    /// Adds a style run referencing an entry from the style table.
    ///
    /// Runs must be contiguous and non-overlapping, and must cover
    /// `0..text.len()` once all runs have been added.
    pub fn push_style_run(&mut self, style_index: u16, range: impl RangeBounds<usize>) {
        let range = resolve_range(range, usize::MAX);
        assert!(
            range.start == self.cursor,
            "StyleRunBuilder expects contiguous non-overlapping runs"
        );
        assert!(
            range.start <= range.end,
            "StyleRunBuilder expects ordered ranges"
        );
        assert!(
            (style_index as usize) < self.lcx.style_table.len(),
            "StyleRunBuilder expects style indices that were previously added via push_style"
        );
        self.cursor = range.end;
        self.lcx.style_runs.push(StyleRun {
            style_index,
            range: range.into(),
        });
    }

    /// Forces a shaping boundary at `byte_index`.
    ///
    /// Arabic joining forms will be maintained across the boundary (`اَلْفُصْ‍|‍حَىٰ`
    /// rather than `اَلْفُصْ|حَىٰ` for a boundary in اَلْفُصْ‍حَىٰ).
    /// However, other shaping features, including but not limited to ligatures
    /// and kerning, will not reach across the boundary.
    ///
    /// Currently, this also introduces a grapheme boundary.
    /// Splits may be pushed in any order; duplicates are currently ignored.
    ///
    /// # Panics
    ///
    /// Building the layout panics if `byte_index` is not a UTF-8 character boundary of the text.
    pub fn push_shaping_split(&mut self, byte_index: usize) {
        self.lcx.shaping_splits.push(byte_index);
    }

    /// Adds an inline box.
    ///
    /// The inline box must have a matching U+FFFC OBJECT REPLACEMENT CHARACTER at its
    /// [`index`](InlineBox::index) in the text.
    ///
    /// # Panics
    ///
    /// Building the layout panics if the inline box's index isn't that of a U+FFFC character in the
    /// text, or if two inline boxes share a single U+FFFC character.
    pub fn push_inline_box(&mut self, inline_box: InlineBox) {
        self.lcx
            .inline_boxes
            .push(LayoutInlineBox::new(inline_box, 0));
    }

    /// Adds a floated box.
    pub fn push_floated_box(&mut self, floated_box: FloatedBox) {
        self.lcx.floated_boxes.push(floated_box);
    }

    /// Sets the paragraph's base direction.
    ///
    /// The default is [`BaseDirection::Auto`], which infers the direction from the text.
    pub fn set_base_direction(&mut self, base_direction: BaseDirection) {
        self.options.base_direction = base_direction;
    }

    /// Set the callback which will be called as a first provider of line breaking decisions.
    ///
    /// See [`LineBreakOverrideFn`] for more details.
    pub fn set_line_break_override(&mut self, overrides: Option<&'b LineBreakOverrideFn>) {
        self.options.line_break_override = overrides;
    }

    /// Set the provider for dictionary data used for word and line segmentation.
    ///
    /// See [`DictionaryProvider`] for more details.
    ///
    /// # Panics
    ///
    /// Building this layout may panic if the provider returns an error other than `IdentifierNotFound`.
    pub fn set_dictionary_provider(&mut self, provider: Option<&'b dyn DictionaryProvider>) {
        self.options.dictionary_provider = provider;
    }

    pub fn build_into(mut self, layout: &mut Layout<B>, text: impl AsRef<str>) {
        let text = text.as_ref();
        if self.cursor == usize::MAX {
            let last = self.lcx.style_runs.last_mut().unwrap();
            assert!(
                last.range.start <= text.len(),
                "StyleRunBuilder requires runs that cover the full text"
            );
            last.range.end = text.len();
            self.cursor = text.len();
        }
        assert!(
            self.cursor == text.len(),
            "StyleRunBuilder requires runs that cover the full text"
        );
        if self.lcx.style_runs.is_empty() {
            // This is reachable for empty text. In this case, the layout still needs a style and an
            // empty style run, e.g. to size a cursor.
            if self.lcx.style_table.is_empty() {
                let style = self.lcx.rcx.resolve_entire_style_set(
                    self.fcx,
                    &TextStyle::default(),
                    self.options.scale,
                );
                self.lcx.style_table.push(style);
            }
            self.lcx.style_runs.push(StyleRun {
                style_index: 0,
                range: (0..0).into(),
            });
        }
        build_into_layout(layout, text, self.lcx, self.fcx, self.options);
    }

    pub fn build(self, text: impl AsRef<str>) -> Layout<B> {
        let mut layout = Layout::default();
        self.build_into(&mut layout, text);
        layout
    }
}

/// Builder for constructing a text layout with a tree of attributes.
#[must_use]
pub struct TreeBuilder<'a, B: Brush> {
    pub(crate) options: BuilderOptions<'a>,
    pub(crate) lcx: &'a mut LayoutContext<B>,
    pub(crate) fcx: &'a mut FontContext,
}

impl<'b, B: Brush> TreeBuilder<'b, B> {
    pub fn push_style_span(&mut self, style: TextStyle<'_, '_, B>) {
        let resolved = self
            .lcx
            .rcx
            .resolve_entire_style_set(self.fcx, &style, self.options.scale);
        self.lcx.tree_style_builder.push_style_span(resolved);
    }

    pub fn push_style_modification_span<'s, 'iter>(
        &mut self,
        properties: impl IntoIterator<Item = &'iter StyleProperty<'s, B>>,
    ) where
        's: 'iter,
        B: 'iter,
    {
        self.lcx
            .tree_style_builder
            .push_style_modification_span(properties.into_iter().map(|p| {
                self.lcx
                    .rcx
                    .resolve_property(self.fcx, p, self.options.scale)
            }));
    }

    pub fn pop_style_span(&mut self) {
        self.lcx.tree_style_builder.pop_style_span();
    }

    pub fn push_text(&mut self, text: &str) {
        self.lcx
            .tree_style_builder
            .push_text(text, &mut self.lcx.shaping_splits);
    }

    /// Forces a shaping boundary before any subsequent text.
    ///
    /// Arabic joining forms will be maintained across the boundary (`اَلْفُصْ‍|‍حَىٰ`
    /// rather than `اَلْفُصْ|حَىٰ` for a boundary in اَلْفُصْ‍حَىٰ).
    /// However, other shaping features, including but not limited to ligatures
    /// and kerning, will not reach across the boundary.
    ///
    /// If there is pending collapsible whitespace (see [`Self::has_pending_whitespace`]), the split
    /// is placed between the resulting space and the subsequent text (if any).
    pub fn push_shaping_split(&mut self) {
        self.lcx
            .tree_style_builder
            .push_shaping_split(&mut self.lcx.shaping_splits);
    }

    /// Returns the pushed text so far, after whitespace processing, but excluding any trailing
    /// collapsible whitespace.
    ///
    /// An inline box appears in the text as a U+FFFC OBJECT REPLACEMENT CHARACTER (see
    /// [`Self::push_inline_box`]).
    ///
    /// This is a prefix of the text that [`Self::build`] will return. Whether that trailing
    /// collapsible whitespace ends up being kept (as a single space) depends on where it is and on
    /// what follows it. See [`Self::has_pending_whitespace`].
    pub fn text(&self) -> &str {
        self.lcx.tree_style_builder.text()
    }

    /// Returns whether [`Self::text`] is followed by collapsible whitespace.
    ///
    /// That white space is not yet part of [`Self::text`]: depending on where it is and what
    /// follows, it is either removed or kept as a single space.
    pub fn has_pending_whitespace(&self) -> bool {
        self.lcx.tree_style_builder.has_pending_whitespace()
    }

    /// Pushes an inline box at the current position in the text.
    ///
    /// Note: this overwrites the box's [`InlineBox::index`].
    ///
    /// This appends a `U+FFFC` OBJECT REPLACEMENT CHARACTER to the text that [`Self::build`] will
    /// return.
    ///
    /// [atomic inline]: https://www.w3.org/TR/css-display-3/#atomic-inline
    pub fn push_inline_box(&mut self, mut inline_box: InlineBox) {
        inline_box.index = self
            .lcx
            .tree_style_builder
            .push_inline_object(&mut self.lcx.shaping_splits);
        let parent_style_index = self.lcx.tree_style_builder.resolve_current_style_id();
        self.lcx
            .inline_boxes
            .push(LayoutInlineBox::new(inline_box, parent_style_index));
    }

    /// Pushes a floated box at the current position in the text.
    ///
    /// Note: this overwrites the box's [`FloatedBox::index`].
    pub fn push_floated_box(&mut self, mut floated_box: FloatedBox) {
        floated_box.index = self.lcx.tree_style_builder.text().len();
        self.lcx.floated_boxes.push(floated_box);
    }

    /// Sets the paragraph's base direction.
    ///
    /// The default is [`BaseDirection::Auto`], which infers the direction from the text.
    pub fn set_base_direction(&mut self, base_direction: BaseDirection) {
        self.options.base_direction = base_direction;
    }

    /// Set the callback which will be called as a first provider of line breaking decisions.
    ///
    /// See [`LineBreakOverrideFn`] for more details.
    pub fn set_line_break_override(&mut self, overrides: Option<&'b LineBreakOverrideFn>) {
        self.options.line_break_override = overrides;
    }

    /// Set the provider for dictionary data used for word and line segmentation.
    ///
    /// See [`DictionaryProvider`] for more details.
    ///
    /// # Panics
    ///
    /// Building this layout may panic if the provider returns an error other than `IdentifierNotFound`.
    pub fn set_dictionary_provider(&mut self, provider: Option<&'b dyn DictionaryProvider>) {
        self.options.dictionary_provider = provider;
    }

    #[inline]
    pub fn build_into(self, layout: &mut Layout<B>) -> String {
        // Apply TreeStyleBuilder styles to LayoutContext.
        let text = self
            .lcx
            .tree_style_builder
            .finish(&mut self.lcx.style_table, &mut self.lcx.style_runs);

        // Call generic layout builder method
        build_into_layout(layout, &text, self.lcx, self.fcx, self.options);

        text
    }

    #[inline]
    pub fn build(self) -> (Layout<B>, String) {
        let mut layout = Layout::default();
        let text = self.build_into(&mut layout);
        (layout, text)
    }
}

fn build_into_layout<B: Brush>(
    layout: &mut Layout<B>,
    text: &str,
    lcx: &mut LayoutContext<B>,
    fcx: &mut FontContext,
    options: BuilderOptions<'_>,
) {
    if text.is_empty() && lcx.style_runs.is_empty() {
        lcx.style_table.push(ResolvedStyle::default());
        lcx.style_runs.push(StyleRun {
            style_index: 0,
            range: (0..0).into(),
        });
    }
    assert!(
        !lcx.style_runs.is_empty(),
        "at least one style run is required"
    );

    // Sort the inline and floated boxes as subsequent code assumes that they are in text index
    // order. Note: It's important that the floated boxes use a stable sort to allow users to
    // control the order of contiguous floated boxes
    //
    // TODO: consider dropping the sort and requiring `push_inline_box` and `push_floated_box`
    // callers to push boxes in text index order (as `TreeBuilder` already does).
    lcx.inline_boxes
        .sort_unstable_by_key(|b| b.inline_box.index);
    lcx.floated_boxes.sort_by_key(|b| b.index);

    lcx.inline_objects.clear();
    for inline_box in &mut lcx.inline_boxes {
        let index = inline_box.inline_box.index;
        assert!(
            text.get(index..)
                .is_some_and(|rest| rest.starts_with('\u{FFFC}')),
            "in-flow inline box {} must be at a U+FFFC OBJECT REPLACEMENT CHARACTER in the text, \
             but its index is {index}",
            inline_box.inline_box.id,
        );
        assert!(
            lcx.inline_objects
                .last()
                .map(|object| object.text_byte_start)
                != Some(index),
            "in-flow inline boxes must each have a U+FFFC of their own, but more than one is at \
             index {index}",
        );
        lcx.inline_objects.push(InlineObject {
            text_byte_start: index,

            // TODO: We may want to follow CSS atomic inline's break behavior more closely.
            // See <https://www.w3.org/TR/css-text-4/#atomic-compat-wrap>.
            break_before: Some(true),
            break_after: Some(true),
        });

        // The box's parent is the span its U+FFFC character is in.
        let run = lcx.style_runs.partition_point(|run| run.range.end <= index);
        inline_box.parent_style_index = lcx.style_runs[run].style_index;
    }

    // Sort as subsequent code assumes that they are in order.
    // Same TODO as for inline boxes.
    lcx.shaping_splits.sort_unstable();
    lcx.shaping_splits.dedup();
    lcx.shaping_splits.retain(|&index| {
        assert!(
            text.is_char_boundary(index),
            "shaping split index {index} must be a UTF-8 character boundary within the text"
        );
        index != 0 && index != text.len()
    });

    let data_sources = AnalysisDataSources::new(options.dictionary_provider);

    crate::analysis::analyze_text(
        lcx,
        text,
        options.base_direction,
        options.line_break_override,
        &data_sources,
    );

    layout.data.clear();
    layout.data.scale = options.scale;
    layout.data.quantize = options.quantize;
    layout.data.base_level = lcx.analysis.paragraph_level();
    layout.data.text.push_str(text);

    lcx.char_style_indices.clear();
    lcx.char_style_indices
        .resize(lcx.analysis.char_info().len(), 0);
    let mut char_index = 0;
    for style_run in &lcx.style_runs {
        for _ in text[style_run.range].chars() {
            lcx.char_style_indices[char_index] = style_run.style_index;
            char_index += 1;
        }
    }

    // Copy the visual styles into the layout
    layout
        .data
        .styles
        .extend(lcx.style_table.iter().map(|s| s.as_layout_style()));

    {
        super::shape::shape_text(
            &lcx.rcx,
            fcx,
            &lcx.style_table,
            &lcx.floated_boxes,
            &lcx.shaping_splits,
            &lcx.analysis,
            &lcx.char_style_indices,
            &mut lcx.scx,
            text,
            layout,
            &mut lcx.style_features,
        );
    }

    crate::layout::style_metrics::resolve_style_metrics(
        &lcx.rcx,
        fcx,
        &lcx.style_table,
        &mut lcx.scx,
        options.quantize,
        &mut layout.data.style_metrics,
    );

    // Move inline boxes into the layout
    layout.data.inline_boxes.clear();
    core::mem::swap(&mut layout.data.inline_boxes, &mut lcx.inline_boxes);
    layout.data.floated_boxes.clear();
    core::mem::swap(&mut layout.data.floated_boxes, &mut lcx.floated_boxes);

    // Find the shaped clusters of the inline objects, and place their boxes there.
    //
    // TODO: Maybe `parley_engine` should expose a utility to easily get indices of shaped clusters?
    for box_index in 0..layout.data.inline_boxes.len() {
        let layout_box = &layout.data.inline_boxes[box_index];
        let index = layout_box.inline_box.index;
        let characters = layout.data.shaped_text.characters();
        let clusters = layout.data.shaped_text.shaped_clusters();
        let char_index =
            characters.partition_point(|c| (c.text_byte_start as usize) < index) as u32;
        let cluster = clusters.partition_point(|c| c.chars_range().start < char_index);
        // Each object is a cluster of its own, but there is none only if shaping was aborted due to
        // lack of fonts.
        layout.data.inline_boxes[box_index].inline_object_cluster =
            (cluster < clusters.len()).then_some(cluster as u32);
        layout.data.place_inline_object(box_index);
    }
}
