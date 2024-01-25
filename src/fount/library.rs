use super::data::*;
use super::scan::{scan_path, FontScanner};
use std::io;
use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock};

/// Indexed collection of fonts and associated metadata supporting queries and
/// fallback.
///
/// This struct is opaque and provides shared storage for a font collection.
/// Accessing the collection is done by creating a [`FontContext`](super::context::FontContext)
/// wrapping this struct.
#[derive(Clone)]
pub struct Library {
    pub(crate) inner: Arc<Inner>,
}

impl Library {
    fn new(system: SystemCollectionData) -> Self {
        let mut user = CollectionData::default();
        user.is_user = true;
        Self {
            inner: Arc::new(Inner {
                system,
                user: Arc::new(RwLock::new(user)),
                user_version: Arc::new(AtomicU64::new(0)),
            }),
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
impl Default for Library {
    fn default() -> Self {
        let system =
            SystemCollectionData::Static(StaticCollection::new(&super::platform::STATIC_DATA));
        Self::new(system)
    }
}

#[cfg(target_os = "linux")]
impl Default for Library {
    fn default() -> Self {
        use std::ffi::OsStr;
        use std::os::unix::prelude::OsStrExt;

        // Find a newline-separated list of all font files that fc-list knows about.
        // It would be nice to have a nul-separated list (because 100% the filenames don't
        // have embedded nuls) but fc-list doesn't like it.
        let cmd = std::process::Command::new("fc-list")
            .arg("--format=%{file}\n")
            .output()
            .expect("failed to execute fc-list");
        if !cmd.status.success() {
            panic!("fc-list failed");
        }
        let mut builder = LibraryBuilder::default();
        for filename in cmd.stdout.split(|&b| b == b'\n') {
            if !filename.is_empty() {
                builder
                    .add_system_path(OsStr::from_bytes(filename))
                    .expect("add_system_path failed");
            }
        }
        builder.build()
    }
}

pub struct Inner {
    pub system: SystemCollectionData,
    pub user: Arc<RwLock<CollectionData>>,
    pub user_version: Arc<AtomicU64>,
}

/// Builder for configuring a font library.
#[derive(Default)]
pub struct LibraryBuilder {
    scanner: FontScanner,
    system: CollectionData,
    fallback: FallbackData,
}

impl LibraryBuilder {
    pub fn add_system_path<T: AsRef<Path>>(&mut self, path: T) -> Result<(), io::Error> {
        scan_path(
            path.as_ref(),
            &mut self.scanner,
            &mut self.system,
            &mut self.fallback,
        )
    }

    pub fn build(self) -> Library {
        let system = SystemCollectionData::Scanned(ScannedCollectionData {
            collection: self.system,
            fallback: self.fallback,
        });
        Library::new(system)
    }
}
