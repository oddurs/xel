//! Tables and columns. Each xel table is a real SQL table under the user's own name, so
//! any SQLite tool can read it; xel's extra knowledge (column types, select options,
//! order) lives in the `xel_tables` and `xel_columns` metadata tables.

use std::fmt;

use rusqlite::{Connection, OptionalExtension, params};

use crate::history::{Change, ColumnSpec, TableSpec, next_id};

use crate::error::{Error, Result};
use crate::workbook::Workbook;

/// Metadata tables, created with every new workbook.
pub(crate) const SCHEMA: &str = "
CREATE TABLE xel_tables (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    position INTEGER NOT NULL
) STRICT;
CREATE TABLE xel_columns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    table_id INTEGER NOT NULL REFERENCES xel_tables (id) ON DELETE CASCADE,
    name TEXT NOT NULL COLLATE NOCASE,
    type TEXT NOT NULL CHECK (type IN ('text', 'number', 'checkbox', 'date', 'select', 'formula')),
    position INTEGER NOT NULL,
    -- The formula text of a formula column; NULL for every other type.
    formula TEXT CHECK ((type = 'formula') = (formula IS NOT NULL)),
    -- 'date' when a formula column's results are dates, so its ISO text reads back as dates.
    formula_kind TEXT CHECK (formula_kind IS NULL OR formula_kind = 'date'),
    UNIQUE (table_id, name)
) STRICT;
CREATE TABLE xel_select_options (
    column_id INTEGER NOT NULL REFERENCES xel_columns (id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    value TEXT NOT NULL COLLATE NOCASE,
    PRIMARY KEY (column_id, position),
    UNIQUE (column_id, value)
) STRICT;
";

/// The longest table or column name accepted, in characters.
const MAX_NAME: usize = 255;

/// The row identity column every user table starts with. An explicit INTEGER PRIMARY
/// KEY keeps row ids stable: SQLite's VACUUM may renumber an implicit rowid.
pub(crate) const ROW_ID: &str = "_id";

/// Column names users cannot take: xel's row id, and the names SQLite gives the
/// implicit row id, which SQLite refuses to rename or drop later.
const RESERVED_COLUMNS: [&str; 4] = [ROW_ID, "rowid", "oid", "_rowid_"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TableId(pub(crate) i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ColumnId(pub(crate) i64);

// The raw ids let a caller across a language boundary hold on to tables and columns. Any
// id is safe to pass back: the engine looks it up and reports one that does not exist.
impl TableId {
    #[must_use]
    pub const fn from_raw(id: i64) -> Self {
        Self(id)
    }

    #[must_use]
    pub const fn raw(self) -> i64 {
        self.0
    }
}

impl ColumnId {
    #[must_use]
    pub const fn from_raw(id: i64) -> Self {
        Self(id)
    }

    #[must_use]
    pub const fn raw(self) -> i64 {
        self.0
    }
}

/// What a column holds, and so how it is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColumnType {
    /// `TEXT`.
    Text,
    /// `REAL`, an IEEE double (see 0009).
    Number,
    /// `INTEGER`, 0 or 1.
    Checkbox,
    /// `TEXT` holding an ISO 8601 date, `YYYY-MM-DD`.
    Date,
    /// `TEXT` holding one of the column's options.
    Select,
    /// Computed from a formula; `ANY`, as results can be of any type (see 0010).
    Formula,
}

impl ColumnType {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Number => "number",
            Self::Checkbox => "checkbox",
            Self::Date => "date",
            Self::Select => "select",
            Self::Formula => "formula",
        }
    }

    pub(crate) fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "text" => Self::Text,
            "number" => Self::Number,
            "checkbox" => Self::Checkbox,
            "date" => Self::Date,
            "select" => Self::Select,
            "formula" => Self::Formula,
            other => return Err(Error::Corrupt(format!("unknown column type {other:?}"))),
        })
    }

    /// The column definition in the user table. STRICT tables enforce the storage
    /// class; the CHECKs keep external writers to the values xel can show.
    pub(crate) fn sql(self, quoted: &str) -> String {
        match self {
            Self::Text | Self::Select => format!("{quoted} TEXT"),
            Self::Number => format!("{quoted} REAL"),
            Self::Formula => format!("{quoted} ANY"),
            Self::Checkbox => format!("{quoted} INTEGER CHECK ({quoted} IN (0, 1))"),
            Self::Date => format!(
                // `IS`, not `=`: date() is NULL for nonsense like 2024-00-01, and a CHECK
                // that evaluates to NULL passes.
                "{quoted} TEXT CHECK ({quoted} GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]' \
                 AND date({quoted}) IS {quoted} \
                 AND {quoted} BETWEEN '1900-01-01' AND '9999-12-31')"
            ),
        }
    }
}

