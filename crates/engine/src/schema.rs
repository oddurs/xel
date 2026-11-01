//! Tables and columns. Each xel table is a real SQL table under the user's own name, so
//! any SQLite tool can read it; xel's extra knowledge (column types, select options,
//! order) lives in the `xel_tables` and `xel_columns` metadata tables.

use std::fmt;

use rusqlite::{OptionalExtension, Transaction, params};

use crate::error::{Error, Result};
use crate::workbook::Workbook;

/// Metadata tables, created with every new workbook.
pub(crate) const SCHEMA: &str = "
CREATE TABLE xel_tables (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    position INTEGER NOT NULL
) STRICT;
CREATE TABLE xel_columns (
    id INTEGER PRIMARY KEY,
    table_id INTEGER NOT NULL REFERENCES xel_tables (id) ON DELETE CASCADE,
    name TEXT NOT NULL COLLATE NOCASE,
    type TEXT NOT NULL CHECK (type IN ('text', 'number', 'checkbox', 'date', 'select')),
    position INTEGER NOT NULL,
    UNIQUE (table_id, name)
) STRICT;
CREATE TABLE xel_select_options (
    column_id INTEGER NOT NULL REFERENCES xel_columns (id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (column_id, position),
    UNIQUE (column_id, value)
) STRICT;
";

/// The longest table or column name accepted, in characters.
const MAX_NAME: usize = 255;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TableId(pub(crate) i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ColumnId(pub(crate) i64);

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
}

impl ColumnType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Number => "number",
            Self::Checkbox => "checkbox",
            Self::Date => "date",
            Self::Select => "select",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "text" => Self::Text,
            "number" => Self::Number,
            "checkbox" => Self::Checkbox,
            "date" => Self::Date,
            "select" => Self::Select,
            other => return Err(Error::Corrupt(format!("unknown column type {other:?}"))),
        })
    }

    /// The column definition in the user table. STRICT tables enforce the storage
    /// class; the CHECKs keep external writers to the values xel can show.
    fn sql(self, quoted: &str) -> String {
        match self {
            Self::Text | Self::Select => format!("{quoted} TEXT"),
            Self::Number => format!("{quoted} REAL"),
            Self::Checkbox => format!("{quoted} INTEGER CHECK ({quoted} IN (0, 1))"),
            Self::Date => format!(
                "{quoted} TEXT CHECK ({quoted} GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]' \
                 AND date({quoted}) = {quoted})"
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
        let mut stmt = self
            .conn
            .prepare("SELECT id, name FROM xel_tables ORDER BY position")?;
        let heads: Vec<(i64, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        heads
            .into_iter()
            .map(|(id, name)| {
                let columns = self.columns(TableId(id))?;
                Ok(Table {
                    id: TableId(id),
                    name,
                    columns,
                })
            })
            .collect()
    }

    pub fn table(&self, id: TableId) -> Result<Table> {
        let name: String = self
            .conn
            .query_row("SELECT name FROM xel_tables WHERE id = ?1", [id.0], |r| {
                r.get(0)
            })
            .optional()?
            .ok_or(Error::NoSuchTable(id))?;
        Ok(Table {
            id,
            name,
            columns: self.columns(id)?,
        })
    }

    fn columns(&self, table: TableId) -> Result<Vec<Column>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, type FROM xel_columns WHERE table_id = ?1 ORDER BY position",
        )?;
        let rows: Vec<(i64, String, String)> = stmt
            .query_map([table.0], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut opts = self.conn.prepare(
            "SELECT value FROM xel_select_options WHERE column_id = ?1 ORDER BY position",
        )?;
        rows.into_iter()
            .map(|(id, name, ty)| {
                let options = opts
                    .query_map([id], |r| r.get(0))?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(Column {
                    id: ColumnId(id),
                    name,
                    ty: ColumnType::parse(&ty)?,
                    options,
                })
            })
            .collect()
    }

    /// Creates a table with at least one column. SQLite has no column-less tables.
    pub fn create_table(&mut self, name: &str, columns: &[(&str, ColumnType)]) -> Result<TableId> {
        check_table_name(name)?;
        if columns.is_empty() {
            return Err(Error::InvalidName {
                name: name.into(),
                reason: "a table needs at least one column".into(),
            });
        }
        for (i, (col, _)) in columns.iter().enumerate() {
            check_name(col)?;
            // SQLite identifiers are case-insensitive for ASCII only.
            if columns[..i]
                .iter()
                .any(|(c, _)| c.eq_ignore_ascii_case(col))
            {
                return Err(Error::DuplicateName((*col).into()));
            }
        }
        let tx = self.conn.transaction()?;
        ensure_table_name_free(&tx, name, None)?;
        let defs: Vec<String> = columns.iter().map(|(c, ty)| ty.sql(&quote(c))).collect();
        tx.execute_batch(&format!(
            "CREATE TABLE {} ({}) STRICT;",
            quote(name),
            defs.join(", ")
        ))?;
        tx.execute(
            "INSERT INTO xel_tables (name, position) \
             VALUES (?1, (SELECT coalesce(max(position), -1) + 1 FROM xel_tables))",
            [name],
        )?;
        let table = tx.last_insert_rowid();
        for (pos, (col, ty)) in (0_i64..).zip(columns) {
            tx.execute(
                "INSERT INTO xel_columns (table_id, name, type, position) VALUES (?1, ?2, ?3, ?4)",
                params![table, col, ty.as_str(), pos],
            )?;
        }
        tx.commit()?;
        Ok(TableId(table))
    }

    pub fn rename_table(&mut self, id: TableId, new_name: &str) -> Result<()> {
        check_table_name(new_name)?;
        let tx = self.conn.transaction()?;
        let old = table_name(&tx, id)?;
        ensure_table_name_free(&tx, new_name, Some(id))?;
        if old.eq_ignore_ascii_case(new_name) && old != new_name {
            // SQLite sees a case-only change as renaming onto an existing table, so step
            // through a name no user table can have.
            let step = quote(&format!("xel_rename_{}", id.0));
            tx.execute_batch(&format!("ALTER TABLE {} RENAME TO {step};", quote(&old)))?;
            tx.execute_batch(&format!(
                "ALTER TABLE {step} RENAME TO {};",
                quote(new_name)
            ))?;
        } else {
            tx.execute_batch(&format!(
                "ALTER TABLE {} RENAME TO {};",
                quote(&old),
                quote(new_name)
            ))?;
        }
        tx.execute(
            "UPDATE xel_tables SET name = ?1 WHERE id = ?2",
            params![new_name, id.0],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_table(&mut self, id: TableId) -> Result<()> {
        let tx = self.conn.transaction()?;
        let name = table_name(&tx, id)?;
        tx.execute_batch(&format!("DROP TABLE {};", quote(&name)))?;
        tx.execute("DELETE FROM xel_tables WHERE id = ?1", [id.0])?;
        tx.commit()?;
        Ok(())
    }

    /// Adds a column at the end of the table.
    pub fn add_column(&mut self, table: TableId, name: &str, ty: ColumnType) -> Result<ColumnId> {
        check_name(name)?;
        let tx = self.conn.transaction()?;
        let tname = table_name(&tx, table)?;
        ensure_column_name_free(&tx, table, name, None)?;
        tx.execute_batch(&format!(
            "ALTER TABLE {} ADD COLUMN {};",
            quote(&tname),
            ty.sql(&quote(name))
        ))?;
        tx.execute(
            "INSERT INTO xel_columns (table_id, name, type, position) VALUES (?1, ?2, ?3, \
             (SELECT coalesce(max(position), -1) + 1 FROM xel_columns WHERE table_id = ?1))",
            params![table.0, name, ty.as_str()],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(ColumnId(id))
    }

    pub fn rename_column(&mut self, id: ColumnId, new_name: &str) -> Result<()> {
        check_name(new_name)?;
        let tx = self.conn.transaction()?;
        let (table, old) = column_owner(&tx, id)?;
        ensure_column_name_free(&tx, table, new_name, Some(id))?;
        let tname = table_name(&tx, table)?;
        tx.execute_batch(&format!(
            "ALTER TABLE {} RENAME COLUMN {} TO {};",
            quote(&tname),
            quote(&old),
            quote(new_name)
        ))?;
        tx.execute(
            "UPDATE xel_columns SET name = ?1 WHERE id = ?2",
            params![new_name, id.0],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Deletes a column and its data. A table's last column cannot be deleted; delete the
    /// table instead.
    pub fn delete_column(&mut self, id: ColumnId) -> Result<()> {
        let tx = self.conn.transaction()?;
        let (table, name) = column_owner(&tx, id)?;
        let count: i64 = tx.query_row(
            "SELECT count(*) FROM xel_columns WHERE table_id = ?1",
            [table.0],
            |r| r.get(0),
        )?;
        if count == 1 {
            return Err(Error::LastColumn(id));
        }
        let tname = table_name(&tx, table)?;
        tx.execute_batch(&format!(
            "ALTER TABLE {} DROP COLUMN {};",
            quote(&tname),
            quote(&name)
        ))?;
        tx.execute("DELETE FROM xel_columns WHERE id = ?1", [id.0])?;
        tx.commit()?;
        Ok(())
    }

    /// Replaces a select column's options. Values already stored are not changed.
    pub fn set_select_options(&mut self, id: ColumnId, options: &[&str]) -> Result<()> {
        let tx = self.conn.transaction()?;
        let ty: String = tx
            .query_row("SELECT type FROM xel_columns WHERE id = ?1", [id.0], |r| {
                r.get(0)
            })
            .optional()?
            .ok_or(Error::NoSuchColumn(id))?;
        if ColumnType::parse(&ty)? != ColumnType::Select {
            return Err(Error::NotASelectColumn(id));
        }
        tx.execute(
            "DELETE FROM xel_select_options WHERE column_id = ?1",
            [id.0],
        )?;
        for (pos, value) in (0_i64..).zip(options) {
            tx.execute(
                "INSERT INTO xel_select_options (column_id, position, value) VALUES (?1, ?2, ?3)",
                params![id.0, pos, value],
            )
            .map_err(|e| match e.sqlite_error_code() {
                Some(rusqlite::ErrorCode::ConstraintViolation) => {
                    Error::DuplicateName((*value).into())
                }
                _ => e.into(),
            })?;
        }
        tx.commit()?;
        Ok(())
    }
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

fn check_table_name(name: &str) -> Result<()> {
    check_name(name)?;
    let lower = name.to_lowercase();
    if lower.starts_with("sqlite_") || lower.starts_with("xel_") {
        return Err(Error::InvalidName {
            name: name.into(),
            reason: "table names cannot start with sqlite_ or xel_".into(),
        });
    }
    Ok(())
}

fn table_name(tx: &Transaction<'_>, id: TableId) -> Result<String> {
    tx.query_row("SELECT name FROM xel_tables WHERE id = ?1", [id.0], |r| {
        r.get(0)
    })
    .optional()?
    .ok_or(Error::NoSuchTable(id))
}

fn column_owner(tx: &Transaction<'_>, id: ColumnId) -> Result<(TableId, String)> {
    tx.query_row(
        "SELECT table_id, name FROM xel_columns WHERE id = ?1",
        [id.0],
        |r| Ok((TableId(r.get(0)?), r.get(1)?)),
    )
    .optional()?
    .ok_or(Error::NoSuchColumn(id))
}

/// Table names share SQLite's namespace with every other schema object, so check all of
/// them, case-insensitively as SQLite does.
fn ensure_table_name_free(tx: &Transaction<'_>, name: &str, except: Option<TableId>) -> Result<()> {
    if let Some(id) = except
        && table_name(tx, id)?.eq_ignore_ascii_case(name)
    {
        return Ok(());
    }
    let taken: bool = tx.query_row(
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
    tx: &Transaction<'_>,
    table: TableId,
    name: &str,
    except: Option<ColumnId>,
) -> Result<()> {
    let taken: bool = tx.query_row(
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
            .prepare("SELECT name, type FROM pragma_table_info(?1)")
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
        raw.execute("INSERT INTO T VALUES (1.5, 1, '2024-02-29')", [])
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
            .execute("INSERT INTO \"Old\" VALUES (42)", [])
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
        raw.execute("INSERT INTO T VALUES (1, '2026-01-31')", [])
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
            Err(Error::InvalidName { .. })
        ));
        // Renaming to a different case of the same name is allowed.
        book.rename_table(t, "ORDERS").unwrap();
        assert_eq!(book.table(t).unwrap().name, "ORDERS");
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
        assert!(matches!(
            book.set_select_options(cols[0].id, &["a", "a"]),
            Err(Error::DuplicateName(_))
        ));
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
