//! Computing formula columns and storing their results in the table (see 0010), so any
//! SQLite tool reads what xel shows.
//!
//! History records only inputs and formula definitions; after every edit, undo, or redo,
//! [`after`] recomputes what the applied changes can have affected. A formula that reads
//! only its own row is recomputed for the rows that changed; one that reads a whole column
//! (`SUM([Qty])`), or depends on such a formula, is recomputed for every row.

use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap};

use rusqlite::Connection;
use rusqlite::types::{Value as SqlValue, ValueRef};

use crate::error::{Error, Result};
use crate::formula::{
    Context, ErrorKind, Expr, Reference, Value, evaluate, iso_from_serial, parse, serial_from_iso,
};
use crate::history::Change;
use crate::schema::{ColumnType, ROW_ID, Table, TableId, quote};

/// Which rows of a table to recompute.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Scope {
    All,
    /// These rows; formulas that read whole columns recompute every row regardless.
    Rows(BTreeSet<i64>),
}

/// Recomputes whatever `changes` can have affected.
pub(crate) fn after(conn: &Connection, changes: &[Change]) -> Result<()> {
    let mut scopes: HashMap<TableId, Scope> = HashMap::new();
    let mut widen = |table: TableId, scope: Scope| {
        let entry = scopes.entry(table).or_insert(Scope::Rows(BTreeSet::new()));
        match (entry, scope) {
            (entry @ Scope::Rows(_), Scope::All) => *entry = Scope::All,
            (Scope::Rows(rows), Scope::Rows(more)) => rows.extend(more),
            (Scope::All, _) => {}
        }
    };
    for change in changes {
        match change {
            // Restored or recreated tables come back with their stored results; recompute
            // anyway so a restore can never leave stale values.
            Change::CreateTable { spec, .. } => widen(spec.id, Scope::All),
            Change::RenameTable { id, .. } => widen(*id, Scope::All),
            Change::AddColumn { table, .. }
            | Change::DeleteColumn { table, .. }
            | Change::RenameColumn { table, .. }
            | Change::SetFormula { table, .. } => widen(*table, Scope::All),
            Change::InsertRows { table, rows } => {
                widen(*table, Scope::Rows(rows.iter().map(|r| r.id).collect()));
            }
            // No row left to recompute, but formulas over whole columns must see the loss.
            Change::DeleteRows { table, .. } => widen(*table, Scope::Rows(BTreeSet::new())),
            Change::SetCells { table, cells } => {
                widen(*table, Scope::Rows(cells.iter().map(|c| c.row).collect()));
            }
            Change::DeleteTable { .. } | Change::SetOptions { .. } => {}
        }
    }
    for (table, scope) in scopes {
        match run(conn, table, &scope) {
            // The table was deleted later in the same entry.
            Err(Error::NoSuchTable(_)) => {}
            other => other?,
        }
    }
    Ok(())
}

/// A formula column and what it reads.
struct Formula {
    name: String,
    /// `None` if the stored formula no longer parses; every cell shows `#NAME?`.
    expr: Option<Expr>,
    /// Lower-cased names of columns read in this row (`[@Col]`) and as a whole (`[Col]`).
    this_row: BTreeSet<String>,
    whole: BTreeSet<String>,
}

impl Formula {
    fn reads(&self) -> impl Iterator<Item = &String> {
        self.this_row.iter().chain(&self.whole)
    }
}

