use std::path::PathBuf;

use crate::rows::RowId;
use crate::schema::{ColumnId, ColumnType, TableId};

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
    #[error("{name:?} is not a valid name: {reason}")]
    InvalidName { name: String, reason: String },
    #[error("the name {0:?} is already used")]
    DuplicateName(String),
    #[error("table {} does not exist", .0.raw())]
    NoSuchTable(TableId),
    #[error("column {} does not exist", .0.raw())]
    NoSuchColumn(ColumnId),
    #[error("a table needs at least one column")]
    NoColumns,
    #[error("a table must keep at least one column; delete the table instead")]
    LastColumn(ColumnId),
    #[error("column {} is not a select column", .0.raw())]
    NotASelectColumn(ColumnId),
    #[error("row {} of table {} does not exist", .0.raw(), .0.table().raw())]
    NoSuchRow(RowId),
    #[error("column {column:?} holds {expected} values, not {found}")]
    TypeMismatch {
        column: String,
        expected: ColumnType,
        found: &'static str,
    },
    #[error("column {column:?} cannot hold infinity or a value that is not a number")]
    NotFinite { column: String },
    #[error("{value:?} is not one of the options in column {column:?}")]
    NotAnOption { column: String, value: String },
    /// Another tool changed the file in a way an undo or redo cannot reconcile.
    #[error("the workbook was changed outside xel: {0}")]
    ChangedOutside(String),
    #[error("formula in column {column:?}: {error}")]
    Formula {
        column: String,
        error: crate::formula::ParseError,
    },
    /// Formula columns are added with `add_formula_column`, which takes the formula.
    #[error("column {0:?} needs a formula")]
    NeedsFormula(String),
    #[error("column {0:?} is computed by a formula and cannot be edited")]
    FormulaColumn(String),
    #[error(
        "the total of column {0:?} reads a single row; a total reads whole columns, such as [Amount]"
    )]
    TotalReadsRow(String),
    #[error("column {} is not a formula column", .0.raw())]
    NotAFormulaColumn(ColumnId),
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
