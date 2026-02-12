// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Glyph bitmap cache with atlas storage and LRU eviction.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::{Debug, Formatter};
use hashbrown::HashMap;
use hashbrown::hash_map::RawEntryMut;
use smallvec::SmallVec;
pub use vello_common::image_cache::ImageCache;
pub use vello_common::multi_atlas::AtlasConfig;

use vello_common::paint::ImageId;

use super::commands::AtlasCommandRecorder;
use super::key::{GlyphCacheKey, SUBPIXEL_BUCKETS};
use super::region::{AtlasSlot, RasterMetrics};
use crate::Pixmap;
#[cfg(feature = "std")]
use hashbrown::HashSet;

/// Padding in pixels added to each side of a glyph to prevent texture bleeding.
pub const GLYPH_PADDING: u16 = 1;

/// Maximum age (in frames) before an unused entry is evicted.
const MAX_ENTRY_AGE: u32 = 64;

/// How often to run the eviction pass (in frames).
const EVICTION_FREQUENCY: u32 = 64;

/// Eviction is forced if cached count exceeds this threshold.
const CACHED_COUNT_THRESHOLD: usize = 512;

/// Maximum glyph dimension that will be cached.
/// Larger glyphs are rendered directly without caching.
pub const MAX_GLYPH_SIZE: u16 = 128;

// ---------------------------------------------------------------------------
// GlyphCache trait — the common interface used by shared rendering code
// ---------------------------------------------------------------------------

/// Common interface for glyph bitmap caches.
///
/// Both the CPU and hybrid (GPU) renderers use this trait to interact with the
/// glyph cache. Shared rendering code in `vello_renderer` is generic over this
/// trait, so different backends can provide different storage strategies without
/// duplicating orchestration logic.
pub trait GlyphCache {
    /// Look up a cached glyph.
    ///
    /// Returns `Some(AtlasSlot)` on cache hit (copy), `None` on miss.
    /// Updates the entry's access time on hit.
    fn get(&mut self, key: &GlyphCacheKey) -> Option<AtlasSlot>;

    /// Insert a glyph entry and allocate space in the atlas.
    ///
    /// Returns `(dst_x, dst_y, atlas_slot, recorder)` if successful, `None`
    /// if allocation failed (e.g., atlas is full).  The returned recorder
    /// accumulates draw commands for the atlas page the glyph was placed on.
    fn insert(
        &mut self,
        image_cache: &mut ImageCache,
        key: GlyphCacheKey,
        meta: RasterMetrics,
    ) -> Option<(u16, u16, AtlasSlot, &mut AtlasCommandRecorder)>;

    /// Queue a bitmap pixmap for later processing.
    fn push_pending_upload(&mut self, image_id: ImageId, pixmap: Arc<Pixmap>, atlas_slot: AtlasSlot);

    /// Take all pending bitmap uploads, leaving the internal queue empty.
    fn take_pending_uploads(&mut self) -> Vec<PendingBitmapUpload>;

    /// Take all pending atlas command recorders (one per dirty page),
    /// leaving the internal collection empty.
    fn take_pending_atlas_commands(&mut self) -> Vec<AtlasCommandRecorder>;

    /// Advance the frame counter. Should be called once per frame/render cycle.
    fn tick(&mut self);

    /// Run maintenance and potentially evict old entries.
    fn maintain(&mut self, image_cache: &mut ImageCache);

    /// Clear the entire cache.
    fn clear(&mut self);

    /// Get the number of cached glyphs.
    fn len(&self) -> usize;

    /// Check if the cache is empty.
    fn is_empty(&self) -> bool;

    /// Get the number of cache hits since last `clear_stats()`.
    fn cache_hits(&self) -> u64;

    /// Get the number of cache misses since last `clear_stats()`.
    fn cache_misses(&self) -> u64;

    /// Clear cache hit/miss statistics without clearing the cache itself.
    fn clear_stats(&mut self);
}