impl fmt::Display for ColumnType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub id: ColumnId,
    pub name: String,
    pub ty: ColumnType,
    /// Choices for a `Select` column, in display order; empty for other types.
    pub options: Vec<String>,
    /// The formula of a `Formula` column, as written; `None` for other types.
    pub formula: Option<String>,
    /// Whether a formula column's results are dates (see `recalc`).
    pub(crate) formula_dates: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub id: TableId,
    pub name: String,
    pub columns: Vec<Column>,
}

impl Workbook {
    /// Every table with its columns, in display order.
    pub fn tables(&self) -> Result<Vec<Table>> {
        load_tables(&self.conn, None)
    }

    pub fn table(&self, id: TableId) -> Result<Table> {
        load_tables(&self.conn, Some(id))?
            .pop()
            .ok_or(Error::NoSuchTable(id))
    }

    /// Creates a table with at least one column. SQLite has no column-less tables.
    pub fn create_table(&mut self, name: &str, columns: &[(&str, ColumnType)]) -> Result<TableId> {
        check_table_name(name)?;
        if columns.is_empty() {
            return Err(Error::NoColumns);
        }
        for (i, (col, ty)) in columns.iter().enumerate() {
            check_column_name(col)?;
            if *ty == ColumnType::Formula {
                return Err(Error::NeedsFormula((*col).into()));
            }
            // SQLite identifiers are case-insensitive for ASCII only.
            if columns[..i]
                .iter()
                .any(|(c, _)| c.eq_ignore_ascii_case(col))
            {
                return Err(Error::DuplicateName((*col).into()));
            }
        }
        self.edit(|tx| {
            ensure_table_name_free(tx, name)?;
            let id = TableId(next_id(tx, "xel_tables")?);
            let first_column = next_id(tx, "xel_columns")?;
            let position: i64 = tx.query_row(
                "SELECT coalesce(max(position), -1) + 1 FROM xel_tables",
                [],
                |r| r.get(0),
            )?;
            let columns = (0_i64..)
                .zip(columns)
                .map(|(i, (col, ty))| ColumnSpec {
                    id: ColumnId(first_column + i),
                    name: (*col).into(),
                    ty: *ty,
                    position: i,
                    options: Vec::new(),
                    formula: None,
                })
                .collect();
            let spec = TableSpec {
                id,
                name: name.into(),
                position,
                columns,
            };
            Ok((
                vec![Change::CreateTable {
                    spec,
                    from_trash: false,
                }],
                id,
            ))
        })
    }

    pub fn rename_table(&mut self, id: TableId, new_name: &str) -> Result<()> {
        check_table_name(new_name)?;
        self.edit(|tx| {
            let old = table_name(tx, id)?;
            if old == new_name {
                return Ok((vec![], ()));
            }
            if !old.eq_ignore_ascii_case(new_name) {
                ensure_table_name_free(tx, new_name)?;
            }
            Ok((
                vec![Change::RenameTable {
                    id,
                    from: old,
                    to: new_name.into(),
                }],
                (),
            ))
        })
    }

    /// Deletes a table. Undo brings it back with its rows.
    pub fn delete_table(&mut self, id: TableId) -> Result<()> {
        self.edit(|tx| {
            Ok((
                vec![Change::DeleteTable {
                    spec: table_spec(tx, id)?,
                }],
                (),
            ))
        })
    }

    /// Adds a column at the end of the table.
    pub fn add_column(&mut self, table: TableId, name: &str, ty: ColumnType) -> Result<ColumnId> {
        if ty == ColumnType::Formula {
            return Err(Error::NeedsFormula(name.into()));
        }
        self.append_column(table, name, ty, None)
    }

