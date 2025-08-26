use crate::{
    FontVariation,
    lru_cache::{LookupKey, LruCache},
};

/// A cache of `ShaperData` instances.
pub(crate) type ShapeDataCache = LruCache<u64, harfrust::ShaperData>;
pub(crate) struct ShapeDataKey {
    font_id: u64,
}

impl ShapeDataKey {
    pub(crate) fn new(font_id: u64) -> Self {
        Self { font_id }
    }
}

impl LookupKey<u64> for ShapeDataKey {
    fn eq(&self, other: &u64) -> bool {
        self.font_id == *other
    }
    fn to_id(self) -> u64 {
        self.font_id
    }
}

/// A cache of `ShaperInstance` instances.
pub(crate) type ShapeInstanceCache = LruCache<ShapeInstanceId, harfrust::ShaperInstance>;
type ShapeInstanceId = (u64, fontique::Synthesis, Option<Box<[FontVariation]>>);

pub(crate) struct ShapeInstanceKey<'a> {
    font_id: u64,
    synthesis: &'a fontique::Synthesis,
    variations: Option<&'a [FontVariation]>,
}

impl<'a> ShapeInstanceKey<'a> {
    pub(crate) fn new(
        font_id: u64,
        synthesis: &'a fontique::Synthesis,
        variations: Option<&'a [FontVariation]>,
    ) -> Self {
        Self {
            font_id,
            synthesis,
            variations,
        }
    }
}

impl<'a> LookupKey<ShapeInstanceId> for ShapeInstanceKey<'a> {
    fn eq(&self, other: &ShapeInstanceId) -> bool {
        self.font_id == other.0
            && *self.synthesis == other.1
            && self.variations == other.2.as_deref()
    }
    fn to_id(self) -> ShapeInstanceId {
        (
            self.font_id,
            self.synthesis.clone(),
            self.variations.map(|v| v.to_vec().into()),
        )
    }
}

/// A cache of `ShapePlan` instances.
pub(crate) type ShapePlanCache = LruCache<ShapePlanId, harfrust::ShapePlan>;
type ShapePlanId = (
    u64,
    harfrust::Direction,
    harfrust::Script,
    Option<harfrust::Language>,
    Box<[harfrust::Feature]>,
);

pub(crate) struct ShapePlanKey<'a> {
    font_id: u64,
    direction: harfrust::Direction,
    script: harfrust::Script,
    language: Option<harfrust::Language>,
    features: &'a [harfrust::Feature],
}

impl<'a> ShapePlanKey<'a> {
    pub(crate) fn new(
        font_id: u64,
        direction: harfrust::Direction,
        script: harfrust::Script,
        language: Option<harfrust::Language>,
        features: &'a [harfrust::Feature],
    ) -> Self {
        Self {
            font_id,
            direction,
            script,
            language,
            features,
        }
    }
}

impl<'a> LookupKey<ShapePlanId> for ShapePlanKey<'a> {
    fn eq(&self, other: &ShapePlanId) -> bool {
        self.font_id == other.0
            && self.direction == other.1
            && self.script == other.2
            && self.language == other.3
            && self.features.len() == other.4.len()
            && self
                .features
                .iter()
                .zip(other.4.iter())
                .all(|(a, b)| a == b)
    }
    fn to_id(self) -> ShapePlanId {
        (
            self.font_id,
            self.direction,
            self.script,
            self.language,
            self.features.to_vec().into(),
        )
    }
}