/// A bitmap glyph pixmap awaiting GPU upload.
///
/// Accumulated during glyph encoding when a bitmap glyph is inserted into the
/// atlas cache. The application must drain these via
/// [`GlyphCache::take_pending_uploads`] and upload each pixmap to the
/// GPU atlas at the position indicated by `image_id` (look up via
/// `ImageCache::get` to obtain atlas layer and offset).
#[derive(Debug)]
pub struct PendingBitmapUpload {
    /// The image ID allocated in the shared `ImageCache`.
    /// Use `image_cache.get(image_id)` to obtain `atlas_id` and `offset`.
    pub image_id: ImageId,
    /// The bitmap pixel data to upload.
    pub pixmap: Arc<Pixmap>,
    /// The atlas slot information for this glyph (includes dimensions).
    pub atlas_slot: AtlasSlot,
}

/// Core glyph bitmap cache data shared by all renderer backends.
///
/// Contains the cache entries, LRU tracking, pending uploads, and statistics.
/// Does **not** own any pixel storage — that responsibility belongs to the
/// concrete wrapper types ([`CpuGlyphAtlas`], [`GpuGlyphAtlas`]).
pub struct GlyphAtlas {
    /// Entries for non-variable fonts.
    static_entries: HashMap<GlyphCacheKey, GlyphCacheEntry>,
    /// Entries for variable fonts, keyed by variation coordinates.
    variable_entries: HashMap<VarKey, HashMap<GlyphCacheKey, GlyphCacheEntry>>,
    /// Current frame serial for LRU tracking.
    serial: u32,
    /// Serial of last eviction pass.
    last_eviction_serial: u32,
    /// Total cached glyph count (across all maps).
    entry_count: usize,
    /// Bitmap glyphs awaiting GPU upload.
    pending_uploads: Vec<PendingBitmapUpload>,
    /// Outline and COLR glyph commands awaiting replay, indexed by atlas page.
    /// Uses `SmallVec` with inline capacity of 1 because most applications use
    /// a single atlas page; the common case avoids heap allocation entirely.
    pending_atlas_commands: SmallVec<[Option<AtlasCommandRecorder>; 1]>,
    /// Number of cache hits since last clear_stats().
    cache_hits: u64,
    /// Number of cache misses since last clear_stats().
    cache_misses: u64,
}

impl GlyphAtlas {
    /// Creates a new empty core cache.
    pub fn new() -> Self {
        Self {
            static_entries: HashMap::new(),
            variable_entries: HashMap::new(),
            serial: 0,
            last_eviction_serial: 0,
            entry_count: 0,
            pending_uploads: Vec::new(),
            pending_atlas_commands: SmallVec::new(),
            cache_hits: 0,
            cache_misses: 0,
        }
    }

    /// Look up a cached glyph.
    pub(crate) fn get(&mut self, key: &GlyphCacheKey) -> Option<AtlasSlot> {
        let serial = self.serial;
        let entries = if key.var_coords.is_empty() {
            &mut self.static_entries
        } else {
            match self
                .variable_entries
                .raw_entry_mut()
                .from_key(&VarLookupKey(&key.var_coords))
            {
                RawEntryMut::Occupied(e) => e.into_mut(),
                RawEntryMut::Vacant(_) => {
                    self.cache_misses += 1;
                    return None;
                }
            }
        };

        match entries.get_mut(key) {
            Some(entry) => {
                entry.serial = serial;
                self.cache_hits += 1;
                Some(entry.atlas_slot)
            }
            None => {
                self.cache_misses += 1;
                None
            }
        }
    }

