//! Helpers shared by unit tests. Kept dependency-free on purpose.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// A uniquely named directory under the system temp dir, removed on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("xel-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).expect("temp dir is writable");
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Best effort: a leftover temp dir must not fail the test that created it.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