    /// Adds a column computed by a formula, such as `=[@Qty]*[@Price]`, and computes it
    /// for every row.
    pub fn add_formula_column(
        &mut self,
        table: TableId,
        name: &str,
        formula: &str,
    ) -> Result<ColumnId> {
        check_formula(name, formula)?;
        self.append_column(table, name, ColumnType::Formula, Some(formula.into()))
    }

    fn append_column(
        &mut self,
        table: TableId,
        name: &str,
        ty: ColumnType,
        formula: Option<String>,
    ) -> Result<ColumnId> {
        check_column_name(name)?;
        self.edit(|tx| {
            table_name(tx, table)?;
            ensure_column_name_free(tx, table, name, None)?;
            let id = ColumnId(next_id(tx, "xel_columns")?);
            let position: i64 = tx.query_row(
                "SELECT coalesce(max(position), -1) + 1 FROM xel_columns WHERE table_id = ?1",
                [table.0],
                |r| r.get(0),
            )?;
            let spec = ColumnSpec {
                id,
                name: name.into(),
                ty,
                position,
                options: Vec::new(),
                formula,
            };
            Ok((
                vec![Change::AddColumn {
                    table,
                    spec,
                    from_trash: false,
                }],
                id,
            ))
        })
    }

    /// Replaces a formula column's formula and recomputes the column.
    pub fn set_formula(&mut self, column: ColumnId, formula: &str) -> Result<()> {
        self.edit(|tx| {
            let spec = column_spec(tx, column)?;
            let Some(from) = spec.formula else {
                return Err(Error::NotAFormulaColumn(column));
            };
            check_formula(&spec.name, formula)?;
            if from == formula {
                return Ok((vec![], ()));
            }
            let table = column_info(tx, column)?.table;
            Ok((
                vec![Change::SetFormula {
                    table,
                    column,
                    from,
                    to: formula.into(),
                }],
                (),
            ))
        })
    }

    pub fn rename_column(&mut self, id: ColumnId, new_name: &str) -> Result<()> {
        check_column_name(new_name)?;
        self.edit(|tx| {
            let info = column_info(tx, id)?;
            if info.name == new_name {
                return Ok((vec![], ()));
            }
            ensure_column_name_free(tx, info.table, new_name, Some(id))?;
            let change = Change::RenameColumn {
                table: info.table,
                id,
                from: info.name,
                to: new_name.into(),
            };
            Ok((vec![change], ()))
        })
    }

    /// Deletes a column and its data; undo brings both back. A table's last column cannot
    /// be deleted; delete the table instead.
    pub fn delete_column(&mut self, id: ColumnId) -> Result<()> {
        self.edit(|tx| {
            let info = column_info(tx, id)?;
            let count: i64 = tx.query_row(
                "SELECT count(*) FROM xel_columns WHERE table_id = ?1",
                [info.table.0],
                |r| r.get(0),
            )?;
            if count == 1 {
                return Err(Error::LastColumn(id));
            }
            let spec = column_spec(tx, id)?;
            Ok((
                vec![Change::DeleteColumn {
                    table: info.table,
                    spec,
                }],
                (),
            ))
        })
    }

    /// Replaces a select column's options. Values already stored are not changed.
    pub fn set_select_options(&mut self, id: ColumnId, options: &[&str]) -> Result<()> {
        for (i, option) in options.iter().enumerate() {
            check_name(option)?;
            if options[..i].iter().any(|o| o.eq_ignore_ascii_case(option)) {
                return Err(Error::DuplicateName((*option).into()));
            }
        }
        self.edit(|tx| {
            if column_info(tx, id)?.ty != ColumnType::Select {
                return Err(Error::NotASelectColumn(id));
            }
            let from = column_spec(tx, id)?.options;
            let to: Vec<String> = options.iter().map(|o| (*o).to_string()).collect();
            if from == to {
                return Ok((vec![], ()));
            }
            Ok((
                vec![Change::SetOptions {
                    column: id,
                    from,
                    to,
                }],
                (),
            ))
        })
    }
}

