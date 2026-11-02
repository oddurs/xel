//! The xel engine for the macOS app, through `UniFFI`. Every type here is what Swift sees;
//! the engine itself stays free of FFI concerns.
//!
//! Every exported function returns `Result`: `UniFFI` catches a Rust panic and throws it
//! in Swift only for throwing functions, and crashes the app for the others.

// Every fallible function returns the one `XelError` type; `# Errors` sections would
// repeat it.
#![allow(clippy::missing_errors_doc)]

use std::sync::{Arc, Mutex, MutexGuard};

use xel_engine::{Aggregate, CellValue, ColumnId, ColumnType, Date, RowId, TableId, Workbook};

uniffi::setup_scaffolding!();

/// An engine error, as Swift receives it. The kinds the app reacts to differently are
/// separate cases; everything else carries the engine's message.
#[derive(Debug, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum XelError {
    #[error("{message}")]
    NotFound { message: String },
    #[error("{message}")]
    AlreadyExists { message: String },
    #[error("{message}")]
    NotAWorkbook { message: String },
    #[error("{message}")]
    NewerFormat { message: String },
    #[error("{message}")]
    ReadOnly { message: String },
    #[error("{message}")]
    Corrupt { message: String },
    /// Another tool changed the file in a way an undo or redo could not reconcile; that
    /// step is dropped from history. The app reloads what it shows.
    #[error("{message}")]
    ChangedOutside { message: String },
    /// A table, column, or row the app still showed is gone, after an undo for example.
    /// The app reloads what it shows.
    #[error("{message}")]
    Gone { message: String },
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
            E::AlreadyExists(_) => Self::AlreadyExists { message },
            E::NotAWorkbook(_) => Self::NotAWorkbook { message },
            E::NewerFormat { .. } => Self::NewerFormat { message },
            E::ReadOnly(_) => Self::ReadOnly { message },
            E::Corrupt(_) => Self::Corrupt { message },
            E::ChangedOutside(_) => Self::ChangedOutside { message },
            E::NoSuchTable(_) | E::NoSuchColumn(_) | E::NoSuchRow(_) => Self::Gone { message },
            E::InvalidName { .. }
            | E::DuplicateName(_)
            | E::NoColumns
            | E::LastColumn(_)
            | E::NotASelectColumn(_)
            | E::TypeMismatch { .. }
            | E::NotFinite { .. }
            | E::NotAnOption { .. }
            | E::Formula { .. }
            | E::NeedsFormula(_)
            | E::FormulaColumn(_)
            | E::NotAFormulaColumn(_)
            | E::NotATotal(_) => Self::Invalid { message },
            _ => Self::Other { message },
        }
    }
}

/// What a column holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ColumnKind {
    Text,
    Number,
    Checkbox,
    Date,
    Select,
    Formula,
}

impl From<ColumnType> for ColumnKind {
    fn from(ty: ColumnType) -> Self {
        match ty {
            ColumnType::Text => Self::Text,
            ColumnType::Number => Self::Number,
            ColumnType::Checkbox => Self::Checkbox,
            ColumnType::Date => Self::Date,
            ColumnType::Select => Self::Select,
            ColumnType::Formula => Self::Formula,
        }
    }
}

