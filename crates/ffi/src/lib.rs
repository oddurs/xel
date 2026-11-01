//! The xel engine for the macOS app, through `UniFFI`. Every type here is what Swift sees;
//! the engine itself stays free of FFI concerns.
//!
//! Every exported function returns `Result`: `UniFFI` catches a Rust panic and throws it
//! in Swift only for throwing functions, and crashes the app for the others.

// Every fallible function returns the one `XelError` type; `# Errors` sections would
// repeat it.
#![allow(clippy::missing_errors_doc)]

use std::sync::{Arc, Mutex, MutexGuard};

use xel_engine::{ColumnType, Workbook};

uniffi::setup_scaffolding!();

/// An engine error, as Swift receives it. The kinds the app reacts to differently are
/// separate cases; everything else carries the engine's message.
#[derive(Debug, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum XelError {
    #[error("{message}")]
    NotFound { message: String },
    #[error("{message}")]
    NotAWorkbook { message: String },
    #[error("{message}")]
    NewerFormat { message: String },
    #[error("{message}")]
    ReadOnly { message: String },
    #[error("{message}")]
    Corrupt { message: String },
    /// A rejected edit: a bad name, a value of the wrong type, and the like.
    #[error("{message}")]
    Invalid { message: String },
    #[error("{message}")]
    Other { message: String },
    #[error("the workbook is closed")]
    Closed,
    /// An earlier call panicked part-way; the in-memory undo history may no longer match
    /// the file, so the workbook must be closed and reopened.
    #[error("the workbook is in an unknown state; close and reopen it")]
    Poisoned,
}

impl From<xel_engine::Error> for XelError {
    fn from(e: xel_engine::Error) -> Self {
        use xel_engine::Error as E;
        let message = e.to_string();
        match e {
            E::NotFound(_) => Self::NotFound { message },
            E::NotAWorkbook(_) => Self::NotAWorkbook { message },
            E::NewerFormat { .. } => Self::NewerFormat { message },
            E::ReadOnly(_) => Self::ReadOnly { message },
            E::Corrupt(_) | E::ChangedOutside(_) => Self::Corrupt { message },
            E::InvalidName { .. }
            | E::DuplicateName(_)
            | E::NoColumns
            | E::LastColumn(_)
            | E::NotASelectColumn(_)
            | E::TypeMismatch { .. }
            | E::NotFinite { .. }
            | E::NotAnOption { .. } => Self::Invalid { message },
            _ => Self::Other { message },
        }
    }
}

/// An open workbook. Swift may call it from any thread; calls are serialized, as the
/// engine's SQLite connection is used by one thread at a time.
#[derive(uniffi::Object)]
pub struct Book {
    inner: Mutex<Option<Workbook>>,
}

impl Book {
    fn new(book: Workbook) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Some(book)),
        })
    }

    fn with<T>(&self, f: impl FnOnce(&mut Workbook) -> Result<T, XelError>) -> Result<T, XelError> {
        let mut guard: MutexGuard<'_, Option<Workbook>> =
            self.inner.lock().map_err(|_| XelError::Poisoned)?;
        f(guard.as_mut().ok_or(XelError::Closed)?)
    }
}

#[uniffi::export]
impl Book {
    #[uniffi::constructor]
    pub fn create(path: &str) -> Result<Arc<Self>, XelError> {
        Ok(Self::new(Workbook::create(path)?))
    }

    #[uniffi::constructor]
    pub fn open(path: &str) -> Result<Arc<Self>, XelError> {
        Ok(Self::new(Workbook::open(path)?))
    }

    /// Closes the file, dropping undo history and its trash, and reports failures that
    /// releasing the object would hide. Later calls fail with `Closed`.
    pub fn close(&self) -> Result<(), XelError> {
        let mut guard = self.inner.lock().map_err(|_| XelError::Poisoned)?;
        match guard.take() {
            Some(book) => Ok(book.close()?),
            None => Err(XelError::Closed),
        }
    }

    pub fn format_version(&self) -> Result<u32, XelError> {
        self.with(|b| Ok(b.format_version()))
    }

    pub fn table_names(&self) -> Result<Vec<String>, XelError> {
        self.with(|b| Ok(b.tables()?.into_iter().map(|t| t.name).collect()))
    }

    /// Creates a table with one text column. 0028 replaces this with the full API.
    pub fn create_table(&self, name: &str) -> Result<(), XelError> {
        self.with(|b| {
            Ok(b.create_table(name, &[("Name", ColumnType::Text)])
                .map(|_| ())?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> String {
        let dir = std::env::temp_dir().join(format!("xel-ffi-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir is writable");
        let path = dir.join(name);
        let _ = std::fs::remove_file(&path);
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn create_reopen_and_read_back() {
        let path = temp_path("round-trip.xel");
        let book = Book::create(&path).unwrap();
        book.create_table("From Rust").unwrap();
        assert_eq!(book.format_version().unwrap(), xel_engine::FORMAT_VERSION);
        book.close().unwrap();
        let reopened = Book::open(&path).unwrap();
        assert_eq!(reopened.table_names().unwrap(), ["From Rust"]);
        reopened.close().unwrap();
    }

    #[test]
    fn errors_keep_the_kinds_the_app_reacts_to() {
        assert!(matches!(
            Book::open("/no/such/file.xel"),
            Err(XelError::NotFound { .. })
        ));
        assert!(matches!(
            Book::open("/etc/hosts"),
            Err(XelError::NotAWorkbook { .. })
        ));
        let path = temp_path("kinds.xel");
        let book = Book::create(&path).unwrap();
        assert!(matches!(
            book.create_table("xel_bad"),
            Err(XelError::Invalid { .. })
        ));
        book.close().unwrap();
    }

    #[test]
    fn a_closed_book_refuses_further_calls() {
        let path = temp_path("closed.xel");
        let book = Book::create(&path).unwrap();
        book.close().unwrap();
        assert_eq!(book.table_names(), Err(XelError::Closed));
        assert_eq!(book.close(), Err(XelError::Closed));
    }

    #[test]
    fn a_panic_poisons_the_book_instead_of_continuing() {
        let path = temp_path("poisoned.xel");
        let book = Book::create(&path).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<(), XelError> = book.with(|_| panic!("a bug inside an engine call"));
        }));
        assert!(result.is_err());
        assert_eq!(book.table_names(), Err(XelError::Poisoned));
    }
}
