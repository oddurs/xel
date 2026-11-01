//! Undo and redo. Every edit is described as a [`Change`] with an exact inverse and
//! applied by [`apply`], so forward edits, undo, and redo run the same code.
//!
//! Deleted tables and columns are not dropped: their data moves to `xel_trash_*` tables
//! so undo can bring it back without holding it in memory. Trash is dropped when no
//! history entry can need it any more, when the workbook closes, and on the next open
//! after a crash.

use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::error::{Error, Result};
use crate::schema::{ColumnId, ColumnType, ROW_ID, TableId, quote};
use crate::workbook::Workbook;

/// How many edits undo reaches back.
const LIMIT: usize = 1000;

/// A column as it must be recreated.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ColumnSpec {
    pub(crate) id: ColumnId,
    pub(crate) name: String,
    pub(crate) ty: ColumnType,
    pub(crate) position: i64,
    pub(crate) options: Vec<String>,
}

/// A table as it must be recreated.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TableSpec {
    pub(crate) id: TableId,
    pub(crate) name: String,
    pub(crate) position: i64,
    pub(crate) columns: Vec<ColumnSpec>,
}

/// A row's stored values, by column.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StoredRow {
    pub(crate) id: i64,
    pub(crate) values: Vec<(ColumnId, SqlValue)>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CellChange {
    pub(crate) column: ColumnId,
    pub(crate) row: i64,
    pub(crate) from: SqlValue,
    pub(crate) to: SqlValue,
}

/// One edit to a workbook.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Change {
    /// `from_trash`: the table's rows wait in its trash table, from an earlier delete.
    CreateTable {
        spec: TableSpec,
        from_trash: bool,
    },
    DeleteTable {
        spec: TableSpec,
    },
    RenameTable {
        id: TableId,
        from: String,
        to: String,
    },
    /// `from_trash`: the column's values wait in its trash table, from an earlier delete.
    AddColumn {
        table: TableId,
        spec: ColumnSpec,
        from_trash: bool,
    },
    DeleteColumn {
        table: TableId,
        spec: ColumnSpec,
    },
    RenameColumn {
        table: TableId,
        id: ColumnId,
        from: String,
        to: String,
    },
    SetOptions {
        column: ColumnId,
        from: Vec<String>,
        to: Vec<String>,
    },
    InsertRows {
        table: TableId,
        rows: Vec<StoredRow>,
    },
    DeleteRows {
        table: TableId,
        rows: Vec<StoredRow>,
    },
    SetCells {
        table: TableId,
        cells: Vec<CellChange>,
    },
}

impl Change {
    pub(crate) fn inverse(&self) -> Self {
        match self.clone() {
            Self::CreateTable { spec, .. } => Self::DeleteTable { spec },
            Self::DeleteTable { spec } => Self::CreateTable {
                spec,
                from_trash: true,
            },
            Self::RenameTable { id, from, to } => Self::RenameTable {
                id,
                from: to,
                to: from,
            },
            Self::AddColumn { table, spec, .. } => Self::DeleteColumn { table, spec },
            Self::DeleteColumn { table, spec } => Self::AddColumn {
                table,
                spec,
                from_trash: true,
            },
            Self::RenameColumn {
                table,
                id,
                from,
                to,
            } => Self::RenameColumn {
                table,
                id,
                from: to,
                to: from,
            },
            Self::SetOptions { column, from, to } => Self::SetOptions {
                column,
                from: to,
                to: from,
            },
            Self::InsertRows { table, rows } => Self::DeleteRows { table, rows },
            Self::DeleteRows { table, rows } => Self::InsertRows { table, rows },
            Self::SetCells { table, cells } => Self::SetCells {
                table,
                cells: cells
                    .into_iter()
                    .map(|c| CellChange {
                        from: c.to,
                        to: c.from,
                        ..c
                    })
                    .collect(),
            },
        }
    }

    /// The trash table this change would restore from, if any.
    fn trash(&self) -> Option<String> {
        match self {
            Self::CreateTable {
                spec,
                from_trash: true,
            } => Some(table_trash(spec.id)),
            Self::AddColumn {
                spec,
                from_trash: true,
                ..
            } => Some(column_trash(spec.id)),
            _ => None,
        }
    }
}