/// Recomputes the formula columns of `table` for `scope`.
pub(crate) fn run(conn: &Connection, table: TableId, scope: &Scope) -> Result<()> {
    let t = crate::schema::load_table(conn, table)?;
    let formulas: Vec<Formula> = t
        .columns
        .iter()
        .filter_map(|c| {
            c.formula
                .as_ref()
                .map(|text| analyze(&c.name, text, &t.name))
        })
        .collect();
    if formulas.is_empty() {
        return Ok(());
    }
    let (order, cyclic) = order(&formulas);

    // Formulas reading a whole column, and everything downstream of them, cover all rows.
    let mut everywhere: BTreeSet<usize> = (0..formulas.len())
        .filter(|&i| !formulas[i].whole.is_empty())
        .collect();
    for &i in &order {
        if formulas[i]
            .reads()
            .any(|n| index_of(&formulas, n).is_some_and(|j| everywhere.contains(&j)))
        {
            everywhere.insert(i);
        }
    }

    for &i in &order {
        let rows = match scope {
            Scope::All => Scope::All,
            Scope::Rows(_) if everywhere.contains(&i) => Scope::All,
            Scope::Rows(rows) if rows.is_empty() => continue,
            Scope::Rows(rows) => Scope::Rows(rows.clone()),
        };
        compute(conn, &t, &formulas[i], &rows)?;
    }
    // A cycle has no order to compute in. Each column in or behind one shows an error
    // instead of a stale value; 0027 refines how cycles are reported.
    for i in cyclic {
        fill(conn, &t, &formulas[i].name, &Value::Error(ErrorKind::Ref))?;
    }
    Ok(())
}

fn index_of(formulas: &[Formula], lower_name: &str) -> Option<usize> {
    formulas
        .iter()
        .position(|f| f.name.to_ascii_lowercase() == lower_name)
}

/// Orders formulas so each comes after the formulas it reads; returns the order and the
/// formulas left over because they are in, or depend on, a cycle.
fn order(formulas: &[Formula]) -> (Vec<usize>, Vec<usize>) {
    let deps: Vec<BTreeSet<usize>> = formulas
        .iter()
        .map(|f| f.reads().filter_map(|n| index_of(formulas, n)).collect())
        .collect();
    let mut done = vec![false; formulas.len()];
    let mut order = Vec::new();
    loop {
        let ready: Vec<usize> = (0..formulas.len())
            .filter(|&i| !done[i] && deps[i].iter().all(|&d| done[d]))
            .collect();
        if ready.is_empty() {
            break;
        }
        for i in ready {
            done[i] = true;
            order.push(i);
        }
    }
    let cyclic = (0..formulas.len()).filter(|&i| !done[i]).collect();
    (order, cyclic)
}

/// Parses a formula and lists the columns of its own table that it reads.
fn analyze(name: &str, text: &str, table: &str) -> Formula {
    let mut f = Formula {
        name: name.into(),
        expr: parse(text).ok(),
        this_row: BTreeSet::new(),
        whole: BTreeSet::new(),
    };
    // An explicit stack: operator chains can nest thousands deep (see eval.rs).
    let mut stack: Vec<&Expr> = f.expr.iter().collect();
    while let Some(e) = stack.pop() {
        match e {
            Expr::Ref(Reference::Structured {
                table: t,
                column,
                this_row,
            }) => {
                if t.as_deref().is_none_or(|t| t.eq_ignore_ascii_case(table)) {
                    let set = if *this_row {
                        &mut f.this_row
                    } else {
                        &mut f.whole
                    };
                    set.insert(column.to_ascii_lowercase());
                }
            }
            Expr::Unary { expr, .. } => stack.push(expr),
            Expr::Binary { left, right, .. } => {
                stack.push(left);
                stack.push(right);
            }
            Expr::Call { args, .. } => stack.extend(args),
            _ => {}
        }
    }
    f
}

/// One row's values by lower-cased column name, and whole columns a formula reads.
struct RowContext<'a> {
    table: &'a str,
    row: &'a HashMap<String, Value>,
    columns: &'a HashMap<String, Vec<Value>>,
}

impl RowContext<'_> {
    /// Only the formula's own table is reachable until linked tables arrive in v0.2.
    fn own(&self, table: Option<&str>) -> std::result::Result<(), ErrorKind> {
        match table {
            None => Ok(()),
            Some(t) if t.eq_ignore_ascii_case(self.table) => Ok(()),
            Some(_) => Err(ErrorKind::Ref),
        }
    }
}

