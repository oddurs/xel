//! The xel engine for the macOS app, through `UniFFI`. Every type here is what Swift sees;
//! the engine itself stays free of FFI concerns.
//!
//! Every exported function returns `Result`: `UniFFI` catches a Rust panic and throws it
//! in Swift only for throwing functions, and crashes the app for the others.

// Every fallible function returns the one `XelError` type; `# Errors` sections would
// repeat it. UniFFI passes strings by value, so owned `String` parameters are required.
#![allow(clippy::missing_errors_doc, clippy::needless_pass_by_value)]

use std::sync::{Arc, Mutex, MutexGuard};

use xel_engine::{ColumnType, Workbook};

uniffi::setup_scaffolding!();

/// An engine error, as Swift receives it.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum XelError {
    #[error("{message}")]
    Engine { message: String },
}

impl From<xel_engine::Error> for XelError {
    fn from(e: xel_engine::Error) -> Self {
        Self::Engine {
            message: e.to_string(),
        }
    }
}

/// An open workbook. Swift may call it from any thread; calls are serialized, as the
/// engine's SQLite connection is used by one thread at a time.
#[derive(uniffi::Object)]
pub struct Book {
    inner: Mutex<Workbook>,
}

impl Book {
    fn lock(&self) -> MutexGuard<'_, Workbook> {
        // A panic while holding the lock leaves the workbook as SQLite left it, which is
        // consistent (every edit is a transaction), so a poisoned lock is still usable.
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[uniffi::export]
impl Book {
    #[uniffi::constructor]
    pub fn create(path: String) -> Result<Arc<Self>, XelError> {
        Ok(Arc::new(Self {
            inner: Mutex::new(Workbook::create(path)?),
        }))
    }

    #[uniffi::constructor]
    pub fn open(path: String) -> Result<Arc<Self>, XelError> {
        Ok(Arc::new(Self {
            inner: Mutex::new(Workbook::open(path)?),
        }))
    }

    pub fn format_version(&self) -> Result<u32, XelError> {
        Ok(self.lock().format_version())
    }

    pub fn table_names(&self) -> Result<Vec<String>, XelError> {
        Ok(self.lock().tables()?.into_iter().map(|t| t.name).collect())
    }

    /// Creates a table with one text column, for the bridge spike.
    pub fn create_table(&self, name: String) -> Result<(), XelError> {
        self.lock()
            .create_table(&name, &[("Name", ColumnType::Text)])?;
        Ok(())
    }
}
