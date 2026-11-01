use std::path::PathBuf;

/// Everything the engine can fail with. Callers match on the variant; the message is
/// for people.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("{} already exists", .0.display())]
    AlreadyExists(PathBuf),
    #[error("{} does not exist", .0.display())]
    NotFound(PathBuf),
    /// The file is not SQLite, or is another application's SQLite database.
    #[error("{} is not an xel workbook", .0.display())]
    NotAWorkbook(PathBuf),
    /// The file cannot be written, so edits would fail later.
    #[error("{} is read-only", .0.display())]
    ReadOnly(PathBuf),
    #[error(
        "this workbook uses format version {found}, but this version of xel supports up to \
         {supported}; update xel to open it"
    )]
    NewerFormat { found: u64, supported: u32 },
    /// The file is an xel workbook, but damaged.
    #[error("the workbook is damaged: {0}")]
    Corrupt(String),
    #[error("cannot access {}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