impl From<ColumnKind> for ColumnType {
    fn from(kind: ColumnKind) -> Self {
        match kind {
            ColumnKind::Text => Self::Text,
            ColumnKind::Number => Self::Number,
            ColumnKind::Checkbox => Self::Checkbox,
            ColumnKind::Date => Self::Date,
            ColumnKind::Select => Self::Select,
            ColumnKind::Formula => Self::Formula,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ColumnInfo {
    pub id: i64,
    pub name: String,
    pub kind: ColumnKind,
    /// A select column's choices, in order; empty for other kinds.
    pub options: Vec<String>,
    /// A formula column's formula, as written.
    pub formula: Option<String>,
    /// The formula of the column's total, if it has one; `Book::totals` has the results.
    pub total: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TableInfo {
    pub id: i64,
    pub name: String,
    pub columns: Vec<ColumnInfo>,
}

impl From<xel_engine::Table> for TableInfo {
    fn from(t: xel_engine::Table) -> Self {
        Self {
            id: t.id.raw(),
            name: t.name,
            columns: t
                .columns
                .into_iter()
                .map(|c| ColumnInfo {
                    id: c.id.raw(),
                    name: c.name,
                    kind: c.ty.into(),
                    options: c.options,
                    formula: c.formula,
                    total: c.total,
                })
                .collect(),
        }
    }
}

/// The totals a total row offers by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TotalKind {
    Sum,
    Average,
    Count,
    Min,
    Max,
}

impl From<TotalKind> for Aggregate {
    fn from(kind: TotalKind) -> Self {
        match kind {
            TotalKind::Sum => Self::Sum,
            TotalKind::Average => Self::Average,
            TotalKind::Count => Self::Count,
            TotalKind::Min => Self::Min,
            TotalKind::Max => Self::Max,
        }
    }
}

/// The total formula for `kind` over the column named `column`, as the total row's menu
/// sets it: `=SUM([Amount])`.
#[uniffi::export]
pub fn total_formula(kind: TotalKind, column: &str) -> Result<String, XelError> {
    Ok(Aggregate::from(kind).formula(column))
}

/// A column to create with a table. Formula columns are added afterwards, with
/// `add_formula_column`, as their formula may refer to these.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NewColumn {
    pub name: String,
    pub kind: ColumnKind,
}

/// One cell's value.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum Cell {
    Empty,
    Text {
        value: String,
    },
    Number {
        value: f64,
    },
    Checkbox {
        value: bool,
    },
    Date {
        year: u16,
        month: u8,
        day: u8,
    },
    /// A formula's error value, such as `#DIV/0!`. Only formulas produce these.
    Error {
        code: String,
    },
}

impl From<CellValue> for Cell {
    fn from(v: CellValue) -> Self {
        match v {
            CellValue::Empty => Self::Empty,
            CellValue::Text(value) => Self::Text { value },
            CellValue::Number(value) => Self::Number { value },
            CellValue::Checkbox(value) => Self::Checkbox { value },
            CellValue::Date(d) => Self::Date {
                year: d.year(),
                month: d.month(),
                day: d.day(),
            },
            CellValue::Error(e) => Self::Error {
                code: e.as_str().into(),
            },
        }
    }
}

impl TryFrom<Cell> for CellValue {
    type Error = XelError;

    fn try_from(cell: Cell) -> Result<Self, XelError> {
        Ok(match cell {
            Cell::Empty => Self::Empty,
            Cell::Text { value } => Self::Text(value),
            Cell::Number { value } => Self::Number(value),
            Cell::Checkbox { value } => Self::Checkbox(value),
            Cell::Date { year, month, day } => Self::Date(Date::new(year, month, day).ok_or_else(
                || XelError::Invalid {
                    message: format!("{year:04}-{month:02}-{day:02} is not a date xel can store"),
                },
            )?),
            Cell::Error { code } => {
                return Err(XelError::Invalid {
                    message: format!("{code} is a formula result and cannot be typed in"),
                });
            }
        })
    }
}

/// A row's identity. Row ids are numbered per table, so the table travels with the id:
/// an id held over from another table is refused rather than editing that table's row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Record)]
pub struct RowRef {
    pub table: i64,
    pub row: i64,
}

impl From<RowId> for RowRef {
    fn from(id: RowId) -> Self {
        Self {
            table: id.table().raw(),
            row: id.raw(),
        }
    }
}

impl From<RowRef> for RowId {
    fn from(r: RowRef) -> Self {
        RowId::new(TableId::from_raw(r.table), r.row)
    }
}

/// One row's cells, in the table's column order.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RowData {
    pub id: RowRef,
    pub cells: Vec<Cell>,
}

/// A value for one column of a new row.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ColumnValue {
    pub column: i64,
    pub value: Cell,
}

