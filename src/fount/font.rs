use peniko::{Blob, WeakBlob};
use std::path::Path;

/// Shared reference to owned font data.
#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct FontData(pub Blob<u8>);

impl FontData {
    /// Creates font data from the specified bytes.
    pub fn new(data: Vec<u8>) -> Self {
        Self(data.into())
    }

    /// Creates font data from the file at the specified path.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        let path = path.as_ref();
        Ok(Self::new(std::fs::read(path)?))
    }

    /// Creates a new weak reference to the data.
    pub fn downgrade(&self) -> WeakFontData {
        WeakFontData(self.0.downgrade())
    }

    /// Returns the underlying bytes of the data.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.data()
    }

    /// Returns the number of strong references to the data.
    pub fn strong_count(&self) -> usize {
        self.0.strong_count()
    }
}

impl std::ops::Deref for FontData {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0.data()
    }
}

impl AsRef<[u8]> for FontData {
    fn as_ref(&self) -> &[u8] {
        self.0.data()
    }
}

/// Weak reference to owned font data.
#[derive(Clone)]
#[repr(transparent)]
pub struct WeakFontData(WeakBlob<u8>);

impl WeakFontData {
    /// Upgrades the weak reference.
    pub fn upgrade(&self) -> Option<FontData> {
        self.0.upgrade().map(FontData)
    }
}