impl Context for RowContext<'_> {
    fn this_row(&self, table: Option<&str>, column: &str) -> std::result::Result<Value, ErrorKind> {
        self.own(table)?;
        self.row
            .get(&column.to_ascii_lowercase())
            .cloned()
            .ok_or(ErrorKind::Ref)
    }

    fn column(
        &self,
        table: Option<&str>,
        column: &str,
    ) -> std::result::Result<Cow<'_, [Value]>, ErrorKind> {
        self.own(table)?;
        self.columns
            .get(&column.to_ascii_lowercase())
            .map(|v| Cow::Borrowed(v.as_slice()))
            .ok_or(ErrorKind::Ref)
    }
}

/// Evaluates one formula column for `scope` and writes the results.
fn compute(conn: &Connection, t: &Table, f: &Formula, scope: &Scope) -> Result<()> {
    let id = quote(ROW_ID);
    let target = quote(&f.name);
    let tname = quote(&t.name);
    let Some(expr) = &f.expr else {
        return fill(conn, t, &f.name, &Value::Error(ErrorKind::Name));
    };

    let mut columns = HashMap::new();
    for name in &f.whole {
        if let Some(col) = t
            .columns
            .iter()
            .find(|c| c.name.to_ascii_lowercase() == *name)
        {
            let sql = format!("SELECT {} FROM {tname} ORDER BY {id}", quote(&col.name));
            let mut stmt = conn.prepare(&sql)?;
            let values = stmt
                .query_map([], |r| Ok(formula_value(col.ty, r.get_ref(0)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            columns.insert(name.clone(), values);
        }
    }

    let readable: Vec<_> = t
        .columns
        .iter()
        .filter(|c| f.this_row.contains(&c.name.to_ascii_lowercase()))
        .collect();
    let list: Vec<String> = std::iter::once(id.clone())
        .chain(readable.iter().map(|c| quote(&c.name)))
        .collect();
    let select = format!("SELECT {} FROM {tname}", list.join(", "));
    let mut read = conn.prepare(&match scope {
        Scope::All => format!("{select} ORDER BY {id}"),
        Scope::Rows(_) => format!("{select} WHERE {id} = ?1"),
    })?;
    let mut write = conn.prepare(&format!("UPDATE {tname} SET {target} = ?1 WHERE {id} = ?2"))?;
    let mut eval_row = |r: &rusqlite::Row<'_>| -> Result<()> {
        let row_id: i64 = r.get(0)?;
        let mut row = HashMap::with_capacity(readable.len());
        for (i, c) in readable.iter().enumerate() {
            row.insert(
                c.name.to_ascii_lowercase(),
                formula_value(c.ty, r.get_ref(i + 1)?),
            );
        }
        let ctx = RowContext {
            table: &t.name,
            row: &row,
            columns: &columns,
        };
        write.execute(rusqlite::params![stored(&evaluate(expr, &ctx)), row_id])?;
        Ok(())
    };
    match scope {
        Scope::All => {
            let mut rows = read.query([])?;
            while let Some(r) = rows.next()? {
                eval_row(r)?;
            }
        }
        Scope::Rows(ids) => {
            for row_id in ids {
                let mut rows = read.query([row_id])?;
                if let Some(r) = rows.next()? {
                    eval_row(r)?;
                }
            }
        }
    }
    Ok(())
}

/// Writes one value into every row of a formula column.
fn fill(conn: &Connection, t: &Table, column: &str, value: &Value) -> Result<()> {
    conn.execute(
        &format!("UPDATE {} SET {} = ?1", quote(&t.name), quote(column)),
        [stored(value)],
    )?;
    Ok(())
}

/// A stored cell as a formula sees it.
pub(crate) fn formula_value(ty: ColumnType, raw: ValueRef<'_>) -> Value {
    match (ty, raw) {
        (_, ValueRef::Null) => Value::Empty,
        (ColumnType::Checkbox | ColumnType::Formula, ValueRef::Integer(i)) => Value::Bool(i != 0),
        (ColumnType::Date, ValueRef::Text(t)) => std::str::from_utf8(t)
            .ok()
            .and_then(serial_from_iso)
            .map_or(Value::Error(ErrorKind::Value), Value::Date),
        (ColumnType::Formula, ValueRef::Text(t)) => {
            let s = String::from_utf8_lossy(t);
            // Dates are stored as ISO text (see `stored`); other text is text.
            serial_from_iso(&s).map_or_else(|| Value::Text(s.into_owned()), Value::Date)
        }
        (ColumnType::Formula, ValueRef::Blob(b)) => Value::Error(error_from(b)),
        (_, ValueRef::Real(n)) => Value::Number(n),
        (_, ValueRef::Integer(i)) =>
        {
            #[expect(
                clippy::cast_precision_loss,
                reason = "only an outside tool stores integers here"
            )]
            Value::Number(i as f64)
        }
        (_, ValueRef::Text(t)) => Value::Text(String::from_utf8_lossy(t).into_owned()),
        (_, ValueRef::Blob(_)) => Value::Error(ErrorKind::Value),
    }
}

