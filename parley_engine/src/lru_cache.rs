// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use hashbrown::Equivalent;

/// A least-recently-used cache. This cache uses a linear scan of its entries
/// to find a given entry - it is optimised for a low number of entries. Preferably
/// keep `max_entries` low - in the order of tens.
pub(crate) struct LruCache<Id, T> {
    store: LruStore<(Id, T)>,
}

impl<Id, T> LruCache<Id, T> {
    pub(crate) fn new(max_entries: usize) -> Self {
        Self {
            store: LruStore::new(max_entries),
        }
    }

    /// Returns a reference to the entry with the given ID. If the entry is not
    /// found, it is created and returned using `make_data`.
    pub(crate) fn entry<K>(&mut self, key: K, make: impl FnOnce() -> T) -> &T
    where
        K: Equivalent<Id> + Into<Id>,
    {
        let (_, value) = self
            .store
            .entry(|(id, _)| key.equivalent(id))
            .or_insert_with(|| (key.into(), make()));
        value
    }
}

struct Record<T> {
    epoch: u64,
    data: T,
}

pub(crate) struct LruStore<T> {
    entries: Vec<Record<T>>,
    epoch: u64,
    max_entries: usize,
}

impl<T> LruStore<T> {
    pub(crate) fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            epoch: 0,
            max_entries,
        }
    }

    /// Finds the first matching entry, or a slot for insertion on a miss.
    pub(crate) fn entry(&mut self, mut is_match: impl FnMut(&T) -> bool) -> Entry<'_, T> {
        self.epoch += 1;
        let mut oldest_epoch = self.epoch;
        let mut oldest_index = 0;
        for (i, entry) in self.entries.iter().enumerate() {
            if is_match(&entry.data) {
                let entry = &mut self.entries[i];
                entry.epoch = self.epoch;
                return Entry::Occupied(&entry.data);
            }
            if entry.epoch < oldest_epoch {
                oldest_epoch = entry.epoch;
                oldest_index = i;
            }
        }
        Entry::Vacant {
            store: self,
            oldest_index,
        }
    }
}

pub(crate) enum Entry<'a, T> {
    Occupied(&'a T),
    Vacant {
        store: &'a mut LruStore<T>,
        oldest_index: usize,
    },
}

impl<'a, T> Entry<'a, T> {
    pub(crate) fn or_insert_with(self, make: impl FnOnce() -> T) -> &'a T {
        match self {
            Self::Occupied(data) => data,
            Self::Vacant {
                store,
                oldest_index,
            } => {
                let entry = Record {
                    epoch: store.epoch,
                    data: make(),
                };
                if store.entries.len() < store.max_entries {
                    &store.entries.push_mut(entry).data
                } else {
                    store.entries[oldest_index] = entry;
                    &store.entries[oldest_index].data
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::{String, ToString};

    #[derive(Debug, Clone, PartialEq)]
    struct TestId(String);
    struct TestLookupKey<'a>(&'a str);

    impl<'a> Equivalent<TestId> for TestLookupKey<'a> {
        fn equivalent(&self, key: &TestId) -> bool {
            self.0 == key.0.as_str()
        }
    }

    impl<'a> From<TestLookupKey<'a>> for TestId {
        fn from(key: TestLookupKey<'a>) -> Self {
            Self(key.0.to_string())
        }
    }

    impl Equivalent<Self> for TestId {
        fn equivalent(&self, key: &Self) -> bool {
            self.0 == key.0
        }
    }

    #[test]
    fn test_retrieve_existing_entry() {
        let mut cache = LruCache::new(3);

        // Insert an entry
        let value1 = cache.entry(TestLookupKey("key1"), || 42);
        assert_eq!(*value1, 42);

        // Retrieve the same entry - make_data should not be called
        let value2 = cache.entry(TestLookupKey("key1"), || {
            panic!("Should not create new data")
        });
        assert_eq!(*value2, 42);
        assert_eq!(cache.store.entries.len(), 1);
    }

    #[test]
    fn test_multiple_entries() {
        let mut cache = LruCache::new(3);

        let value1 = cache.entry(TestLookupKey("key1"), || 1);
        assert_eq!(*value1, 1);

        let value2 = cache.entry(TestLookupKey("key2"), || 2);
        assert_eq!(*value2, 2);

        let value3 = cache.entry(TestLookupKey("key3"), || 3);
        assert_eq!(*value3, 3);

        assert_eq!(cache.store.entries.len(), 3);
        assert_eq!(cache.store.epoch, 3);
    }

    #[test]
    fn test_lru_eviction() {
        let mut cache = LruCache::new(3);

        // Add three entries
        cache.entry(TestLookupKey("key1"), || 1);
        cache.entry(TestLookupKey("key2"), || 2);
        cache.entry(TestLookupKey("key3"), || 3);

        // Access key1 to update its epoch
        cache.entry(TestLookupKey("key1"), || panic!("Should not create"));

        // Add key4 - should evict key2 (oldest untouched)
        cache.entry(TestLookupKey("key4"), || 4);

        // Verify key1 is still present
        let value1 = cache.entry(TestLookupKey("key1"), || {
            panic!("key1 should still be present")
        });
        assert_eq!(*value1, 1);

        // Verify key2 was evicted
        let mut was_created = false;
        cache.entry(TestLookupKey("key2"), || {
            was_created = true;
            20
        });
        assert!(was_created, "key2 should have been evicted");
    }

    #[test]
    fn test_lru_eviction_after_multiple_hits() {
        let mut cache = LruCache::new(3);

        cache.entry(TestLookupKey("key1"), || 1);
        cache.entry(TestLookupKey("key2"), || 2);
        cache.entry(TestLookupKey("key3"), || 3);

        // Hit all three in order: key3 first (making it LRU among hits),
        // then key1, then key2 (most recently used).
        cache.entry(TestLookupKey("key3"), || panic!("Should not create"));
        cache.entry(TestLookupKey("key1"), || panic!("Should not create"));
        cache.entry(TestLookupKey("key2"), || panic!("Should not create"));

        // Insert key4 — should evict key3 (least recently accessed)
        cache.entry(TestLookupKey("key4"), || 4);

        // key1 and key2 should still be present with original values
        // (check these first since verifying key3 eviction will trigger another eviction)
        let v1 = cache.entry(TestLookupKey("key1"), || {
            panic!("key1 should still be present")
        });
        assert_eq!(*v1, 1);

        let v2 = cache.entry(TestLookupKey("key2"), || {
            panic!("key2 should still be present")
        });
        assert_eq!(*v2, 2);

        // key3 should have been evicted
        let mut key3_recreated = false;
        cache.entry(TestLookupKey("key3"), || {
            key3_recreated = true;
            30
        });
        assert!(key3_recreated, "key3 should have been evicted as the LRU");
    }
}
