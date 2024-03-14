use fontique::{Collection, SourceCache};

#[derive(Default)]
pub struct FontContext {
    pub collection: Collection,
    pub source_cache: SourceCache,
}
