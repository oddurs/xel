//! Helpers shared by unit tests. Kept dependency-free on purpose.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// A fresh directory under the system temp dir, removed on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        loop {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("xel-test-{}-{n}", std::process::id()));
            // create_dir, not create_dir_all: a directory left by a killed run with a
            // recycled pid must not be reused.
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
                Err(e) => panic!("cannot create {}: {e}", path.display()),
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Drop cannot return an error and must not panic during unwinding; report it.
        if let Err(e) = std::fs::remove_dir_all(&self.0) {
            eprintln!("warning: could not remove {}: {e}", self.0.display());
        }
    }
}
