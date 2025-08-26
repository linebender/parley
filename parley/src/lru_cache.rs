/// A lookup key is distinct from the ID type. This allows the lookup key
/// to not require ownership of the underlying ID data, which could require
/// allocations.
pub(crate) trait LookupKey<ID> {
    fn eq(&self, other: &ID) -> bool;
    fn to_id(self) -> ID;
}

/// An entry in the cache.
pub(crate) struct Entry<ID, T> {
    pub epoch: u64,
    pub id: ID,
    pub data: T,
}

/// A least-recently-used cache. This cache uses a linear scan of its entries
/// to find a given entry - it is optimised for a low number of entries. Preferably
/// keep `max_entries` low in the order of tens.
pub(crate) struct LruCache<ID, T> {
    entries: Vec<Entry<ID, T>>,
    epoch: u64,
    max_entries: usize,
}

impl<ID, T> LruCache<ID, T> {
    pub(crate) fn new(max_entries: usize) -> Self {
        Self {
            entries: Default::default(),
            epoch: 0,
            max_entries,
        }
    }

    /// Returns a reference to the entry with the given ID. If the entry is not
    /// found, it is created and returned using `make_data`.
    pub(crate) fn entry<'a>(
        &'a mut self,
        id: impl LookupKey<ID>,
        make_data: impl FnOnce() -> T,
    ) -> &'a T {
        match self.find_entry(id, make_data) {
            (true, index) => {
                let entry = &mut self.entries[index];
                entry.epoch = self.epoch;
                &entry.data
            }
            (false, index) => {
                self.epoch += 1;
                let entry = &mut self.entries[index];
                entry.epoch = self.epoch;
                &entry.data
            }
        }
    }

    fn find_entry(
        &mut self,
        id: impl LookupKey<ID>,
        make_data: impl FnOnce() -> T,
    ) -> (bool, usize) {
        let epoch = self.epoch;
        let mut lowest_serial = epoch;
        let mut lowest_index = 0;
        for (i, entry) in self.entries.iter().enumerate() {
            if id.eq(&entry.id) {
                return (true, i);
            }
            if entry.epoch < lowest_serial {
                lowest_serial = entry.epoch;
                lowest_index = i;
            }
        }
        if self.entries.len() < self.max_entries {
            lowest_index = self.entries.len();
            self.entries.push(Entry {
                epoch,
                id: id.to_id(),
                data: make_data(),
            });
        } else {
            let entry = &mut self.entries[lowest_index];
            entry.epoch = epoch;
            entry.id = id.to_id();
            entry.data = make_data();
        }
        (false, lowest_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple ID type for testing
    #[derive(Debug, Clone, PartialEq)]
    struct TestId(String);

    // LookupKey implementation that allows &str to lookup TestId
    struct TestLookupKey<'a>(&'a str);

    impl<'a> LookupKey<TestId> for TestLookupKey<'a> {
        fn eq(&self, other: &TestId) -> bool {
            self.0 == other.0.as_str()
        }

        fn to_id(self) -> TestId {
            TestId(self.0.to_string())
        }
    }

    // Alternative implementation for owned strings
    impl LookupKey<TestId> for TestId {
        fn eq(&self, other: &TestId) -> bool {
            self.0 == other.0
        }

        fn to_id(self) -> TestId {
            self
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
        assert_eq!(cache.entries.len(), 1);
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

        assert_eq!(cache.entries.len(), 3);
        assert_eq!(cache.epoch, 3);
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
}