fn table_trash(id: TableId) -> String {
    format!("xel_trash_table_{}", id.0)
}

fn column_trash(id: ColumnId) -> String {
    format!("xel_trash_column_{}", id.0)
}

/// Undo and redo stacks. Each entry is the list of changes that perform it, in order:
/// inverses on the undo stack, forward changes on the redo stack.
#[derive(Debug, Default)]
pub(crate) struct History {
    undo: Vec<Vec<Change>>,
    redo: Vec<Vec<Change>>,
}

impl Workbook {
    /// Makes an edit: `build` validates and describes it as changes (allocating any new
    /// ids inside the transaction), which are then applied and recorded for undo.
    pub(crate) fn edit<T>(
        &mut self,
        build: impl FnOnce(&Transaction<'_>) -> Result<(Vec<Change>, T)>,
    ) -> Result<T> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (changes, out) = build(&tx)?;
        if changes.is_empty() {
            return Ok(out);
        }
        for change in &changes {
            apply(&tx, change)?;
        }
        // A new edit ends redo, and the oldest undo entry may fall off the end; drop the
        // trash either was keeping, in this same transaction.
        for change in self.history.redo.iter().flatten() {
            release(&tx, change)?;
        }
        if self.history.undo.len() >= LIMIT {
            for change in &self.history.undo[0] {
                release(&tx, change)?;
            }
        }
        tx.commit()?;
        self.history.redo.clear();
        if self.history.undo.len() >= LIMIT {
            self.history.undo.remove(0);
        }
        self.history
            .undo
            .push(changes.iter().rev().map(Change::inverse).collect());
        Ok(out)
    }

    /// Undoes the latest edit. Returns `false` when there is nothing to undo.
    pub fn undo(&mut self) -> Result<bool> {
        self.replay(true)
    }

    /// Redoes the latest undone edit. Returns `false` when there is nothing to redo.
    pub fn redo(&mut self) -> Result<bool> {
        self.replay(false)
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.history.undo.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.history.redo.is_empty()
    }

    fn replay(&mut self, undo: bool) -> Result<bool> {
        let (from, to) = if undo {
            (&mut self.history.undo, &mut self.history.redo)
        } else {
            (&mut self.history.redo, &mut self.history.undo)
        };
        let Some(entry) = from.last() else {
            return Ok(false);
        };
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for change in entry {
            apply(&tx, change)?;
        }
        tx.commit()?;
        let entry = from.pop().expect("checked above");
        to.push(entry.iter().rev().map(Change::inverse).collect());
        Ok(true)
    }
}

/// Drops every trash table: nothing can restore from them once history is gone.
pub(crate) fn purge_trash(conn: &Connection) -> Result<()> {
    let names: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' AND name GLOB 'xel_trash_*'")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for name in names {
        conn.execute_batch(&format!("DROP TABLE {};", quote(&name)))?;
    }
    Ok(())
}

fn release(conn: &Connection, change: &Change) -> Result<()> {
    if let Some(trash) = change.trash() {
        conn.execute_batch(&format!("DROP TABLE IF EXISTS {};", quote(&trash)))?;
    }
    Ok(())
}

/// The next id SQLite's AUTOINCREMENT would hand out for `table`. Allocating up front
/// lets a change carry its ids, so redo recreates things under the same ids.
pub(crate) fn next_id(conn: &Connection, table: &str) -> Result<i64> {
    let seq: Option<i64> = conn
        .query_row(
            "SELECT seq FROM sqlite_sequence WHERE name = ?1",
            [table],
            |r| r.get(0),
        )
        .optional()?;
    Ok(seq.unwrap_or(0) + 1)
}

fn table_name(conn: &Connection, id: TableId) -> Result<String> {
    conn.query_row("SELECT name FROM xel_tables WHERE id = ?1", [id.0], |r| {
        r.get(0)
    })
    .optional()?
    .ok_or(Error::NoSuchTable(id))
}

fn column_name(conn: &Connection, id: ColumnId) -> Result<String> {
    conn.query_row("SELECT name FROM xel_columns WHERE id = ?1", [id.0], |r| {
        r.get(0)
    })
    .optional()?
    .ok_or(Error::NoSuchColumn(id))
}

