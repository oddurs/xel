//! Rows: inserting, editing, deleting, and reading windows of a table in display order.
//! Display order is row id order for now; views add sorting in v0.2.

use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{Connection, params_from_iter};

use crate::cell::{CellValue, Date};
use crate::error::{Error, Result};
use crate::schema::{Column, ColumnId, ColumnType, ROW_ID, Table, TableId, quote};
use crate::workbook::Workbook;

/// A row's identity within its table. Stable across edits, deletes of other rows, and
/// VACUUM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RowId(pub(crate) i64);

/// One row's cells, in the table's column order.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub id: RowId,
    pub cells: Vec<CellValue>,
}

impl Workbook {
    pub fn row_count(&self, table: TableId) -> Result<u64> {
        let t = self.table(table)?;
        let n: i64 = self.conn.query_row(
            &format!("SELECT count(*) FROM {}", quote(&t.name)),
            [],
            |r| r.get(0),
        )?;
        Ok(n.unsigned_abs())
    }

    /// Up to `limit` rows starting at row `offset`, in display order.
    pub fn rows(&self, table: TableId, offset: u64, limit: u64) -> Result<Vec<Row>> {
        let t = self.table(table)?;
        let cols: Vec<String> = t.columns.iter().map(|c| quote(&c.name)).collect();
        let sql = format!(
            "SELECT {}, {} FROM {} ORDER BY {} LIMIT ?1 OFFSET ?2",
            quote(ROW_ID),
            cols.join(", "),
            quote(&t.name),
            quote(ROW_ID)
        );
        let clamp = |n: u64| i64::try_from(n).unwrap_or(i64::MAX);
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let mut rows = stmt.query([clamp(limit), clamp(offset)])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            let cells = t
                .columns
                .iter()
                .enumerate()
                .map(|(i, col)| read_cell(col, r.get_ref(i + 1)?))
                .collect::<Result<_>>()?;
            out.push(Row {
                id: RowId(r.get(0)?),
                cells,
            });
        }
        Ok(out)
    }

    /// Appends a row. Columns not listed are left empty.
    pub fn insert_row(
        &mut self,
        table: TableId,
        values: &[(ColumnId, CellValue)],
    ) -> Result<RowId> {
        let t = self.table(table)?;
        let mut names = Vec::with_capacity(values.len());
        let mut params = Vec::with_capacity(values.len());
        for (id, value) in values {
            let col = column_of(&t, *id)?;
            params.push(stored(col, value)?);
            names.push(quote(&col.name));
        }
        let tx = self.write()?;
        let sql = if names.is_empty() {
            format!("INSERT INTO {} DEFAULT VALUES", quote(&t.name))
        } else {
            let marks = vec!["?"; names.len()].join(", ");
            format!(
                "INSERT INTO {} ({}) VALUES ({marks})",
                quote(&t.name),
                names.join(", ")
            )
        };
        tx.execute(&sql, params_from_iter(params))?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(RowId(id))
    }

    /// Sets one cell. The value must fit the column's type.
    pub fn set_cell(&mut self, column: ColumnId, row: RowId, value: &CellValue) -> Result<()> {
        let (t, col) = self.table_of_column(column)?;
        let stored = stored(&col, value)?;
        let tx = self.write()?;
        let changed = tx.execute(
            &format!(
                "UPDATE {} SET {} = ?1 WHERE {} = ?2",
                quote(&t.name),
                quote(&col.name),
                quote(ROW_ID)
            ),
            rusqlite::params![stored, row.0],
        )?;
        if changed == 0 {
            return Err(Error::NoSuchRow(row));
        }
        tx.commit()?;
        Ok(())
    }

    /// Deletes rows. Fails without deleting anything if any of them does not exist.
    pub fn delete_rows(&mut self, table: TableId, rows: &[RowId]) -> Result<()> {
        let t = self.table(table)?;
        let tx = self.write()?;
        delete_each(&tx, &t, rows)?;
        tx.commit()?;
        Ok(())
    }

    fn table_of_column(&self, column: ColumnId) -> Result<(Table, Column)> {
        let table: i64 = self
            .conn
            .query_row(
                "SELECT table_id FROM xel_columns WHERE id = ?1",
                [column.0],
                |r| r.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NoSuchColumn(column),
                e => e.into(),
            })?;
        let t = self.table(TableId(table))?;
        let col = column_of(&t, column)?.clone();
        Ok((t, col))
    }
}

fn delete_each(conn: &Connection, t: &Table, rows: &[RowId]) -> Result<()> {
    let mut stmt = conn.prepare(&format!(
        "DELETE FROM {} WHERE {} = ?1",
        quote(&t.name),
        quote(ROW_ID)
    ))?;
    for row in rows {
        if stmt.execute([row.0])? == 0 {
            return Err(Error::NoSuchRow(*row));
        }
    }
    Ok(())
}

fn column_of(t: &Table, id: ColumnId) -> Result<&Column> {
    t.columns
        .iter()
        .find(|c| c.id == id)
        .ok_or(Error::NoSuchColumn(id))
}