/// Everything needed to recreate a table exactly.
fn table_spec(conn: &Connection, id: TableId) -> Result<TableSpec> {
    let (name, position): (String, i64) = conn
        .query_row(
            "SELECT name, position FROM xel_tables WHERE id = ?1",
            [id.0],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or(Error::NoSuchTable(id))?;
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM xel_columns WHERE table_id = ?1 ORDER BY position, id")?
        .query_map([id.0], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let columns = ids
        .into_iter()
        .map(|c| column_spec(conn, ColumnId(c)))
        .collect::<Result<_>>()?;
    Ok(TableSpec {
        id,
        name,
        position,
        columns,
    })
}

/// Everything needed to recreate a column exactly.
fn column_spec(conn: &Connection, id: ColumnId) -> Result<ColumnSpec> {
    let (name, ty, position, formula): (String, String, i64, Option<String>) = conn
        .query_row(
            "SELECT name, type, position, formula FROM xel_columns WHERE id = ?1",
            [id.0],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?
        .ok_or(Error::NoSuchColumn(id))?;
    let options = conn
        .prepare("SELECT value FROM xel_select_options WHERE column_id = ?1 ORDER BY position")?
        .query_map([id.0], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(ColumnSpec {
        id,
        name,
        ty: ColumnType::parse(&ty)?,
        position,
        options,
        formula,
    })
}

/// One table with its columns and options.
pub(crate) fn load_table(conn: &Connection, id: TableId) -> Result<Table> {
    load_tables(conn, Some(id))?
        .pop()
        .ok_or(Error::NoSuchTable(id))
}

/// Loads every table, or just `only`, with columns and options in one query.
fn load_tables(conn: &Connection, only: Option<TableId>) -> Result<Vec<Table>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, c.id, c.name, c.type, o.value, c.formula, c.formula_kind
         FROM xel_tables t
         LEFT JOIN xel_columns c ON c.table_id = t.id
         LEFT JOIN xel_select_options o ON o.column_id = c.id
         WHERE ?1 IS NULL OR t.id = ?1
         ORDER BY t.position, t.id, c.position, c.id, o.position",
    )?;
    let mut rows = stmt.query([only.map(|t| t.0)])?;
    let mut tables: Vec<Table> = Vec::new();
    while let Some(r) = rows.next()? {
        let table = TableId(r.get(0)?);
        if tables.last().is_none_or(|t| t.id != table) {
            tables.push(Table {
                id: table,
                name: r.get(1)?,
                columns: Vec::new(),
            });
        }
        let Some(col) = r.get::<_, Option<i64>>(2)? else {
            continue;
        };
        let columns = &mut tables.last_mut().expect("pushed above").columns;
        if columns.last().is_none_or(|c| c.id.0 != col) {
            columns.push(Column {
                id: ColumnId(col),
                name: r.get(3)?,
                ty: ColumnType::parse(&r.get::<_, String>(4)?)?,
                options: Vec::new(),
                formula: r.get(6)?,
                formula_dates: r.get::<_, Option<String>>(7)?.as_deref() == Some("date"),
            });
        }
        if let Some(option) = r.get::<_, Option<String>>(5)? {
            columns
                .last_mut()
                .expect("pushed above")
                .options
                .push(option);
        }
    }
    Ok(tables)
}

/// Quotes an identifier for SQL, doubling embedded quotes.
pub(crate) fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn check_name(name: &str) -> Result<()> {
    let invalid = |reason: &str| {
        Err(Error::InvalidName {
            name: name.into(),
            reason: reason.into(),
        })
    };
    if name.trim().is_empty() {
        return invalid("a name cannot be empty");
    }
    if name.trim() != name {
        return invalid("a name cannot start or end with spaces");
    }
    if name.chars().count() > MAX_NAME {
        return invalid("a name can be at most 255 characters");
    }
    if name.chars().any(char::is_control) {
        return invalid("a name cannot contain control characters");
    }
    Ok(())
}

/// A formula must parse; references to columns that do not exist are allowed and show
/// `#REF!`, as in Excel.
fn check_formula(column: &str, formula: &str) -> Result<()> {
    crate::formula::parse(formula)
        .map(|_| ())
        .map_err(|error| Error::Formula {
            column: column.into(),
            error,
        })
}

fn check_column_name(name: &str) -> Result<()> {
    check_name(name)?;
    if RESERVED_COLUMNS
        .iter()
        .any(|r| r.eq_ignore_ascii_case(name))
    {
        return Err(Error::InvalidName {
            name: name.into(),
            reason: "_id, rowid, oid, and _rowid_ are reserved".into(),
        });
    }
    Ok(())
}

fn check_table_name(name: &str) -> Result<()> {
    check_name(name)?;
    let lower = name.to_ascii_lowercase();
    if lower.starts_with("sqlite_") || lower.starts_with("xel_") {
        return Err(Error::InvalidName {
            name: name.into(),
            reason: "table names cannot start with sqlite_ or xel_".into(),
        });
    }
    Ok(())
}

pub(crate) fn table_name(conn: &Connection, id: TableId) -> Result<String> {
    conn.query_row("SELECT name FROM xel_tables WHERE id = ?1", [id.0], |r| {
        r.get(0)
    })
    .optional()?
    .ok_or(Error::NoSuchTable(id))
}

struct ColumnInfo {
    table: TableId,
    name: String,
    ty: ColumnType,
}

fn column_info(conn: &Connection, id: ColumnId) -> Result<ColumnInfo> {
    let (table, name, ty): (i64, String, String) = conn
        .query_row(
            "SELECT table_id, name, type FROM xel_columns WHERE id = ?1",
            [id.0],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?
        .ok_or(Error::NoSuchColumn(id))?;
    Ok(ColumnInfo {
        table: TableId(table),
        name,
        ty: ColumnType::parse(&ty)?,
    })
}

/// Table names share SQLite's namespace with every other schema object, so check all of
/// them, case-insensitively for ASCII as SQLite does.
fn ensure_table_name_free(conn: &Connection, name: &str) -> Result<()> {
    let taken: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_schema WHERE lower(name) = lower(?1))",
        [name],
        |r| r.get(0),
    )?;
    if taken {
        Err(Error::DuplicateName(name.into()))
    } else {
        Ok(())
    }
}

fn ensure_column_name_free(
    conn: &Connection,
    table: TableId,
    name: &str,
    except: Option<ColumnId>,
) -> Result<()> {
    let taken: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM xel_columns \
         WHERE table_id = ?1 AND name = ?2 AND id IS NOT ?3)",
        params![table.0, name, except.map(|c| c.0)],
        |r| r.get(0),
    )?;
    if taken {
        Err(Error::DuplicateName(name.into()))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::test_support::TempDir;

    fn book(dir: &TempDir) -> (Workbook, std::path::PathBuf) {
        let path = dir.path().join("book.xel");
        (Workbook::create(&path).unwrap(), path)
    }

    /// `(name, declared type)` of each column, as plain SQLite sees the table.
    fn sql_columns(path: &std::path::Path, table: &str) -> Vec<(String, String)> {
        let raw = Connection::open(path).unwrap();
        let mut stmt = raw
            .prepare("SELECT name, type FROM pragma_table_info(?1) WHERE name <> '_id'")
            .unwrap();
        stmt.query_map([table], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn tables_are_real_sql_tables_under_their_own_names() {
        let dir = TempDir::new();
        let (mut book, path) = book(&dir);
        book.create_table(
            "Expenses",
            &[
                ("Item", ColumnType::Text),
                ("Amount", ColumnType::Number),
                ("Paid", ColumnType::Checkbox),
                ("When", ColumnType::Date),
                ("Category", ColumnType::Select),
            ],
        )
        .unwrap();
        assert_eq!(
            sql_columns(&path, "Expenses"),
            [
                ("Item", "TEXT"),
                ("Amount", "REAL"),
                ("Paid", "INTEGER"),
                ("When", "TEXT"),
                ("Category", "TEXT")
            ]
            .map(|(a, b)| (a.to_string(), b.to_string()))
        );
        let t = &book.tables().unwrap()[0];
        assert_eq!(t.name, "Expenses");
        let types: Vec<_> = t.columns.iter().map(|c| c.ty).collect();
        assert_eq!(
            types,
            [
                ColumnType::Text,
                ColumnType::Number,
                ColumnType::Checkbox,
                ColumnType::Date,
                ColumnType::Select
            ]
        );
    }

    #[test]
    fn every_table_has_a_stable_integer_row_id() {
        let dir = TempDir::new();
        let (mut book, path) = book(&dir);
        book.create_table("T", &[("a", ColumnType::Text)]).unwrap();
        book.close().unwrap();
        let raw = Connection::open(&path).unwrap();
        let pk: (String, String, i64) = raw
            .query_row(
                "SELECT name, type, pk FROM pragma_table_info('T') WHERE pk = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(pk, ("_id".into(), "INTEGER".into(), 1));
        // VACUUM renumbers implicit rowids but never an INTEGER PRIMARY KEY.
        raw.execute_batch(
            "INSERT INTO T (a) VALUES ('x'), ('y'), ('z'); DELETE FROM T WHERE a = 'x'; VACUUM;",
        )
        .unwrap();
        let ids: Vec<i64> = raw
            .prepare("SELECT _id FROM T ORDER BY _id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(ids, [2, 3]);
    }

    #[test]
    fn storage_of_each_type_is_what_plain_sqlite_sees() {
        let dir = TempDir::new();
        let (mut book, path) = book(&dir);
        book.create_table(
            "T",
            &[
                ("n", ColumnType::Number),
                ("c", ColumnType::Checkbox),
                ("d", ColumnType::Date),
            ],
        )
        .unwrap();
        book.close().unwrap();
        let raw = Connection::open(&path).unwrap();
        raw.execute("INSERT INTO T (n, c, d) VALUES (1.5, 1, '2024-02-29')", [])
            .unwrap();
        let row: (String, String, String) = raw
            .query_row("SELECT typeof(n), typeof(c), typeof(d) FROM T", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!(row, ("real".into(), "integer".into(), "text".into()));
        // The constraints keep external writers to values xel can show.
        for bad in [
            "INSERT INTO T (c) VALUES (2)",
            "INSERT INTO T (d) VALUES ('2024-02-30')",
            "INSERT INTO T (d) VALUES ('29/02/2024')",
            "INSERT INTO T (n) VALUES ('abc')",
        ] {
            assert!(raw.execute(bad, []).is_err(), "{bad} was accepted");
        }
    }

    #[test]
    fn renaming_keeps_the_data() {
        let dir = TempDir::new();
        let (mut book, path) = book(&dir);
        let t = book
            .create_table("Old", &[("a", ColumnType::Number)])
            .unwrap();
        book.conn
            .execute("INSERT INTO \"Old\" (a) VALUES (42)", [])
            .unwrap();
        let col = book.tables().unwrap()[0].columns[0].id;

        book.rename_table(t, "New").unwrap();
        book.rename_column(col, "b").unwrap();
        book.close().unwrap();

        let raw = Connection::open(&path).unwrap();
        let v: f64 = raw
            .query_row("SELECT b FROM New", [], |r| r.get(0))
            .unwrap();
        assert!((v - 42.0).abs() < f64::EPSILON);
        let book = Workbook::open(&path).unwrap();
        let t = &book.tables().unwrap()[0];
        assert_eq!((t.name.as_str(), t.columns[0].name.as_str()), ("New", "b"));
    }

    #[test]
    fn renamed_columns_keep_their_constraints() {
        let dir = TempDir::new();
        let (mut book, path) = book(&dir);
        let t = book
            .create_table(
                "T",
                &[("Done", ColumnType::Checkbox), ("Due", ColumnType::Date)],
            )
            .unwrap();
        let cols = book.table(t).unwrap().columns;
        book.rename_column(cols[0].id, "Finished").unwrap();
        book.rename_column(cols[1].id, "Deadline").unwrap();
        book.close().unwrap();
        let raw = Connection::open(&path).unwrap();
        assert!(
            raw.execute("INSERT INTO T (Finished) VALUES (7)", [])
                .is_err()
        );
        assert!(
            raw.execute("INSERT INTO T (Deadline) VALUES ('soon')", [])
                .is_err()
        );
        raw.execute(
            "INSERT INTO T (Finished, Deadline) VALUES (1, '2026-01-31')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn awkward_names_round_trip() {
        let dir = TempDir::new();
        let (mut book, path) = book(&dir);
        for name in [
            "My Table",
            "say \"hi\"",
            "ÞÆÖ ✓",
            "a]b[c",
            "select",
            "Robert'); DROP TABLE x;--",
        ] {
            let t = book
                .create_table(name, &[(name, ColumnType::Text)])
                .unwrap();
            let table = book.table(t).unwrap();
            assert_eq!(table.name, name);
            assert_eq!(table.columns[0].name, name);
            assert_eq!(sql_columns(&path, name)[0].0, name);
        }
        book.close().unwrap();
        let names: Vec<String> = Workbook::open(&path)
            .unwrap()
            .tables()
            .unwrap()
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(names.len(), 6);
    }

    #[test]
    fn invalid_and_duplicate_names_are_refused() {
        let dir = TempDir::new();
        let (mut book, _) = book(&dir);
        let t = book
            .create_table("Orders", &[("Id", ColumnType::Number)])
            .unwrap();
        for bad in [
            "",
            "  ",
            " lead",
            "trail ",
            "xel_meta",
            "SQLITE_x",
            "tab\there",
        ] {
            assert!(
                matches!(
                    book.create_table(bad, &[("a", ColumnType::Text)]),
                    Err(Error::InvalidName { .. })
                ),
                "{bad:?}"
            );
        }
        assert!(matches!(
            book.create_table("orders", &[("a", ColumnType::Text)]),
            Err(Error::DuplicateName(_))
        ));
        assert!(matches!(
            book.create_table("T", &[("a", ColumnType::Text), ("A", ColumnType::Text)]),
            Err(Error::DuplicateName(_))
        ));
        assert!(matches!(
            book.add_column(t, "ID", ColumnType::Text),
            Err(Error::DuplicateName(_))
        ));
        assert!(matches!(
            book.create_table("Empty", &[]),
            Err(Error::NoColumns)
        ));
        // Renaming to a different case of the same name is allowed.
        book.rename_table(t, "ORDERS").unwrap();
        assert_eq!(book.table(t).unwrap().name, "ORDERS");
    }

    #[test]
    fn rowid_aliases_are_not_column_names() {
        let dir = TempDir::new();
        let (mut book, _) = book(&dir);
        for alias in ["_id", "rowid", "ROWID", "oid", "_rowid_"] {
            assert!(
                matches!(
                    book.create_table("T", &[(alias, ColumnType::Text)]),
                    Err(Error::InvalidName { .. })
                ),
                "{alias}"
            );
        }
        let t = book.create_table("T", &[("a", ColumnType::Text)]).unwrap();
        let a = book.table(t).unwrap().columns[0].id;
        assert!(matches!(
            book.add_column(t, "oid", ColumnType::Text),
            Err(Error::InvalidName { .. })
        ));
        assert!(matches!(
            book.rename_column(a, "rowid"),
            Err(Error::InvalidName { .. })
        ));
    }

    #[test]
    fn renaming_to_the_same_name_is_a_no_op() {
        let dir = TempDir::new();
        let (mut book, _) = book(&dir);
        let t = book
            .create_table("Orders", &[("Id", ColumnType::Number)])
            .unwrap();
        let id = book.table(t).unwrap().columns[0].id;
        book.rename_table(t, "Orders").unwrap();
        book.rename_column(id, "Id").unwrap();
        assert_eq!(book.table(t).unwrap().name, "Orders");
    }

    #[test]
    fn a_reader_holding_the_file_delays_a_write_instead_of_failing_it() {
        let dir = TempDir::new();
        let (mut book, path) = book(&dir);
        let reader = Connection::open(&path).unwrap();
        reader
            .execute_batch("BEGIN; SELECT count(*) FROM xel_tables;")
            .unwrap();
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            reader.execute_batch("COMMIT").unwrap();
        });
        book.create_table("T", &[("a", ColumnType::Text)]).unwrap();
        releaser.join().unwrap();
    }

    #[test]
    fn tables_load_with_columns_and_options_in_order() {
        let dir = TempDir::new();
        let (mut book, _) = book(&dir);
        let a = book
            .create_table("A", &[("x", ColumnType::Select), ("y", ColumnType::Text)])
            .unwrap();
        book.create_table("B", &[("z", ColumnType::Number)])
            .unwrap();
        let x = book.table(a).unwrap().columns[0].id;
        book.set_select_options(x, &["c", "a", "b"]).unwrap();
        let shape: Vec<String> = book
            .tables()
            .unwrap()
            .iter()
            .map(|t| {
                let cols: Vec<String> = t
                    .columns
                    .iter()
                    .map(|c| format!("{}{:?}", c.name, c.options))
                    .collect();
                format!("{}: {}", t.name, cols.join(" "))
            })
            .collect();
        assert_eq!(shape, ["A: x[\"c\", \"a\", \"b\"] y[]", "B: z[]"]);
    }

    #[test]
    fn tied_positions_do_not_split_tables_or_columns() {
        let dir = TempDir::new();
        let (mut book, _) = book(&dir);
        let t = book
            .create_table("U", &[("p", ColumnType::Select), ("q", ColumnType::Select)])
            .unwrap();
        book.create_table("V", &[("r", ColumnType::Text)]).unwrap();
        let cols = book.table(t).unwrap().columns;
        book.set_select_options(cols[0].id, &["p1", "p2"]).unwrap();
        book.set_select_options(cols[1].id, &["q1", "q2"]).unwrap();
        // Ties can only come from an outside edit today; the loader must still cope.
        book.conn
            .execute_batch(
                "UPDATE xel_columns SET position = 0; UPDATE xel_tables SET position = 0;",
            )
            .unwrap();
        let tables = book.tables().unwrap();
        assert_eq!(tables.len(), 2);
        let u = tables.iter().find(|t| t.name == "U").unwrap();
        let shape: Vec<_> = u
            .columns
            .iter()
            .map(|c| (c.name.as_str(), c.options.len()))
            .collect();
        assert_eq!(shape, [("p", 2), ("q", 2)]);
    }

    #[test]
    fn columns_can_be_added_and_deleted_but_not_the_last() {
        let dir = TempDir::new();
        let (mut book, path) = book(&dir);
        let t = book.create_table("T", &[("a", ColumnType::Text)]).unwrap();
        for ty in [
            ColumnType::Number,
            ColumnType::Checkbox,
            ColumnType::Date,
            ColumnType::Select,
        ] {
            let b = book.add_column(t, "b", ty).unwrap();
            assert_eq!(sql_columns(&path, "T").len(), 2, "{ty}");
            book.delete_column(b).unwrap();
            assert_eq!(sql_columns(&path, "T").len(), 1, "{ty}");
        }
        let a = book.table(t).unwrap().columns[0].id;
        assert!(matches!(book.delete_column(a), Err(Error::LastColumn(_))));
    }

    #[test]
    fn deleting_a_table_removes_it_and_its_metadata() {
        let dir = TempDir::new();
        let (mut book, path) = book(&dir);
        let t = book
            .create_table("T", &[("a", ColumnType::Select)])
            .unwrap();
        let a = book.table(t).unwrap().columns[0].id;
        book.set_select_options(a, &["x", "y"]).unwrap();
        book.delete_table(t).unwrap();
        assert!(book.tables().unwrap().is_empty());
        assert!(sql_columns(&path, "T").is_empty());
        let leftovers: i64 = book
            .conn
            .query_row(
                "SELECT (SELECT count(*) FROM xel_columns) + (SELECT count(*) FROM xel_select_options)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(leftovers, 0);
        assert!(matches!(book.table(t), Err(Error::NoSuchTable(_))));
    }

    #[test]
    fn select_options_are_ordered_and_unique() {
        let dir = TempDir::new();
        let (mut book, _) = book(&dir);
        let t = book
            .create_table("T", &[("s", ColumnType::Select), ("n", ColumnType::Number)])
            .unwrap();
        let cols = book.table(t).unwrap().columns;
        book.set_select_options(cols[0].id, &["High", "Low", "Medium"])
            .unwrap();
        assert_eq!(
            book.table(t).unwrap().columns[0].options,
            ["High", "Low", "Medium"]
        );
        for bad in [&["a", "A"][..], &[""], &["  x"], &["a\nb"]] {
            assert!(book.set_select_options(cols[0].id, bad).is_err(), "{bad:?}");
        }
        assert_eq!(
            book.table(t).unwrap().columns[0].options,
            ["High", "Low", "Medium"],
            "a failed update changes nothing"
        );
        assert!(matches!(
            book.set_select_options(cols[1].id, &["a"]),
            Err(Error::NotASelectColumn(_))
        ));
    }
}