fn insert_column_metadata(conn: &Connection, table: TableId, spec: &ColumnSpec) -> Result<()> {
    conn.execute(
        "INSERT INTO xel_columns (id, table_id, name, type, position) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            spec.id.0,
            table.0,
            spec.name,
            spec.ty.as_str(),
            spec.position
        ],
    )?;
    set_options(conn, spec.id, &spec.options)
}

fn set_options(conn: &Connection, column: ColumnId, options: &[String]) -> Result<()> {
    conn.execute(
        "DELETE FROM xel_select_options WHERE column_id = ?1",
        [column.0],
    )?;
    for (pos, value) in (0_i64..).zip(options) {
        conn.execute(
            "INSERT INTO xel_select_options (column_id, position, value) VALUES (?1, ?2, ?3)",
            params![column.0, pos, value],
        )?;
    }
    Ok(())
}

/// Renames a SQL table, stepping through a temporary name for a case-only change, which
/// SQLite would otherwise see as renaming onto an existing table.
pub(crate) fn rename_sql_table(conn: &Connection, id: TableId, from: &str, to: &str) -> Result<()> {
    if from.eq_ignore_ascii_case(to) && from != to {
        let step = quote(&format!("xel_rename_{}", id.0));
        conn.execute_batch(&format!("ALTER TABLE {} RENAME TO {step};", quote(from)))?;
        conn.execute_batch(&format!("ALTER TABLE {step} RENAME TO {};", quote(to)))?;
    } else {
        conn.execute_batch(&format!(
            "ALTER TABLE {} RENAME TO {};",
            quote(from),
            quote(to)
        ))?;
    }
    Ok(())
}