/// A value for one cell of an existing row.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CellEdit {
    pub row: RowRef,
    pub column: i64,
    pub value: Cell,
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

    /// Opens a private working copy of the document at `src`, written to `working`, which
    /// must not exist yet. The document is not touched until `save_copy` (see 0011).
    #[uniffi::constructor]
    pub fn open_copy(src: &str, working: &str) -> Result<Arc<Self>, XelError> {
        Ok(Self::new(Workbook::open_copy(src, working)?))
    }

    /// Writes a complete copy of the workbook to `dest`, which must not exist yet.
    pub fn save_copy(&self, dest: &str) -> Result<(), XelError> {
        self.with(|b| Ok(b.save_copy(dest)?))
    }

    /// Closes the file, dropping undo history and its trash, and reports failures that
    /// releasing the object would hide. Later calls fail with `Closed`. A poisoned book
    /// closes too: closing needs nothing from the history a panic may have broken.
    pub fn close(&self) -> Result<(), XelError> {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match guard.take() {
            Some(book) => Ok(book.close()?),
            None => Err(XelError::Closed),
        }
    }

    pub fn format_version(&self) -> Result<u32, XelError> {
        self.with(|b| Ok(b.format_version()))
    }

    pub fn tables(&self) -> Result<Vec<TableInfo>, XelError> {
        self.with(|b| Ok(b.tables()?.into_iter().map(Into::into).collect()))
    }

    pub fn table(&self, id: i64) -> Result<TableInfo, XelError> {
        self.with(|b| Ok(b.table(TableId::from_raw(id))?.into()))
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "UniFFI hands sequences over owned"
    )]
    pub fn create_table(&self, name: &str, columns: Vec<NewColumn>) -> Result<i64, XelError> {
        let columns: Vec<(&str, ColumnType)> = columns
            .iter()
            .map(|c| (c.name.as_str(), c.kind.into()))
            .collect();
        self.with(|b| Ok(b.create_table(name, &columns)?.raw()))
    }

    pub fn rename_table(&self, table: i64, name: &str) -> Result<(), XelError> {
        self.with(|b| Ok(b.rename_table(TableId::from_raw(table), name)?))
    }

    pub fn delete_table(&self, table: i64) -> Result<(), XelError> {
        self.with(|b| Ok(b.delete_table(TableId::from_raw(table))?))
    }

    pub fn add_column(&self, table: i64, name: &str, kind: ColumnKind) -> Result<i64, XelError> {
        self.with(|b| {
            Ok(b.add_column(TableId::from_raw(table), name, kind.into())?
                .raw())
        })
    }

    pub fn add_formula_column(
        &self,
        table: i64,
        name: &str,
        formula: &str,
    ) -> Result<i64, XelError> {
        self.with(|b| {
            Ok(
                b.add_formula_column(TableId::from_raw(table), name, formula)?
                    .raw(),
            )
        })
    }

    pub fn set_formula(&self, column: i64, formula: &str) -> Result<(), XelError> {
        self.with(|b| Ok(b.set_formula(ColumnId::from_raw(column), formula)?))
    }

    /// Sets a column's total formula, or removes the total with `None`.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "UniFFI hands optional strings over owned"
    )]
    pub fn set_total(&self, column: i64, formula: Option<String>) -> Result<(), XelError> {
        self.with(|b| Ok(b.set_total(ColumnId::from_raw(column), formula.as_deref())?))
    }

    /// Each column's total in column order; `None` for a column without one.
    pub fn totals(&self, table: i64) -> Result<Vec<Option<Cell>>, XelError> {
        self.with(|b| {
            Ok(b.totals(TableId::from_raw(table))?
                .into_iter()
                .map(|t| t.map(Into::into))
                .collect())
        })
    }

    pub fn rename_column(&self, column: i64, name: &str) -> Result<(), XelError> {
        self.with(|b| Ok(b.rename_column(ColumnId::from_raw(column), name)?))
    }

    pub fn delete_column(&self, column: i64) -> Result<(), XelError> {
        self.with(|b| Ok(b.delete_column(ColumnId::from_raw(column))?))
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "UniFFI hands sequences over owned"
    )]
    pub fn set_select_options(&self, column: i64, options: Vec<String>) -> Result<(), XelError> {
        let options: Vec<&str> = options.iter().map(String::as_str).collect();
        self.with(|b| Ok(b.set_select_options(ColumnId::from_raw(column), &options)?))
    }

    pub fn row_count(&self, table: i64) -> Result<u64, XelError> {
        self.with(|b| Ok(b.row_count(TableId::from_raw(table))?))
    }

    /// Up to `limit` rows of `table`, starting at the `offset`th in display order.
    pub fn rows(&self, table: i64, offset: u64, limit: u64) -> Result<Vec<RowData>, XelError> {
        self.with(|b| {
            Ok(b.rows(TableId::from_raw(table), offset, limit)?
                .into_iter()
                .map(|r| RowData {
                    id: r.id.into(),
                    cells: r.cells.into_iter().map(Into::into).collect(),
                })
                .collect())
        })
    }

    /// Adds a row at the end of `table`; columns not given are empty. Returns its id.
    pub fn insert_row(&self, table: i64, values: Vec<ColumnValue>) -> Result<RowRef, XelError> {
        let values = values
            .into_iter()
            .map(|v| Ok((ColumnId::from_raw(v.column), v.value.try_into()?)))
            .collect::<Result<Vec<_>, XelError>>()?;
        self.with(|b| Ok(b.insert_row(TableId::from_raw(table), &values)?.into()))
    }

    /// Sets cells of `table`, all or none, as one undo step.
    pub fn set_cells(&self, table: i64, edits: Vec<CellEdit>) -> Result<(), XelError> {
        let t = TableId::from_raw(table);
        let edits = edits
            .into_iter()
            .map(|e| {
                Ok((
                    ColumnId::from_raw(e.column),
                    e.row.into(),
                    e.value.try_into()?,
                ))
            })
            .collect::<Result<Vec<_>, XelError>>()?;
        self.with(|b| Ok(b.set_cells(t, &edits)?))
    }

    pub fn delete_rows(&self, table: i64, rows: Vec<RowRef>) -> Result<(), XelError> {
        let rows: Vec<RowId> = rows.into_iter().map(Into::into).collect();
        self.with(|b| Ok(b.delete_rows(TableId::from_raw(table), &rows)?))
    }

    /// Undoes the last edit; false when there is nothing to undo.
    pub fn undo(&self) -> Result<bool, XelError> {
        self.with(|b| Ok(b.undo()?))
    }

    /// Redoes the last undone edit; false when there is nothing to redo.
    pub fn redo(&self) -> Result<bool, XelError> {
        self.with(|b| Ok(b.redo()?))
    }

    pub fn can_undo(&self) -> Result<bool, XelError> {
        self.with(|b| Ok(b.can_undo()))
    }

    pub fn can_redo(&self) -> Result<bool, XelError> {
        self.with(|b| Ok(b.can_redo()))
    }

    /// Panics inside a call on purpose, so the app's tests can prove a Rust panic
    /// reaches Swift as a thrown error and poisons this book, rather than crashing the
    /// app. The app never calls it.
    ///
    /// # Panics
    ///
    /// Always.
    pub fn check_panic_boundary(&self) -> Result<(), XelError> {
        self.with(|_| panic!("check_panic_boundary panics on purpose"))
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
        book.create_table(
            "From Rust",
            vec![NewColumn {
                name: "Name".into(),
                kind: ColumnKind::Text,
            }],
        )
        .unwrap();
        assert_eq!(book.format_version().unwrap(), xel_engine::FORMAT_VERSION);
        book.close().unwrap();
        let reopened = Book::open(&path).unwrap();
        assert_eq!(reopened.tables().unwrap()[0].name, "From Rust");
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
            book.create_table("xel_bad", vec![]),
            Err(XelError::Invalid { .. })
        ));
        assert!(matches!(book.table(999), Err(XelError::Gone { .. })));
        book.close().unwrap();
    }

    #[test]
    fn a_closed_book_refuses_further_calls() {
        let path = temp_path("closed.xel");
        let book = Book::create(&path).unwrap();
        book.close().unwrap();
        assert_eq!(book.tables().map(|_| ()), Err(XelError::Closed));
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
        assert_eq!(book.tables().map(|_| ()), Err(XelError::Poisoned));
    }
}