/// Checks `value` against the column and converts it to its stored form.
fn stored(col: &Column, value: &CellValue) -> Result<SqlValue> {
    let mismatch = || Error::TypeMismatch {
        column: col.name.clone(),
        expected: col.ty,
        found: value.kind(),
    };
    Ok(match (col.ty, value) {
        (_, CellValue::Empty) => SqlValue::Null,
        (ColumnType::Text, CellValue::Text(s)) => SqlValue::Text(s.clone()),
        (ColumnType::Number, CellValue::Number(n)) if n.is_finite() => {
            SqlValue::Real(if *n == 0.0 { 0.0 } else { *n })
        }
        (ColumnType::Checkbox, CellValue::Checkbox(b)) => SqlValue::Integer(i64::from(*b)),
        (ColumnType::Date, CellValue::Date(d)) => SqlValue::Text(d.to_string()),
        (ColumnType::Select, CellValue::Text(s)) => {
            // Store the option's own spelling, so "high" in a column with "High" reads
            // back as "High".
            let option = col
                .options
                .iter()
                .find(|o| o.eq_ignore_ascii_case(s))
                .ok_or_else(|| Error::NotAnOption {
                    column: col.name.clone(),
                    value: s.clone(),
                })?;
            SqlValue::Text(option.clone())
        }
        _ => return Err(mismatch()),
    })
}