/// Applies one change. Callers validate before building changes; this only executes.
#[allow(
    clippy::too_many_lines,
    reason = "one arm per kind of change reads best together"
)]
pub(crate) fn apply(conn: &Connection, change: &Change) -> Result<()> {
    match change {
        Change::CreateTable { spec, from_trash } => {
            if *from_trash {
                rename_sql_table(conn, spec.id, &table_trash(spec.id), &spec.name)?;
            } else {
                let defs: Vec<String> = spec
                    .columns
                    .iter()
                    .map(|c| c.ty.sql(&quote(&c.name)))
                    .collect();
                conn.execute_batch(&format!(
                    // AUTOINCREMENT: a deleted row's id is never handed out again, so an
                    // id held by undo, a selection, or a script cannot point at another row.
                    "CREATE TABLE {} ({} INTEGER PRIMARY KEY AUTOINCREMENT, {}) STRICT;",
                    quote(&spec.name),
                    quote(ROW_ID),
                    defs.join(", ")
                ))?;
            }
            conn.execute(
                "INSERT INTO xel_tables (id, name, position) VALUES (?1, ?2, ?3)",
                params![spec.id.0, spec.name, spec.position],
            )?;
            for column in &spec.columns {
                insert_column_metadata(conn, spec.id, column)?;
            }
        }
        Change::DeleteTable { spec } => {
            rename_sql_table(conn, spec.id, &spec.name, &table_trash(spec.id))?;
            conn.execute("DELETE FROM xel_tables WHERE id = ?1", [spec.id.0])?;
        }
        Change::RenameTable { id, from, to } => {
            rename_sql_table(conn, *id, from, to)?;
            conn.execute(
                "UPDATE xel_tables SET name = ?1 WHERE id = ?2",
                params![to, id.0],
            )?;
        }
        Change::AddColumn {
            table,
            spec,
            from_trash,
        } => {
            let t = quote(&table_name(conn, *table)?);
            let col = quote(&spec.name);
            conn.execute_batch(&format!(
                "ALTER TABLE {t} ADD COLUMN {};",
                spec.ty.sql(&col)
            ))?;
            insert_column_metadata(conn, *table, spec)?;
            if *from_trash {
                let trash = quote(&column_trash(spec.id));
                let id = quote(ROW_ID);
                conn.execute_batch(&format!(
                    "UPDATE {t} SET {col} = (SELECT value FROM {trash} WHERE {trash}.{id} = {t}.{id}); \
                     DROP TABLE {trash};"
                ))?;
            }
        }
        Change::DeleteColumn { table, spec } => {
            let t = quote(&table_name(conn, *table)?);
            let col = quote(&spec.name);
            let trash = quote(&column_trash(spec.id));
            conn.execute_batch(&format!(
                "CREATE TABLE {trash} AS SELECT {id}, {col} AS value FROM {t}; \
                 ALTER TABLE {t} DROP COLUMN {col};",
                id = quote(ROW_ID),
            ))?;
            conn.execute("DELETE FROM xel_columns WHERE id = ?1", [spec.id.0])?;
        }
        Change::RenameColumn {
            table,
            id,
            from,
            to,
        } => {
            conn.execute_batch(&format!(
                "ALTER TABLE {} RENAME COLUMN {} TO {};",
                quote(&table_name(conn, *table)?),
                quote(from),
                quote(to)
            ))?;
            conn.execute(
                "UPDATE xel_columns SET name = ?1 WHERE id = ?2",
                params![to, id.0],
            )?;
        }
        Change::SetOptions { column, to, .. } => set_options(conn, *column, to)?,
        Change::InsertRows { table, rows } => {
            let t = quote(&table_name(conn, *table)?);
            for row in rows {
                let mut names = vec![quote(ROW_ID)];
                let mut values = vec![SqlValue::Integer(row.id)];
                for (column, value) in &row.values {
                    names.push(quote(&column_name(conn, *column)?));
                    values.push(value.clone());
                }
                let marks = vec!["?"; names.len()].join(", ");
                conn.prepare_cached(&format!(
                    "INSERT INTO {t} ({}) VALUES ({marks})",
                    names.join(", ")
                ))?
                .execute(rusqlite::params_from_iter(values))?;
            }
        }
        Change::DeleteRows { table, rows } => {
            let t = quote(&table_name(conn, *table)?);
            let mut stmt =
                conn.prepare_cached(&format!("DELETE FROM {t} WHERE {} = ?1", quote(ROW_ID)))?;
            for row in rows {
                stmt.execute([row.id])?;
            }
        }
        Change::SetCells { table, cells } => {
            let t = quote(&table_name(conn, *table)?);
            for cell in cells {
                let col = quote(&column_name(conn, cell.column)?);
                conn.prepare_cached(&format!(
                    "UPDATE {t} SET {col} = ?1 WHERE {} = ?2",
                    quote(ROW_ID)
                ))?
                .execute(params![cell.to, cell.row])?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use rusqlite::Connection;

    use super::*;
    use crate::cell::{CellValue, Date};
    use crate::rows::RowId;
    use crate::test_support::TempDir;

    /// Everything a user can see, in a canonical form: metadata, options, and every row
    /// of every user table with columns sorted by name (a restored column comes back at
    /// the end of the SQL table). Trash and SQLite's id counters are not part of it.
    fn snapshot(conn: &Connection) -> String {
        fn dump(conn: &Connection, sql: &str, out: &mut String) {
            let mut stmt = conn.prepare(sql).unwrap();
            let n = stmt.column_count();
            let rows = stmt
                .query_map([], |r| {
                    (0..n)
                        .map(|i| r.get::<_, SqlValue>(i))
                        .collect::<rusqlite::Result<Vec<_>>>()
                })
                .unwrap();
            for row in rows {
                writeln!(out, "{:?}", row.unwrap()).unwrap();
            }
        }
        let mut out = String::new();
        dump(
            conn,
            "SELECT id, name, position FROM xel_tables ORDER BY id",
            &mut out,
        );
        dump(
            conn,
            "SELECT id, table_id, name, type, position FROM xel_columns ORDER BY id",
            &mut out,
        );
        dump(
            conn,
            "SELECT column_id, position, value FROM xel_select_options ORDER BY column_id, position",
            &mut out,
        );
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM xel_tables ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        for table in tables {
            let mut cols: Vec<String> = conn
                .prepare("SELECT name FROM pragma_table_info(?1)")
                .unwrap()
                .query_map([&table], |r| r.get(0))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap();
            cols.sort();
            let list: Vec<String> = cols.iter().map(|c| quote(c)).collect();
            writeln!(out, "table {table:?} {cols:?}").unwrap();
            let sql = format!(
                "SELECT {} FROM {} ORDER BY {}",
                list.join(", "),
                quote(&table),
                quote(ROW_ID)
            );
            dump(conn, &sql, &mut out);
        }
        out
    }

    fn trash_count(conn: &Connection) -> i64 {
        conn.query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name GLOB 'xel_trash_*'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    }

    fn book(dir: &TempDir) -> (Workbook, std::path::PathBuf) {
        let path = dir.path().join("book.xel");
        (Workbook::create(&path).unwrap(), path)
    }

    /// Runs `edit`, then checks undo restores the state before it and redo the state after.
    fn round_trip(book: &mut Workbook, edit: impl FnOnce(&mut Workbook)) {
        let before = snapshot(&book.conn);
        edit(book);
        let after = snapshot(&book.conn);
        assert_ne!(before, after, "the edit changed nothing");
        assert!(book.undo().unwrap());
        assert_eq!(snapshot(&book.conn), before, "undo");
        assert!(book.redo().unwrap());
        assert_eq!(snapshot(&book.conn), after, "redo");
    }

    #[test]
    fn every_kind_of_edit_undoes_and_redoes_exactly() {
        let dir = TempDir::new();
        let (mut book, _) = book(&dir);
        let t = book
            .create_table(
                "T",
                &[("Name", ColumnType::Text), ("Kind", ColumnType::Select)],
            )
            .unwrap();
        let cols: Vec<ColumnId> = book
            .table(t)
            .unwrap()
            .columns
            .iter()
            .map(|c| c.id)
            .collect();
        book.set_select_options(cols[1], &["a", "b"]).unwrap();
        let row = book
            .insert_row(
                t,
                &[
                    (cols[0], CellValue::Text("x".into())),
                    (cols[1], CellValue::Text("a".into())),
                ],
            )
            .unwrap();

        round_trip(&mut book, |b| {
            b.set_cell(cols[0], row, &CellValue::Text("y".into()))
                .unwrap();
        });
        round_trip(&mut book, |b| {
            b.insert_row(t, &[(cols[0], CellValue::Text("z".into()))])
                .unwrap();
        });
        round_trip(&mut book, |b| b.delete_rows(t, &[row]).unwrap());
        round_trip(&mut book, |b| {
            b.add_column(t, "When", ColumnType::Date).unwrap();
        });
        round_trip(&mut book, |b| b.rename_column(cols[0], "Title").unwrap());
        round_trip(&mut book, |b| {
            b.set_select_options(cols[1], &["c"]).unwrap();
        });
        round_trip(&mut book, |b| b.delete_column(cols[1]).unwrap());
        round_trip(&mut book, |b| b.rename_table(t, "Renamed").unwrap());
        round_trip(&mut book, |b| b.rename_table(t, "t").unwrap());
        round_trip(&mut book, |b| b.delete_table(t).unwrap());
        round_trip(&mut book, |b| {
            b.create_table("U", &[("n", ColumnType::Number)]).unwrap();
        });
    }

    #[test]
    fn undo_and_redo_report_when_there_is_nothing_to_do() {
        let dir = TempDir::new();
        let (mut book, _) = book(&dir);
        assert!(!book.can_undo() && !book.can_redo());
        assert!(!book.undo().unwrap());
        book.create_table("T", &[("a", ColumnType::Text)]).unwrap();
        assert!(book.can_undo() && !book.can_redo());
        assert!(book.undo().unwrap());
        assert!(!book.can_undo() && book.can_redo());
        assert!(!book.undo().unwrap());
    }

    #[test]
    fn a_failed_edit_changes_nothing_and_records_nothing() {
        let dir = TempDir::new();
        let (mut book, _) = book(&dir);
        let t = book
            .create_table("T", &[("n", ColumnType::Number)])
            .unwrap();
        let n = book.table(t).unwrap().columns[0].id;
        let before = snapshot(&book.conn);
        let gone = RowId { table: t, id: 99 };
        assert!(
            book.set_cells(t, &[(n, gone, CellValue::Number(1.0))])
                .is_err()
        );
        assert!(book.delete_column(n).is_err(), "last column");
        assert_eq!(snapshot(&book.conn), before);
        assert!(book.undo().unwrap());
        assert!(
            book.tables().unwrap().is_empty(),
            "the only recorded edit was create_table"
        );
    }

    #[test]
    fn no_op_edits_are_not_recorded() {
        let dir = TempDir::new();
        let (mut book, _) = book(&dir);
        let t = book
            .create_table("T", &[("s", ColumnType::Select)])
            .unwrap();
        let s = book.table(t).unwrap().columns[0].id;
        book.rename_table(t, "T").unwrap();
        book.rename_column(s, "s").unwrap();
        book.set_select_options(s, &[]).unwrap();
        assert!(book.undo().unwrap());
        assert!(!book.can_undo(), "only create_table was recorded");
    }

    #[test]
    fn trash_lives_only_as_long_as_history_needs_it() {
        let dir = TempDir::new();
        let (mut book, path) = book(&dir);
        let t = book
            .create_table("T", &[("a", ColumnType::Text), ("b", ColumnType::Text)])
            .unwrap();
        let b = book.table(t).unwrap().columns[1].id;
        book.delete_column(b).unwrap();
        book.delete_table(t).unwrap();
        assert_eq!(
            trash_count(&book.conn),
            2,
            "both deletes can still be undone"
        );

        book.undo().unwrap();
        assert_eq!(
            trash_count(&book.conn),
            1,
            "the table came back from its trash"
        );
        book.create_table("Other", &[("x", ColumnType::Text)])
            .unwrap();
        assert_eq!(
            trash_count(&book.conn),
            1,
            "redo of the table delete needs no trash"
        );

        book.undo().unwrap();
        book.undo().unwrap();
        assert_eq!(
            trash_count(&book.conn),
            1,
            "Other is in trash; column b is back"
        );
        book.insert_row(t, &[]).unwrap();
        assert_eq!(
            trash_count(&book.conn),
            0,
            "a new edit ends redo and its trash"
        );

        book.delete_table(t).unwrap();
        book.close().unwrap();
        let raw = Connection::open(&path).unwrap();
        assert_eq!(trash_count(&raw), 0, "close drops trash");
        // A crash leaves trash behind; the next open drops it.
        raw.execute_batch("CREATE TABLE xel_trash_table_99 (x)")
            .unwrap();
        drop(raw);
        let book = Workbook::open(&path).unwrap();
        assert_eq!(trash_count(&book.conn), 0);
    }

    #[test]
    fn restored_rows_keep_their_ids_and_new_ids_stay_unique() {
        let dir = TempDir::new();
        let (mut book, _) = book(&dir);
        let t = book.create_table("T", &[("a", ColumnType::Text)]).unwrap();
        let first = book.insert_row(t, &[]).unwrap();
        book.delete_rows(t, &[first]).unwrap();
        book.undo().unwrap();
        assert_eq!(book.rows(t, 0, 10).unwrap()[0].id, first);
        let second = book.insert_row(t, &[]).unwrap();
        assert!(second.id > first.id);
        book.rename_table(t, "Renamed").unwrap();
        let third = book.insert_row(t, &[]).unwrap();
        assert!(third.id > second.id, "ids stay unique across a rename");
    }

    /// A small deterministic generator, so failures replay.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, n: usize) -> usize {
            usize::try_from(self.next() % n.max(1) as u64).unwrap()
        }

        fn pick<T: Copy>(&mut self, items: &[T]) -> Option<T> {
            (!items.is_empty()).then(|| items[self.below(items.len())])
        }
    }

    /// One random edit. Returns whether it changed anything; edits that fail validation
    /// (an option that does not exist, the last column) are allowed and change nothing.
    fn random_edit(book: &mut Workbook, rng: &mut Rng, n: usize) {
        const TYPES: [ColumnType; 5] = [
            ColumnType::Text,
            ColumnType::Number,
            ColumnType::Checkbox,
            ColumnType::Date,
            ColumnType::Select,
        ];
        let tables = book.tables().unwrap();
        let table = rng.pick(&tables.iter().map(|t| t.id).collect::<Vec<_>>());
        let value_for = |ty: ColumnType, options: &[String], rng: &mut Rng| match ty {
            ColumnType::Text => CellValue::Text(format!("t{}", rng.below(100))),
            ColumnType::Number => {
                CellValue::Number(f64::from(u32::try_from(rng.below(1000)).unwrap()) / 4.0)
            }
            ColumnType::Checkbox => CellValue::Checkbox(rng.below(2) == 1),
            ColumnType::Date => CellValue::Date(
                Date::new(2000 + u16::try_from(rng.below(50)).unwrap(), 1, 1).unwrap(),
            ),
            ColumnType::Select => options
                .first()
                .map_or(CellValue::Empty, |o| CellValue::Text(o.clone())),
        };
        let _ = match (rng.below(11), table) {
            (0, _) | (_, None) => book
                .create_table(
                    &format!("T{n}"),
                    &[("a", TYPES[rng.below(5)]), ("b", TYPES[rng.below(5)])],
                )
                .map(|_| ()),
            (1, Some(t)) => book.rename_table(t, &format!("R{n}")),
            (2, Some(t)) if rng.below(3) == 0 => book.delete_table(t),
            (3, Some(t)) => book
                .add_column(t, &format!("c{n}"), TYPES[rng.below(5)])
                .map(|_| ()),
            (4, Some(t)) => {
                let cols = book.table(t).unwrap().columns;
                book.rename_column(cols[rng.below(cols.len())].id, &format!("r{n}"))
            }
            (5, Some(t)) => {
                let cols = book.table(t).unwrap().columns;
                book.delete_column(cols[rng.below(cols.len())].id)
            }
            (6, Some(t)) => {
                let cols = book.table(t).unwrap().columns;
                match cols.iter().find(|c| c.ty == ColumnType::Select) {
                    Some(c) => book.set_select_options(c.id, &[&format!("o{n}"), "keep"]),
                    None => Ok(()),
                }
            }
            (7 | 8, Some(t)) => {
                let cols = book.table(t).unwrap().columns;
                let mut values = Vec::new();
                for c in &cols {
                    if rng.below(2) == 0 {
                        values.push((c.id, value_for(c.ty, &c.options, rng)));
                    }
                }
                book.insert_row(t, &values).map(|_| ())
            }
            (9, Some(t)) => {
                let rows = book.rows(t, 0, 1000).unwrap();
                let cols = book.table(t).unwrap().columns;
                match rng.pick(&rows.iter().map(|r| r.id).collect::<Vec<_>>()) {
                    Some(row) => {
                        let c = &cols[rng.below(cols.len())];
                        book.set_cell(c.id, row, &value_for(c.ty, &c.options, rng))
                    }
                    None => Ok(()),
                }
            }
            (_, Some(t)) => {
                let rows: Vec<RowId> = book
                    .rows(t, 0, 1000)
                    .unwrap()
                    .iter()
                    .map(|r| r.id)
                    .collect();
                let doomed: Vec<RowId> = rows.into_iter().filter(|_| rng.below(3) == 0).collect();
                book.delete_rows(t, &doomed)
            }
        };
    }

    #[test]
    fn random_edit_sequences_undo_and_redo_exactly() {
        for seed in 1..=20_u64 {
            let dir = TempDir::new();
            let (mut book, _) = book(&dir);
            let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
            let mut states = vec![snapshot(&book.conn)];
            for n in 0..60 {
                random_edit(&mut book, &mut rng, n);
                let now = snapshot(&book.conn);
                if Some(&now) != states.last() {
                    states.push(now);
                }
            }
            for (i, want) in states.iter().enumerate().rev().skip(1) {
                assert!(
                    book.undo().unwrap(),
                    "seed {seed}: ran out of undo at state {i}"
                );
                assert_eq!(
                    &snapshot(&book.conn),
                    want,
                    "seed {seed}: undo to state {i}"
                );
            }
            assert!(!book.can_undo(), "seed {seed}: more undo than edits");
            for (i, want) in states.iter().enumerate().skip(1) {
                assert!(
                    book.redo().unwrap(),
                    "seed {seed}: ran out of redo at state {i}"
                );
                assert_eq!(
                    &snapshot(&book.conn),
                    want,
                    "seed {seed}: redo to state {i}"
                );
            }
        }
    }
}