    /// Allocate atlas space and insert a cache entry.
    ///
    /// Returns `(atlas_idx, dst_x, dst_y, atlas_slot)` on success. The caller is
    /// responsible for ensuring the atlas page at `atlas_idx` exists in its
    /// storage backend.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "atlas offsets fit in u16 at reasonable atlas sizes"
    )]
    pub(crate) fn insert_entry(
        &mut self,
        image_cache: &mut ImageCache,
        key: GlyphCacheKey,
        meta: RasterMetrics,
    ) -> Option<(usize, u16, u16, AtlasSlot)> {
        // Add padding to prevent texture bleeding between glyphs
        let padded_w = u32::from(meta.width) + u32::from(GLYPH_PADDING) * 2;
        let padded_h = u32::from(meta.height) + u32::from(GLYPH_PADDING) * 2;

        let image_id = image_cache.allocate(padded_w, padded_h).ok()?;
        let resource = image_cache.get(image_id)?;
        let atlas_idx = resource.atlas_id.as_u32() as usize;

        // Offset by padding to position glyph inside padded region
        let x = resource.offset[0] + GLYPH_PADDING;
        let y = resource.offset[1] + GLYPH_PADDING;

        // Create atlas slot
        let atlas_slot = AtlasSlot {
            image_id,
            page_index: atlas_idx as u32,
            x,
            y,
            width: meta.width,
            height: meta.height,
            bearing_x: meta.bearing_x,
            bearing_y: meta.bearing_y,
        };

        // Store entry
        let entry = GlyphCacheEntry {
            atlas_slot,
            serial: self.serial,
        };

        let entries = if key.var_coords.is_empty() {
            &mut self.static_entries
        } else {
            match self
                .variable_entries
                .raw_entry_mut()
                .from_key(&VarLookupKey(&key.var_coords))
            {
                RawEntryMut::Occupied(e) => e.into_mut(),
                RawEntryMut::Vacant(e) => e.insert(key.var_coords.clone(), HashMap::new()).1,
            }
        };

        entries.insert(key, entry);
        self.entry_count += 1;

        Some((atlas_idx, atlas_slot.x, atlas_slot.y, atlas_slot))
    }

    /// Take all pending bitmap uploads, leaving the internal queue empty.
    pub fn take_pending_uploads(&mut self) -> Vec<PendingBitmapUpload> {
        core::mem::take(&mut self.pending_uploads)
    }

    /// Queue a bitmap pixmap for later processing.
    pub fn push_pending_upload(
        &mut self,
        image_id: ImageId,
        pixmap: Arc<Pixmap>,
        atlas_slot: AtlasSlot,
    ) {
        self.pending_uploads.push(PendingBitmapUpload {
            image_id,
            pixmap,
            atlas_slot,
        });
    }

    /// Take all pending atlas command recorders, leaving the internal collection empty.
    pub fn take_pending_atlas_commands(
        &mut self,
    ) -> SmallVec<[Option<AtlasCommandRecorder>; 1]> {
        core::mem::take(&mut self.pending_atlas_commands)
    }

    /// Get (or create) the command recorder for the given atlas page.
    ///
    /// Uses direct index access into a `SmallVec` — no hashing overhead.
    pub fn recorder_for_page(
        &mut self,
        page_index: u32,
        atlas_width: u16,
        atlas_height: u16,
    ) -> &mut AtlasCommandRecorder {
        let idx = page_index as usize;
        if self.pending_atlas_commands.len() <= idx {
            self.pending_atlas_commands.resize_with(idx + 1, || None);
        }
        self.pending_atlas_commands[idx]
            .get_or_insert_with(|| AtlasCommandRecorder::new(page_index, atlas_width, atlas_height))
    }

    /// Advance the frame counter.
    pub fn tick(&mut self) {
        self.serial = self.serial.wrapping_add(1);
    }

    /// Run maintenance and potentially evict old entries.
    pub fn maintain(&mut self, image_cache: &mut ImageCache) {
        let frames_since_eviction = self.serial.wrapping_sub(self.last_eviction_serial);
        if frames_since_eviction < EVICTION_FREQUENCY && self.entry_count < CACHED_COUNT_THRESHOLD {
            return;
        }

        self.last_eviction_serial = self.serial;
        self.evict_old_entries(image_cache);
    }

    /// Evict entries that haven't been used recently.
    fn evict_old_entries(&mut self, image_cache: &mut ImageCache) {
        let serial = self.serial;
        let entry_count = &mut self.entry_count;

        // Evict from static entries
        self.static_entries.retain(|_, entry| {
            let age = serial.wrapping_sub(entry.serial);
            if age > MAX_ENTRY_AGE {
                image_cache.deallocate(entry.atlas_slot.image_id);
                *entry_count = entry_count.saturating_sub(1);
                false
            } else {
                true
            }
        });

        // Evict from variable entries
        self.variable_entries.retain(|_, entries| {
            entries.retain(|_, entry| {
                let age = serial.wrapping_sub(entry.serial);
                if age > MAX_ENTRY_AGE {
                    image_cache.deallocate(entry.atlas_slot.image_id);
                    *entry_count = entry_count.saturating_sub(1);
                    false
                } else {
                    true
                }
            });
            !entries.is_empty()
        });
    }

    /// Clear the entire cache (entries, pending uploads, stats).
    pub fn clear(&mut self) {
        self.static_entries.clear();
        self.variable_entries.clear();
        self.serial = 0;
        self.last_eviction_serial = 0;
        self.entry_count = 0;
        self.pending_uploads.clear();
        self.pending_atlas_commands.clear();
        self.cache_hits = 0;
        self.cache_misses = 0;
    }

    /// Get the number of cached glyphs.
    #[inline]
    pub fn len(&self) -> usize {
        self.entry_count
    }

    /// Check if the cache is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entry_count == 0
    }

    /// Get the number of cache hits since last `clear_stats()`.
    #[inline]
    pub fn cache_hits(&self) -> u64 {
        self.cache_hits
    }

    /// Get the number of cache misses since last `clear_stats()`.
    #[inline]
    pub fn cache_misses(&self) -> u64 {
        self.cache_misses
    }

    /// Clear cache hit/miss statistics without clearing the cache itself.
    pub fn clear_stats(&mut self) {
        self.cache_hits = 0;
        self.cache_misses = 0;
    }

    /// Print cache hit/miss statistics to stdout.
    pub fn print_cache_stats(&self) {
        let total = self.cache_hits + self.cache_misses;
        let hit_rate = if total > 0 {
            (self.cache_hits as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        println!("=== Cache Hit/Miss Statistics ===");
        println!("Cache hits:   {}", self.cache_hits);
        println!("Cache misses: {}", self.cache_misses);
        println!("Total lookups: {}", total);
        println!("Hit rate:     {:.2}%", hit_rate);
    }
}

/// Statistics about cached glyphs.
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct GlyphCacheStats {
    /// Total number of cached glyph entries.
    pub total_glyphs: usize,
    /// Number of glyphs from static (non-variable) fonts.
    pub static_glyphs: usize,
    /// Number of glyphs from variable fonts.
    pub variable_glyphs: usize,
    /// Number of atlas pages currently allocated.
    pub page_count: usize,
    /// Number of unique glyph IDs (same glyph may have multiple entries due to subpixel).
    pub unique_glyph_ids: usize,
    /// Distribution of entries across subpixel buckets.
    pub subpixel_distribution: [usize; SUBPIXEL_BUCKETS as usize],
    /// List of unique font sizes used.
    pub sizes_used: Vec<f32>,
}

#[cfg(feature = "std")]
impl GlyphAtlas {
    /// Get detailed statistics about cached glyphs.
    ///
    /// `page_count` is supplied by the caller because page storage is
    /// owned by the concrete cache wrapper, not the core.
    pub fn stats(&self, page_count: usize) -> GlyphCacheStats {
        let mut unique_ids = HashSet::new();
        let mut subpixel_dist = [0; SUBPIXEL_BUCKETS as usize];
        let mut sizes = HashSet::new();

        for key in self.static_entries.keys() {
            unique_ids.insert(key.glyph_id);
            subpixel_dist[key.subpixel_x as usize] += 1;
            sizes.insert(key.size_bits);
        }

        let variable_count: usize = self.variable_entries.values().map(|m| m.len()).sum();

        for entries in self.variable_entries.values() {
            for key in entries.keys() {
                unique_ids.insert(key.glyph_id);
                subpixel_dist[key.subpixel_x as usize] += 1;
                sizes.insert(key.size_bits);
            }
        }

        GlyphCacheStats {
            total_glyphs: self.entry_count,
            static_glyphs: self.static_entries.len(),
            variable_glyphs: variable_count,
            page_count,
            unique_glyph_ids: unique_ids.len(),
            subpixel_distribution: subpixel_dist,
            sizes_used: sizes.into_iter().map(f32::from_bits).collect(),
        }
    }

    /// Print cache statistics to stdout.
    pub fn print_stats(&self, page_count: usize) {
        let stats = self.stats(page_count);
        println!("=== Glyph Atlas Statistics ===");
        println!("Total cached glyphs: {}", stats.total_glyphs);
        println!("Unique glyph IDs: {}", stats.unique_glyph_ids);
        println!("Atlas pages: {}", stats.page_count);
        println!("Static font glyphs: {}", stats.static_glyphs);
        println!("Variable font glyphs: {}", stats.variable_glyphs);
        println!("Subpixel distribution: {:?}", stats.subpixel_distribution);
        println!("Font sizes: {:?}", stats.sizes_used);

        // Show duplication ratio
        if stats.unique_glyph_ids > 0 {
            let ratio = stats.total_glyphs as f32 / stats.unique_glyph_ids as f32;
            println!("Avg entries per unique glyph: {:.2}", ratio);
        }
    }

    /// Returns all cached glyph keys (for debugging).
    pub fn all_keys(&self) -> Vec<&GlyphCacheKey> {
        let mut keys: Vec<_> = self.static_entries.keys().collect();
        for entries in self.variable_entries.values() {
            keys.extend(entries.keys());
        }
        keys
    }

    /// Print all cached keys grouped by glyph ID.
    ///
    /// This is useful for understanding why the same glyph appears multiple
    /// times in the atlas (e.g., different subpixel positions or sizes).
    pub fn print_keys_grouped(&self) {
        // Store (key, source) where source indicates static or variable
        let mut by_glyph: HashMap<u32, Vec<(&GlyphCacheKey, &str)>> = HashMap::new();

        for key in self.static_entries.keys() {
            by_glyph
                .entry(key.glyph_id)
                .or_default()
                .push((key, "stat"));
        }
        for entries in self.variable_entries.values() {
            for key in entries.keys() {
                by_glyph
                    .entry(key.glyph_id)
                    .or_default()
                    .push((key, "var "));
            }
        }

        println!(
            "=== Glyph Keys Grouped by ID ({} unique) ===",
            by_glyph.len()
        );

        let mut ids: Vec<_> = by_glyph.keys().copied().collect();
        ids.sort();

        for glyph_id in ids {
            let keys = &by_glyph[&glyph_id];
            let suffix = if keys.len() == 1 { "entry" } else { "entries" };
            println!("glyph_id {:4} ({} {}):", glyph_id, keys.len(), suffix);
            for (k, source) in keys {
                println!(
                    "    [{}] subpx: {}, size: {:.2}, hinted: {}, font_id: {:016x}, font_index: {}",
                    source,
                    k.subpixel_x,
                    f32::from_bits(k.size_bits),
                    k.hinted,
                    k.font_id,
                    k.font_index,
                );
            }
        }
    }
}

impl Default for GlyphAtlas {
    fn default() -> Self {
        Self::new()
    }
}

impl Debug for GlyphAtlas {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GlyphAtlas")
            .field("entry_count", &self.entry_count)
            .field("static_entries", &self.static_entries.len())
            .field("variable_fonts", &self.variable_entries.len())
            .field("serial", &self.serial)
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Save a pixmap to a PNG file at the specified path.
#[cfg(feature = "png")]
pub(crate) fn save_pixmap_to_png(pixmap: &Pixmap, path: &std::path::Path) -> std::io::Result<()> {
    use std::fs::File;
    use std::io::BufWriter;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(path)?;
    let w = BufWriter::new(file);

    let mut encoder = png::Encoder::new(w, pixmap.width() as u32, pixmap.height() as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder
        .write_header()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    writer
        .write_image_data(pixmap.data_as_u8_slice())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    Ok(())
}

/// Internal cache entry storing atlas slot and access time.
struct GlyphCacheEntry {
    /// Atlas slot information for blitting.
    atlas_slot: AtlasSlot,

    /// Frame serial when last accessed (for LRU eviction).
    serial: u32,
}

/// Key for variable font caches (owned version).
type VarKey = SmallVec<[skrifa::instance::NormalizedCoord; 4]>;

/// Lookup key for variable font caches (borrowed version).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
struct VarLookupKey<'a>(&'a [skrifa::instance::NormalizedCoord]);

impl hashbrown::Equivalent<VarKey> for VarLookupKey<'_> {
    fn equivalent(&self, other: &VarKey) -> bool {
        self.0 == other.as_slice()
    }
}

impl From<VarLookupKey<'_>> for VarKey {
    fn from(key: VarLookupKey<'_>) -> Self {
        SmallVec::from_slice(key.0)
    }
}
