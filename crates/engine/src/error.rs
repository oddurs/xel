use std::fmt;
use std::path::PathBuf;

/// Everything the engine can fail with. Callers match on the variant; the message is
/// for people.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The path already exists, so creating a workbook there would overwrite it.
    AlreadyExists(PathBuf),
    /// The file is not SQLite, or is SQLite without xel's metadata.
    NotAWorkbook(PathBuf),
    /// The file was written by a newer xel than this one.
    NewerFormat {
        found: u32,
        supported: u32,
    },
    /// The file's metadata is present but unreadable.
    CorruptMetadata(String),
    Sqlite(rusqlite::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyExists(path) => write!(f, "{} already exists", path.display()),
            Self::NotAWorkbook(path) => write!(f, "{} is not an xel workbook", path.display()),
            Self::NewerFormat { found, supported } => write!(
                f,
                "this workbook uses format version {found}, but this version of xel supports \
                 up to {supported}; update xel to open it"
            ),
            Self::CorruptMetadata(detail) => write!(f, "workbook metadata is corrupt: {detail}"),
            Self::Sqlite(e) => write!(f, "database error: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(e) => Some(e),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