/// A formula result as stored in its `ANY` column: numbers as `REAL`, booleans as 0/1,
/// text as `TEXT`, dates as ISO `TEXT`, and errors as a `BLOB` of their text, so an error
/// is never mistaken for text that happens to read `#DIV/0!`.
pub(crate) fn stored(value: &Value) -> SqlValue {
    match value {
        Value::Empty => SqlValue::Null,
        Value::Number(n) => SqlValue::Real(*n),
        Value::Bool(b) => SqlValue::Integer(i64::from(*b)),
        Value::Text(s) => SqlValue::Text(s.clone()),
        Value::Date(d) => iso_from_serial(*d).map_or(SqlValue::Real(*d), SqlValue::Text),
        Value::Error(e) => SqlValue::Blob(e.as_str().as_bytes().to_vec()),
    }
}

/// The error a stored `BLOB` names; unknown bytes read as `#VALUE!`.
pub(crate) fn error_from(bytes: &[u8]) -> ErrorKind {
    ErrorKind::ALL
        .into_iter()
        .find(|e| e.as_str().as_bytes() == bytes)
        .unwrap_or(ErrorKind::Value)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use crate::cell::{CellValue, Date};
    use crate::error::Error;
    use crate::formula::ErrorKind;
    use crate::rows::RowId;
    use crate::schema::{ColumnId, ColumnType, TableId};
    use crate::test_support::TempDir;
    use crate::workbook::Workbook;

    struct Orders {
        _dir: TempDir,
        path: std::path::PathBuf,
        book: Workbook,
        t: TableId,
        qty: ColumnId,
        price: ColumnId,
        rows: Vec<RowId>,
    }

    fn orders() -> Orders {
        let dir = TempDir::new();
        let path = dir.path().join("book.xel");
        let mut book = Workbook::create(&path).unwrap();
        let t = book
            .create_table(
                "Orders",
                &[("Qty", ColumnType::Number), ("Price", ColumnType::Number)],
            )
            .unwrap();
        let cols = book.table(t).unwrap().columns;
        let (qty, price) = (cols[0].id, cols[1].id);
        let rows = [(2.0, 1.5), (3.0, 10.0)]
            .iter()
            .map(|(q, p)| {
                book.insert_row(
                    t,
                    &[(qty, CellValue::Number(*q)), (price, CellValue::Number(*p))],
                )
                .unwrap()
            })
            .collect();
        Orders {
            _dir: dir,
            path,
            book,
            t,
            qty,
            price,
            rows,
        }
    }

    fn column(book: &Workbook, t: TableId, c: ColumnId) -> Vec<CellValue> {
        let i = book
            .table(t)
            .unwrap()
            .columns
            .iter()
            .position(|x| x.id == c)
            .unwrap();
        book.rows(t, 0, 1000)
            .unwrap()
            .into_iter()
            .map(|r| r.cells[i].clone())
            .collect()
    }

    fn n(x: f64) -> CellValue {
        CellValue::Number(x)
    }

    #[test]
    fn formula_columns_compute_every_row_and_sqlite_sees_the_results() {
        let mut o = orders();
        let total = o
            .book
            .add_formula_column(o.t, "Total", "=[@Qty]*[@Price]")
            .unwrap();
        assert_eq!(column(&o.book, o.t, total), [n(3.0), n(30.0)]);
        let raw = Connection::open(&o.path).unwrap();
        let stored: Vec<f64> = raw
            .prepare("SELECT Total FROM Orders ORDER BY _id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(stored, [3.0, 30.0]);
    }

    #[test]
    fn an_input_edit_updates_its_row_before_returning() {
        let mut o = orders();
        let total = o
            .book
            .add_formula_column(o.t, "Total", "=[@Qty]*[@Price]")
            .unwrap();
        o.book.set_cell(o.qty, o.rows[0], &n(10.0)).unwrap();
        assert_eq!(column(&o.book, o.t, total), [n(15.0), n(30.0)]);
        let new = o
            .book
            .insert_row(o.t, &[(o.qty, n(1.0)), (o.price, n(4.0))])
            .unwrap();
        assert_eq!(column(&o.book, o.t, total)[2], n(4.0));
        o.book.delete_rows(o.t, &[new]).unwrap();
        assert_eq!(column(&o.book, o.t, total).len(), 2);
    }

    #[test]
    fn changing_the_formula_recomputes_the_whole_column() {
        let mut o = orders();
        let total = o
            .book
            .add_formula_column(o.t, "Total", "=[@Qty]*[@Price]")
            .unwrap();
        o.book.set_formula(total, "=[@Qty]+[@Price]").unwrap();
        assert_eq!(column(&o.book, o.t, total), [n(3.5), n(13.0)]);
        assert_eq!(
            o.book.table(o.t).unwrap().columns[2].formula.as_deref(),
            Some("=[@Qty]+[@Price]")
        );
    }

    #[test]
    fn whole_column_formulas_follow_every_edit() {
        let mut o = orders();
        let share = o
            .book
            .add_formula_column(o.t, "Share", "=[@Qty]/SUM([Qty])")
            .unwrap();
        assert_eq!(column(&o.book, o.t, share), [n(0.4), n(0.6)]);
        o.book.set_cell(o.qty, o.rows[1], &n(8.0)).unwrap();
        assert_eq!(
            column(&o.book, o.t, share),
            [n(0.2), n(0.8)],
            "the other row changed too"
        );
        o.book.delete_rows(o.t, &[o.rows[1]]).unwrap();
        assert_eq!(column(&o.book, o.t, share), [n(1.0)]);
    }

    #[test]
    fn formulas_can_read_formulas() {
        let mut o = orders();
        o.book
            .add_formula_column(o.t, "Total", "=[@Qty]*[@Price]")
            .unwrap();
        // Added second but evaluated after Total, which it reads.
        let doubled = o
            .book
            .add_formula_column(o.t, "Doubled", "=[@Total]*2")
            .unwrap();
        let grand = o
            .book
            .add_formula_column(o.t, "Grand", "=SUM([Doubled])")
            .unwrap();
        assert_eq!(column(&o.book, o.t, doubled), [n(6.0), n(60.0)]);
        o.book.set_cell(o.price, o.rows[0], &n(2.0)).unwrap();
        assert_eq!(column(&o.book, o.t, doubled), [n(8.0), n(60.0)]);
        assert_eq!(column(&o.book, o.t, grand), [n(68.0), n(68.0)]);
    }

    #[test]
    fn a_cycle_shows_errors_instead_of_hanging() {
        let mut o = orders();
        let a = o.book.add_formula_column(o.t, "A", "=1").unwrap();
        let b = o.book.add_formula_column(o.t, "B", "=[@A]+1").unwrap();
        o.book.set_formula(a, "=[@B]+1").unwrap();
        let err = CellValue::Error(ErrorKind::Ref);
        assert_eq!(column(&o.book, o.t, a), [err.clone(), err.clone()]);
        assert_eq!(column(&o.book, o.t, b), [err.clone(), err]);
        o.book.undo().unwrap();
        assert_eq!(
            column(&o.book, o.t, b),
            [n(2.0), n(2.0)],
            "undo breaks the cycle"
        );
    }

    #[test]
    fn every_result_type_round_trips_and_is_stored_unambiguously() {
        let dir = TempDir::new();
        let path = dir.path().join("book.xel");
        let mut book = Workbook::create(&path).unwrap();
        let t = book
            .create_table(
                "T",
                &[("Due", ColumnType::Date), ("Note", ColumnType::Text)],
            )
            .unwrap();
        let cols = book.table(t).unwrap().columns;
        let due = Date::new(2024, 1, 31).unwrap();
        book.insert_row(
            t,
            &[
                (cols[0].id, CellValue::Date(due)),
                (cols[1].id, CellValue::Text("#DIV/0!".into())),
            ],
        )
        .unwrap();
        let cases = [
            ("=1.5", n(1.5), "real"),
            ("=\"hi\"", CellValue::Text("hi".into()), "text"),
            ("=TRUE", CellValue::Checkbox(true), "integer"),
            (
                "=[@Due]+1",
                CellValue::Date(Date::new(2024, 2, 1).unwrap()),
                "text",
            ),
            ("=1/0", CellValue::Error(ErrorKind::Div0), "blob"),
            ("=[@Note]", CellValue::Text("#DIV/0!".into()), "text"),
            ("=[@Missing]", CellValue::Error(ErrorKind::Ref), "blob"),
        ];
        for (i, (formula, want, storage)) in cases.iter().enumerate() {
            let name = format!("F{i}");
            let c = book.add_formula_column(t, &name, formula).unwrap();
            assert_eq!(column(&book, t, c), std::slice::from_ref(want), "{formula}");
            let raw = Connection::open(&path).unwrap();
            let class: String = raw
                .query_row(&format!("SELECT typeof(\"{name}\") FROM T"), [], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(&class, storage, "{formula}");
        }
    }

    #[test]
    fn formula_columns_refuse_edits_and_bad_formulas() {
        let mut o = orders();
        let total = o
            .book
            .add_formula_column(o.t, "Total", "=[@Qty]*[@Price]")
            .unwrap();
        assert!(matches!(
            o.book.set_cell(total, o.rows[0], &n(1.0)),
            Err(Error::FormulaColumn(_))
        ));
        let bad = o.book.add_formula_column(o.t, "Bad", "=1+");
        assert!(
            matches!(&bad, Err(Error::Formula { error, .. }) if error.offset == 3),
            "{bad:?}"
        );
        assert!(matches!(
            o.book.set_formula(o.qty, "=1"),
            Err(Error::NotAFormulaColumn(_))
        ));
        assert!(matches!(
            o.book.add_column(o.t, "X", ColumnType::Formula),
            Err(Error::NeedsFormula(_))
        ));
        assert!(matches!(
            o.book.create_table("U", &[("X", ColumnType::Formula)]),
            Err(Error::NeedsFormula(_))
        ));
    }

    #[test]
    fn undo_restores_formula_results_with_their_inputs() {
        let mut o = orders();
        let total = o
            .book
            .add_formula_column(o.t, "Total", "=[@Qty]*[@Price]")
            .unwrap();
        o.book.set_cell(o.qty, o.rows[0], &n(10.0)).unwrap();
        o.book.set_formula(total, "=[@Qty]-[@Price]").unwrap();
        o.book.undo().unwrap();
        assert_eq!(column(&o.book, o.t, total), [n(15.0), n(30.0)]);
        o.book.undo().unwrap();
        assert_eq!(column(&o.book, o.t, total), [n(3.0), n(30.0)]);
        o.book.redo().unwrap();
        o.book.redo().unwrap();
        assert_eq!(column(&o.book, o.t, total), [n(8.5), n(-7.0)]);
    }

    #[test]
    fn renaming_a_referenced_column_shows_ref_errors_until_formulas_are_rewritten() {
        let mut o = orders();
        let total = o
            .book
            .add_formula_column(o.t, "Total", "=[@Qty]*2")
            .unwrap();
        o.book.rename_column(o.qty, "Quantity").unwrap();
        let err = CellValue::Error(ErrorKind::Ref);
        assert_eq!(column(&o.book, o.t, total), [err.clone(), err]);
        o.book.undo().unwrap();
        assert_eq!(column(&o.book, o.t, total), [n(4.0), n(6.0)]);
    }
}