/// Converts a stored value back, according to the column's type.
fn read_cell(col: &Column, raw: ValueRef<'_>) -> Result<CellValue> {
    let corrupt = || {
        Error::Corrupt(format!(
            "column {:?} holds a value that is not {}",
            col.name, col.ty
        ))
    };
    Ok(match (col.ty, raw) {
        (_, ValueRef::Null) => CellValue::Empty,
        (ColumnType::Text | ColumnType::Select, ValueRef::Text(t)) => {
            CellValue::Text(String::from_utf8_lossy(t).into_owned())
        }
        (ColumnType::Number, ValueRef::Real(n)) => CellValue::Number(n),
        (ColumnType::Checkbox, ValueRef::Integer(i)) => CellValue::Checkbox(i != 0),
        (ColumnType::Date, ValueRef::Text(t)) => CellValue::Date(
            std::str::from_utf8(t)
                .ok()
                .and_then(Date::from_iso)
                .ok_or_else(corrupt)?,
        ),
        _ => return Err(corrupt()),
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::test_support::TempDir;

    struct Fixture {
        _dir: TempDir,
        path: std::path::PathBuf,
        book: Workbook,
        table: TableId,
        cols: Vec<ColumnId>,
    }

    fn fixture() -> Fixture {
        let dir = TempDir::new();
        let path = dir.path().join("book.xel");
        let mut book = Workbook::create(&path).unwrap();
        let table = book
            .create_table(
                "T",
                &[
                    ("Name", ColumnType::Text),
                    ("Amount", ColumnType::Number),
                    ("Paid", ColumnType::Checkbox),
                    ("Due", ColumnType::Date),
                    ("Status", ColumnType::Select),
                ],
            )
            .unwrap();
        let cols: Vec<ColumnId> = book
            .table(table)
            .unwrap()
            .columns
            .iter()
            .map(|c| c.id)
            .collect();
        book.set_select_options(cols[4], &["Open", "Done"]).unwrap();
        Fixture {
            _dir: dir,
            path,
            book,
            table,
            cols,
        }
    }

    fn text(s: &str) -> CellValue {
        CellValue::Text(s.into())
    }

    #[test]
    fn rows_round_trip_every_type() {
        let mut f = fixture();
        let due = Date::new(2026, 12, 1).unwrap();
        let id = f
            .book
            .insert_row(
                f.table,
                &[
                    (f.cols[0], text("Rent")),
                    (f.cols[1], CellValue::Number(1200.5)),
                    (f.cols[2], CellValue::Checkbox(true)),
                    (f.cols[3], CellValue::Date(due)),
                    (f.cols[4], text("open")),
                ],
            )
            .unwrap();
        f.book.insert_row(f.table, &[]).unwrap();
        let rows = f.book.rows(f.table, 0, 10).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, id);
        assert_eq!(
            rows[0].cells,
            [
                text("Rent"),
                CellValue::Number(1200.5),
                CellValue::Checkbox(true),
                CellValue::Date(due),
                text("Open"),
            ],
            "select values take the option's spelling"
        );
        assert!(rows[1].cells.iter().all(|c| *c == CellValue::Empty));

        let raw = Connection::open(&f.path).unwrap();
        let stored: (String, f64, i64, String, String) = raw
            .query_row(
                "SELECT Name, Amount, Paid, Due, Status FROM T WHERE _id = ?1",
                [id.0],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(
            stored,
            ("Rent".into(), 1200.5, 1, "2026-12-01".into(), "Open".into())
        );
    }

    #[test]
    fn wrong_types_are_refused_with_the_column_and_type() {
        let mut f = fixture();
        let row = f.book.insert_row(f.table, &[]).unwrap();
        let err = f.book.set_cell(f.cols[1], row, &text("abc")).unwrap_err();
        assert!(
            matches!(&err, Error::TypeMismatch { column, expected: ColumnType::Number, found: "text" } if column == "Amount"),
            "{err:?}"
        );
        let msg = err.to_string();
        assert!(msg.contains("Amount") && msg.contains("number"), "{msg}");
        assert!(matches!(
            f.book
                .set_cell(f.cols[1], row, &CellValue::Number(f64::NAN)),
            Err(Error::TypeMismatch { .. })
        ));
        assert!(matches!(
            f.book.set_cell(f.cols[4], row, &text("Maybe")),
            Err(Error::NotAnOption { .. })
        ));
        assert!(matches!(
            f.book.insert_row(f.table, &[(f.cols[2], text("yes"))]),
            Err(Error::TypeMismatch { .. })
        ));
        assert_eq!(
            f.book.row_count(f.table).unwrap(),
            1,
            "the failed insert added nothing"
        );
    }

    #[test]
    fn cells_can_be_set_and_cleared() {
        let mut f = fixture();
        let row = f
            .book
            .insert_row(f.table, &[(f.cols[1], CellValue::Number(1.0))])
            .unwrap();
        f.book
            .set_cell(f.cols[1], row, &CellValue::Number(-0.0))
            .unwrap();
        f.book.set_cell(f.cols[0], row, &text("x")).unwrap();
        let cells = &f.book.rows(f.table, 0, 1).unwrap()[0].cells;
        assert!(matches!(cells[1], CellValue::Number(z) if z == 0.0 && z.is_sign_positive()));
        f.book.set_cell(f.cols[0], row, &CellValue::Empty).unwrap();
        assert_eq!(
            f.book.rows(f.table, 0, 1).unwrap()[0].cells[0],
            CellValue::Empty
        );
        assert!(matches!(
            f.book.set_cell(f.cols[0], RowId(999), &text("x")),
            Err(Error::NoSuchRow(_))
        ));
    }

    #[test]
    fn deleting_rows_keeps_the_order_of_the_rest() {
        let mut f = fixture();
        let ids: Vec<RowId> = (0..10)
            .map(|i| {
                f.book
                    .insert_row(f.table, &[(f.cols[1], CellValue::Number(f64::from(i)))])
                    .unwrap()
            })
            .collect();
        f.book
            .delete_rows(f.table, &[ids[2], ids[5], ids[0]])
            .unwrap();
        let amounts: Vec<CellValue> = f
            .book
            .rows(f.table, 0, 100)
            .unwrap()
            .into_iter()
            .map(|r| r.cells[1].clone())
            .collect();
        let want: Vec<CellValue> = [1, 3, 4, 6, 7, 8, 9]
            .iter()
            .map(|&i| CellValue::Number(f64::from(i)))
            .collect();
        assert_eq!(amounts, want);
        // A delete that names a missing row deletes nothing.
        assert!(matches!(
            f.book.delete_rows(f.table, &[ids[1], ids[2]]),
            Err(Error::NoSuchRow(_))
        ));
        assert_eq!(f.book.row_count(f.table).unwrap(), 7);
        // New rows go after the existing ones even after deletes.
        let new = f.book.insert_row(f.table, &[]).unwrap();
        assert_eq!(f.book.rows(f.table, 7, 1).unwrap()[0].id, new);
    }

    #[test]
    fn windows_page_through_the_table() {
        let mut f = fixture();
        for i in 0..25 {
            f.book
                .insert_row(f.table, &[(f.cols[1], CellValue::Number(f64::from(i)))])
                .unwrap();
        }
        assert_eq!(f.book.rows(f.table, 20, 10).unwrap().len(), 5);
        assert_eq!(f.book.rows(f.table, 25, 10).unwrap().len(), 0);
        assert_eq!(f.book.rows(f.table, u64::MAX, u64::MAX).unwrap().len(), 0);
        assert_eq!(
            f.book.rows(f.table, 10, 1).unwrap()[0].cells[1],
            CellValue::Number(10.0)
        );
    }

    /// Reads rows 500,000–500,100 of a 1,000,000-row table. Run with
    /// `cargo test --release -p xel-engine window_read_benchmark -- --ignored --nocapture`.
    #[test]
    #[ignore = "benchmark; builds a 1,000,000-row table"]
    fn window_read_benchmark() {
        let f = fixture();
        f.book
            .conn
            .execute_batch(
                "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 1000000) \
                 INSERT INTO T (Name, Amount, Paid, Due, Status) \
                 SELECT 'row ' || i, i * 1.5, i % 2, '2026-01-01', 'Open' FROM n;",
            )
            .unwrap();
        let mut times = Vec::new();
        for _ in 0..21 {
            let start = std::time::Instant::now();
            let rows = f.book.rows(f.table, 500_000, 100).unwrap();
            times.push(start.elapsed());
            assert_eq!(rows.len(), 100);
        }
        times.sort();
        let median = times[times.len() / 2];
        eprintln!(
            "rows 500,000..500,100 of 1,000,000: median {median:?}, max {:?}",
            times.last().unwrap()
        );
        assert!(
            median < std::time::Duration::from_millis(10),
            "median {median:?}"
        );
    }
}
