//! Computing formula columns and storing their results in the table (see 0010), so any
//! SQLite tool reads what xel shows.
//!
//! History records only inputs and formula definitions; after every edit, undo, or redo,
//! [`after`] recomputes what the applied changes can have affected:
//! - A cell edit recomputes, for the edited rows, the formulas that read an edited column,
//!   and the formulas downstream of them.
//! - A formula that reads a whole column (`SUM([Qty])`), or depends on one, is recomputed
//!   for every row when anything it reads changes. Parts of a formula that do not depend
//!   on the row are evaluated once per pass, so `[@Qty]/SUM([Qty])` stays linear.
//! - Formula, column, and table changes recompute every formula of the table.
//!
//! Column totals (see 0026) are recomputed after the formula columns, when anything they
//! read changed, and stored in `xel_columns`.

use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap};

use rusqlite::Connection;
use rusqlite::types::{Value as SqlValue, ValueRef};

use crate::error::{Error, Result};
use crate::formula::{
    Arg, Context, ErrorKind, Expr, Reference, Value, eval_arg, evaluate, function, iso_from_serial,
    number_from_text, parse, serial_from_iso,
};
use crate::history::Change;
use crate::schema::{Column, ColumnId, ColumnType, ROW_ID, Table, TableId, quote};

/// Above this many rows, recomputing a formula scans the table once instead of querying
/// row by row.
const ROW_BY_ROW: usize = 1000;

/// Which rows of a table to recompute, and why.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Scope {
    All,
    Rows {
        rows: BTreeSet<i64>,
        /// The columns whose cells changed; `None` when whole rows came or went.
        columns: Option<BTreeSet<ColumnId>>,
    },
}

impl Scope {
    fn rows(rows: BTreeSet<i64>, columns: Option<BTreeSet<ColumnId>>) -> Self {
        Self::Rows { rows, columns }
    }

    fn widen(&mut self, other: Self) {
        match (self, other) {
            (Self::All, _) => {}
            (me @ Self::Rows { .. }, Self::All) => *me = Self::All,
            (
                Self::Rows { rows, columns },
                Self::Rows {
                    rows: more,
                    columns: cols,
                },
            ) => {
                rows.extend(more);
                match (columns.as_mut(), cols) {
                    (Some(mine), Some(theirs)) => mine.extend(theirs),
                    _ => *columns = None,
                }
            }
        }
    }
}

/// Recomputes whatever `changes` can have affected.
pub(crate) fn after(conn: &Connection, changes: &[Change]) -> Result<()> {
    let mut scopes: HashMap<TableId, Scope> = HashMap::new();
    // Totals whose own formula changed, beyond what the scopes reach.
    let mut retotal: HashMap<TableId, BTreeSet<ColumnId>> = HashMap::new();
    let mut widen = |table: TableId, scope: Scope| match scopes.get_mut(&table) {
        Some(existing) => existing.widen(scope),
        None => {
            scopes.insert(table, scope);
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
                widen(
                    *table,
                    Scope::rows(rows.iter().map(|r| r.id).collect(), None),
                );
            }
            // No row is left to recompute, but formulas over whole columns see the loss.
            Change::DeleteRows { table, .. } => widen(*table, Scope::rows(BTreeSet::new(), None)),
            Change::SetCells { table, cells } => widen(
                *table,
                Scope::rows(
                    cells.iter().map(|c| c.row).collect(),
                    Some(cells.iter().map(|c| c.column).collect()),
                ),
            ),
            Change::SetTotal { table, column, .. } => {
                retotal.entry(*table).or_default().insert(*column);
            }
            Change::DeleteTable { .. } | Change::SetOptions { .. } => {}
        }
    }
    for table in retotal.keys() {
        // Rows of no columns: no formula column is touched, only the totals named.
        widen(*table, Scope::rows(BTreeSet::new(), Some(BTreeSet::new())));
    }
    let none = BTreeSet::new();
    for (table, scope) in scopes {
        match run(conn, table, &scope, retotal.get(&table).unwrap_or(&none)) {
            // The table was deleted later in the same entry.
            Err(Error::NoSuchTable(_)) => {}
            other => other?,
        }
    }
    Ok(())
}

/// A formula column and what it reads.
struct Formula<'t> {
    column: &'t Column,
    /// `None` if the stored formula no longer parses; every cell shows `#NAME?`.
    expr: Option<Expr>,
    /// Lower-cased names of columns read in this row (`[@Col]`) and as a whole (`[Col]`).
    this_row: BTreeSet<String>,
    whole: BTreeSet<String>,
}

impl Formula<'_> {
    fn reads(&self) -> impl Iterator<Item = &String> {
        self.this_row.iter().chain(&self.whole)
    }
}

fn lower(name: &str) -> String {
    name.to_ascii_lowercase()
}

/// Recomputes the formula columns and totals of `table` for `scope`, and the totals in
/// `retotal` whatever the scope.
pub(crate) fn run(
    conn: &Connection,
    table: TableId,
    scope: &Scope,
    retotal: &BTreeSet<ColumnId>,
) -> Result<()> {
    let t = crate::schema::load_table(conn, table)?;
    let formulas: Vec<Formula<'_>> = t
        .columns
        .iter()
        .filter_map(|c| c.formula.as_ref().map(|text| analyze(c, text, &t.name)))
        .collect();
    let index_of = |name: &str| formulas.iter().position(|f| lower(&f.column.name) == name);
    let (order, cyclic) = order(&formulas, &index_of);

    // Which formulas this scope touches: all of them, or those reading an edited column,
    // and everything downstream either way.
    let mut touched: BTreeSet<usize> = match scope {
        Scope::All | Scope::Rows { columns: None, .. } => (0..formulas.len()).collect(),
        Scope::Rows {
            columns: Some(changed),
            ..
        } => {
            let names: BTreeSet<String> = t
                .columns
                .iter()
                .filter(|c| changed.contains(&c.id))
                .map(|c| lower(&c.name))
                .collect();
            (0..formulas.len())
                .filter(|&i| formulas[i].reads().any(|n| names.contains(n)))
                .collect()
        }
    };
    // Formulas over whole columns, and everything downstream, cover every row.
    let mut everywhere: BTreeSet<usize> = (0..formulas.len())
        .filter(|&i| !formulas[i].whole.is_empty())
        .collect();
    for &i in &order {
        let upstream: Vec<usize> = formulas[i].reads().filter_map(|n| index_of(n)).collect();
        if upstream.iter().any(|j| touched.contains(j)) {
            touched.insert(i);
        }
        if upstream.iter().any(|j| everywhere.contains(j)) {
            everywhere.insert(i);
        }
    }

    // Whether each formula column holds dates, updated as the pass recomputes them, so a
    // formula reading another formula column sees its current kind.
    let mut dates: HashMap<ColumnId, bool> =
        t.columns.iter().map(|c| (c.id, c.formula_dates)).collect();
    for &i in order.iter().filter(|i| touched.contains(i)) {
        let rows = match scope {
            Scope::All => None,
            Scope::Rows { .. } if everywhere.contains(&i) => None,
            Scope::Rows { rows, .. } if rows.len() > ROW_BY_ROW => None,
            Scope::Rows { rows, .. } if rows.is_empty() => continue,
            Scope::Rows { rows, .. } => Some(rows),
        };
        let now_dates = compute(conn, &t, &formulas[i], rows, &dates)?;
        dates.insert(formulas[i].column.id, now_dates);
    }
    // A cycle has no order to compute in; its columns show an error instead of a stale
    // value. Nothing a cell edit does can change that, so it is written only on full passes.
    if *scope == Scope::All {
        for i in cyclic {
            fill(
                conn,
                &t,
                &formulas[i].column.name,
                &Value::Error(ErrorKind::Ref),
            )?;
        }
    }

    // Totals reading an edited column or a recomputed formula column; all of them when
    // rows came or went or the table changed shape.
    let changed: Option<BTreeSet<String>> = match scope {
        Scope::All | Scope::Rows { columns: None, .. } => None,
        Scope::Rows {
            columns: Some(columns),
            ..
        } => Some(
            t.columns
                .iter()
                .filter(|c| columns.contains(&c.id))
                .map(|c| lower(&c.name))
                .chain(touched.iter().map(|&i| lower(&formulas[i].column.name)))
                .collect(),
        ),
    };
    let stale: Vec<Formula<'_>> = t
        .columns
        .iter()
        .filter_map(|c| c.total.as_ref().map(|text| analyze(c, text, &t.name)))
        .filter(|total| {
            retotal.contains(&total.column.id)
                || changed
                    .as_ref()
                    .is_none_or(|names| total.reads().any(|n| names.contains(n)))
        })
        .collect();
    // Each column is read once however many totals use it.
    let names: BTreeSet<String> = stale.iter().flat_map(|f| f.whole.iter().cloned()).collect();
    let columns = whole_columns(conn, &t, &names, &dates)?;
    for total in &stale {
        compute_total(conn, &t, total, &columns, &dates)?;
    }
    Ok(())
}

/// Evaluates a column's total over whole columns and stores the result beside it.
fn compute_total(
    conn: &Connection,
    t: &Table,
    total: &Formula<'_>,
    columns: &HashMap<String, Vec<Value>>,
    dates: &HashMap<ColumnId, bool>,
) -> Result<()> {
    let value = match &total.expr {
        Some(expr) => evaluate(
            expr,
            &RowContext {
                table: &t.name,
                row: &HashMap::new(),
                columns,
            },
        ),
        None => Value::Error(ErrorKind::Name),
    };
    // Excel's total row takes the column's format, so the earliest day of a date column
    // shows as a day. Values here are typed, so a date column's Min, Max, and Average
    // totals become dates; a hand-written total keeps what its formula returns.
    let column = total.column;
    let date_column = column.ty == ColumnType::Date
        || (column.ty == ColumnType::Formula
            && dates
                .get(&column.id)
                .copied()
                .unwrap_or(column.formula_dates));
    let value = match value {
        Value::Number(n) if date_column && is_dated_aggregate(column) => Value::Date(n),
        v => v,
    };
    conn.execute(
        "UPDATE xel_columns SET total_value = ?1, total_kind = ?2 WHERE id = ?3",
        rusqlite::params![
            stored(&value),
            matches!(value, Value::Date(_)).then_some("date"),
            total.column.id.0
        ],
    )?;
    Ok(())
}

/// Whether a column's total is one of the menu's aggregates that gives a day for a date
/// column.
fn is_dated_aggregate(column: &Column) -> bool {
    column.total.as_ref().is_some_and(|text| {
        crate::schema::Aggregate::ALL
            .into_iter()
            .any(|a| a.keeps_dates() && a.formula(&column.name) == *text)
    })
}

/// The values of the named columns of `t`, by lower-cased name, in row order.
fn whole_columns(
    conn: &Connection,
    t: &Table,
    names: &BTreeSet<String>,
    dates: &HashMap<ColumnId, bool>,
) -> Result<HashMap<String, Vec<Value>>> {
    let tname = quote(&t.name);
    let id = quote(ROW_ID);
    let mut columns = HashMap::new();
    for name in names {
        if let Some(col) = t.columns.iter().find(|c| lower(&c.name) == *name) {
            let is_dates = dates.get(&col.id).copied().unwrap_or(col.formula_dates);
            let sql = format!("SELECT {} FROM {tname} ORDER BY {id}", quote(&col.name));
            let mut stmt = conn.prepare(&sql)?;
            let values = stmt
                .query_map([], |r| Ok(formula_value(col.ty, is_dates, r.get_ref(0)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            columns.insert(name.clone(), values);
        }
    }
    Ok(columns)
}

/// Orders formulas so each comes after the formulas it reads; returns the order and the
/// formulas left over because they are in, or depend on, a cycle.
fn order(
    formulas: &[Formula<'_>],
    index_of: &dyn Fn(&str) -> Option<usize>,
) -> (Vec<usize>, Vec<usize>) {
    let deps: Vec<BTreeSet<usize>> = formulas
        .iter()
        .map(|f| f.reads().filter_map(|n| index_of(n)).collect())
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
fn analyze<'t>(column: &'t Column, text: &str, table: &str) -> Formula<'t> {
    let mut f = Formula {
        column,
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
                    set.insert(lower(column));
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

/// Whether `expr` can give a different result in different rows: it reads this row, a
/// name, or calls a volatile function.
pub(crate) fn varies_by_row(expr: &Expr) -> bool {
    let mut stack = vec![expr];
    while let Some(e) = stack.pop() {
        match e {
            Expr::Ref(Reference::Structured { this_row: true, .. }) | Expr::Name(_) => return true,
            Expr::Call { name, args } => {
                if function(name).is_some_and(|f| f.volatile) {
                    return true;
                }
                stack.extend(args);
            }
            Expr::Unary { expr, .. } => stack.push(expr),
            Expr::Binary { left, right, .. } => {
                stack.push(left);
                stack.push(right);
            }
            _ => {}
        }
    }
    false
}

/// Replaces function calls that give the same single value in every row with that value,
/// so `[@Qty]/SUM([Qty])` sums once per pass instead of once per row. Calls returning a
/// range, a date, or an empty value are left alone: they have no literal form.
fn hoist(expr: &mut Expr, ctx: &dyn Context) {
    let mut stack = vec![expr];
    while let Some(e) = stack.pop() {
        if matches!(e, Expr::Call { .. }) && !varies_by_row(e) {
            let literal = match eval_arg(e, ctx) {
                Arg::Value(v) | Arg::Ref(v) => match v {
                    Value::Number(n) => Some(Expr::Number(n)),
                    Value::Text(s) => Some(Expr::Text(s)),
                    Value::Bool(b) => Some(Expr::Bool(b)),
                    Value::Error(k) => Some(Expr::Error(k)),
                    Value::Date(_) | Value::Empty => None,
                },
                Arg::Range(_) => None,
            };
            if let Some(literal) = literal {
                *e = literal;
                continue;
            }
        }
        match e {
            Expr::Unary { expr, .. } => stack.push(expr),
            Expr::Binary { left, right, .. } => {
                stack.push(left);
                stack.push(right);
            }
            Expr::Call { args, .. } => stack.extend(args.iter_mut()),
            _ => {}
        }
    }
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
        self.row.get(&lower(column)).cloned().ok_or(ErrorKind::Ref)
    }

    fn column(
        &self,
        table: Option<&str>,
        column: &str,
    ) -> std::result::Result<Cow<'_, [Value]>, ErrorKind> {
        self.own(table)?;
        self.columns
            .get(&lower(column))
            .map(|v| Cow::Borrowed(v.as_slice()))
            .ok_or(ErrorKind::Ref)
    }
}

/// Evaluates one formula column for every row, or for `rows`, and writes the results.
/// Returns whether the column now holds dates.
fn compute(
    conn: &Connection,
    t: &Table,
    f: &Formula<'_>,
    rows: Option<&BTreeSet<i64>>,
    dates: &HashMap<ColumnId, bool>,
) -> Result<bool> {
    let is_dates = |c: &Column| dates.get(&c.id).copied().unwrap_or(c.formula_dates);
    let id = quote(ROW_ID);
    let target = quote(&f.column.name);
    let tname = quote(&t.name);
    let Some(parsed) = &f.expr else {
        fill(conn, t, &f.column.name, &Value::Error(ErrorKind::Name))?;
        return Ok(false);
    };

    let columns = whole_columns(conn, t, &f.whole, dates)?;
    let no_row = HashMap::new();
    let mut expr = parsed.clone();
    hoist(
        &mut expr,
        &RowContext {
            table: &t.name,
            row: &no_row,
            columns: &columns,
        },
    );

    let readable: Vec<&Column> = t
        .columns
        .iter()
        .filter(|c| f.this_row.contains(&lower(&c.name)))
        .collect();
    let list: Vec<String> = std::iter::once(id.clone())
        .chain(readable.iter().map(|c| quote(&c.name)))
        .collect();
    let select = format!("SELECT {} FROM {tname}", list.join(", "));
    let mut read = conn.prepare(&match rows {
        None => format!("{select} ORDER BY {id}"),
        Some(_) => format!("{select} WHERE {id} = ?1"),
    })?;
    let mut write = conn.prepare(&format!("UPDATE {tname} SET {target} = ?1 WHERE {id} = ?2"))?;
    let mut kinds = Kinds::default();
    let mut eval_row = |r: &rusqlite::Row<'_>| -> Result<()> {
        let row_id: i64 = r.get(0)?;
        let mut row = HashMap::with_capacity(readable.len());
        for (i, c) in readable.iter().enumerate() {
            row.insert(
                lower(&c.name),
                formula_value(c.ty, is_dates(c), r.get_ref(i + 1)?),
            );
        }
        let ctx = RowContext {
            table: &t.name,
            row: &row,
            columns: &columns,
        };
        let value = evaluate(&expr, &ctx);
        kinds.see(&value);
        write.execute(rusqlite::params![stored(&value), row_id])?;
        Ok(())
    };
    match rows {
        None => {
            let mut all = read.query([])?;
            while let Some(r) = all.next()? {
                eval_row(r)?;
            }
        }
        Some(ids) => {
            for row_id in ids {
                let mut one = read.query([row_id])?;
                if let Some(r) = one.next()? {
                    eval_row(r)?;
                }
            }
        }
    }
    kinds.record(conn, f.column, is_dates(f.column), rows.is_none())
}

/// What kinds of results a pass produced, to decide whether the column holds dates.
#[derive(Default)]
struct Kinds {
    dates: bool,
    others: bool,
}

impl Kinds {
    fn see(&mut self, v: &Value) {
        match v {
            Value::Date(_) => self.dates = true,
            Value::Empty | Value::Error(_) => {}
            _ => self.others = true,
        }
    }

    /// A column holds dates when every non-blank, non-error result is a date. A partial
    /// pass sees only some rows, so it can only take that away; a full pass decides afresh.
    fn record(&self, conn: &Connection, column: &Column, was: bool, full: bool) -> Result<bool> {
        let dates = if full {
            self.dates && !self.others
        } else {
            was && !self.others
        };
        if dates != was {
            conn.execute(
                "UPDATE xel_columns SET formula_kind = ?1 WHERE id = ?2",
                rusqlite::params![dates.then_some("date"), column.id.0],
            )?;
        }
        Ok(dates)
    }
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
/// `dates` says whether a formula column's results are dates.
pub(crate) fn formula_value(ty: ColumnType, dates: bool, raw: ValueRef<'_>) -> Value {
    match (ty, raw) {
        (_, ValueRef::Null) => Value::Empty,
        (ColumnType::Checkbox | ColumnType::Formula, ValueRef::Integer(i)) => Value::Bool(i != 0),
        (ColumnType::Date, ValueRef::Text(t)) => std::str::from_utf8(t)
            .ok()
            .and_then(serial_from_iso)
            .map_or(Value::Error(ErrorKind::Value), Value::Date),
        (ColumnType::Formula, ValueRef::Text(t)) => {
            let s = String::from_utf8_lossy(t);
            // Only a column whose results are dates reads its ISO text as dates; text that
            // merely looks like a date stays text.
            let date = dates.then(|| serial_from_date_text(&s)).flatten();
            date.map_or_else(|| Value::Text(s.into_owned()), Value::Date)
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

/// `YYYY-MM-DD` or `YYYY-MM-DD HH:MM:SS`, as `stored` writes dates.
fn serial_from_date_text(s: &str) -> Option<f64> {
    let day = serial_from_iso(s.get(..10)?)?;
    match s.get(10..) {
        Some("") => Some(day),
        Some(rest) => Some(day + number_from_text(rest.strip_prefix(' ')?)?),
        None => None,
    }
}

/// A formula result as stored in its `ANY` column: numbers as `REAL`, booleans as 0/1,
/// text as `TEXT`, dates as ISO `TEXT` (with the time when there is one), and errors as a
/// `BLOB` of their text, so an error is never mistaken for text reading `#DIV/0!`.
pub(crate) fn stored(value: &Value) -> SqlValue {
    match value {
        Value::Empty => SqlValue::Null,
        Value::Number(n) => SqlValue::Real(*n),
        Value::Bool(b) => SqlValue::Integer(i64::from(*b)),
        Value::Text(s) => SqlValue::Text(s.clone()),
        Value::Date(d) => match iso_from_serial(*d) {
            Some(day) if d.fract() == 0.0 => SqlValue::Text(day),
            Some(day) => SqlValue::Text(format!("{day} {}", time_of_day(*d))),
            None => SqlValue::Real(*d),
        },
        Value::Error(e) => SqlValue::Blob(e.as_str().as_bytes().to_vec()),
    }
}

/// The time part of a serial, as `HH:MM:SS`, rounded to the second.
fn time_of_day(serial: f64) -> String {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a fraction of one day"
    )]
    let seconds = (serial.fract() * 86_400.0).round().min(86_399.0) as u32;
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
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
    use crate::schema::{Aggregate, ColumnId, ColumnType, TableId};
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
    fn date_looking_text_stays_text_and_dates_keep_their_time() {
        let dir = TempDir::new();
        let mut book = Workbook::create(dir.path().join("book.xel")).unwrap();
        let t = book
            .create_table(
                "T",
                &[("Code", ColumnType::Text), ("Due", ColumnType::Date)],
            )
            .unwrap();
        let cols = book.table(t).unwrap().columns;
        book.insert_row(
            t,
            &[
                (cols[0].id, CellValue::Text("2024-01-31".into())),
                (cols[1].id, CellValue::Date(Date::new(2024, 1, 31).unwrap())),
            ],
        )
        .unwrap();
        let echo = book.add_formula_column(t, "Echo", "=[@Code]").unwrap();
        assert_eq!(
            column(&book, t, echo),
            [CellValue::Text("2024-01-31".into())]
        );
        let compared = book
            .add_formula_column(t, "Same", "=[@Echo]=\"2024-01-31\"")
            .unwrap();
        assert_eq!(column(&book, t, compared), [CellValue::Checkbox(true)]);
        let noon = book.add_formula_column(t, "Noon", "=[@Due]+0.5").unwrap();
        let back = book
            .add_formula_column(t, "Back", "=[@Noon]-[@Due]")
            .unwrap();
        assert_eq!(
            column(&book, t, back),
            [n(0.5)],
            "the time survived storage"
        );
        let next = book.add_formula_column(t, "Next", "=[@Due]+1").unwrap();
        assert_eq!(
            column(&book, t, next),
            [CellValue::Date(Date::new(2024, 2, 1).unwrap())]
        );
        assert!(matches!(column(&book, t, noon)[0], CellValue::Date(_)));
    }

    /// A share-of-total formula over 100,000 rows, recomputed after one cell edit. Run with
    /// `cargo test --release -p xel-engine share_of_total_benchmark -- --ignored --nocapture`.
    #[test]
    #[ignore = "benchmark"]
    fn share_of_total_benchmark() {
        let mut o = orders();
        o.book
            .conn
            .execute_batch(
                "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 100000) \
                 INSERT INTO Orders (Qty, Price) SELECT i % 97, 1 FROM n;",
            )
            .unwrap();
        o.book
            .add_formula_column(o.t, "Share", "=[@Qty]/SUM([Qty])")
            .unwrap();
        let start = std::time::Instant::now();
        o.book.set_cell(o.qty, o.rows[0], &n(5.0)).unwrap();
        let elapsed = start.elapsed();
        eprintln!("one edit with a share-of-total column over 100,002 rows: {elapsed:?}");
        if !cfg!(debug_assertions) {
            assert!(
                elapsed < std::time::Duration::from_millis(500),
                "{elapsed:?}"
            );
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

    fn totals(book: &Workbook, t: TableId) -> Vec<Option<CellValue>> {
        book.totals(t).unwrap()
    }

    fn num(n: f64) -> CellValue {
        CellValue::Number(n)
    }

    #[test]
    fn totals_follow_every_edit_including_formula_columns() {
        let mut o = orders();
        o.book
            .set_total(o.qty, Some(&Aggregate::Sum.formula("Qty")))
            .unwrap();
        assert_eq!(totals(&o.book, o.t), [Some(num(5.0)), None]);

        o.book
            .set_cell(o.qty, o.rows[0], &CellValue::Number(10.0))
            .unwrap();
        assert_eq!(totals(&o.book, o.t), [Some(num(13.0)), None]);
        let extra = o
            .book
            .insert_row(o.t, &[(o.qty, CellValue::Number(1.0))])
            .unwrap();
        assert_eq!(totals(&o.book, o.t)[0], Some(num(14.0)));
        o.book.delete_rows(o.t, &[extra]).unwrap();
        assert_eq!(totals(&o.book, o.t)[0], Some(num(13.0)));
        o.book.undo().unwrap();
        assert_eq!(
            totals(&o.book, o.t)[0],
            Some(num(14.0)),
            "undo restores the row"
        );

        // A total over a formula column follows the formula's inputs.
        let total = o
            .book
            .add_formula_column(o.t, "Total", "=[@Qty]*[@Price]")
            .unwrap();
        o.book
            .set_total(total, Some(&Aggregate::Sum.formula("Total")))
            .unwrap();
        assert_eq!(totals(&o.book, o.t)[2], Some(num(10.0 * 1.5 + 30.0)));
        o.book
            .set_cell(o.price, o.rows[1], &CellValue::Number(1.0))
            .unwrap();
        assert_eq!(totals(&o.book, o.t)[2], Some(num(15.0 + 3.0)));
        assert_eq!(
            totals(&o.book, o.t)[0],
            Some(num(14.0)),
            "the Qty total is unchanged"
        );
    }

    #[test]
    fn totals_are_stored_in_xel_columns_and_survive_reopen_and_save() {
        let mut o = orders();
        o.book
            .set_total(o.price, Some(&Aggregate::Max.formula("Price")))
            .unwrap();
        let saved = o.path.with_file_name("saved.xel");
        o.book.save_copy(&saved).unwrap();
        o.book.close().unwrap();

        let plain: (String, f64) = Connection::open(&o.path)
            .unwrap()
            .query_row(
                "SELECT total, total_value FROM xel_columns WHERE name = 'Price'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(plain, ("=MAX([Price])".into(), 10.0));
        for path in [&o.path, &saved] {
            let book = Workbook::open(path).unwrap();
            assert_eq!(totals(&book, o.t), [None, Some(num(10.0))]);
            assert_eq!(
                book.table(o.t).unwrap().columns[1].total.as_deref(),
                Some("=MAX([Price])")
            );
        }
    }

    #[test]
    fn setting_a_total_undoes_and_refuses_what_a_total_row_cannot_show() {
        let mut o = orders();
        let sum = Aggregate::Sum.formula("Qty");
        o.book.set_total(o.qty, Some(&sum)).unwrap();
        o.book.undo().unwrap();
        assert_eq!(totals(&o.book, o.t), [None, None]);
        o.book.redo().unwrap();
        assert_eq!(totals(&o.book, o.t)[0], Some(num(5.0)));

        assert!(matches!(
            o.book.set_total(o.qty, Some("=[@Qty]")),
            Err(Error::NotATotal(_))
        ));
        assert!(matches!(
            o.book.set_total(o.qty, Some("=SUM([Qty]")),
            Err(Error::Formula { .. })
        ));
        assert!(matches!(
            o.book.set_total(o.qty, Some("=SUM([Qty])*Rate")),
            Err(Error::NotATotal(_))
        ));
        assert_eq!(
            totals(&o.book, o.t)[0],
            Some(num(5.0)),
            "refused totals change nothing"
        );

        // A deleted column takes its total along, and undo brings both back.
        o.book.delete_column(o.qty).unwrap();
        o.book.undo().unwrap();
        assert_eq!(totals(&o.book, o.t)[0], Some(num(5.0)));

        o.book.set_total(o.qty, None).unwrap();
        assert_eq!(totals(&o.book, o.t), [None, None]);
        assert!(o.book.table(o.t).unwrap().columns[0].total.is_none());
    }

    #[test]
    fn aggregates_cover_every_kind_and_awkward_names() {
        let dir = TempDir::new();
        let mut book = Workbook::create(dir.path().join("book.xel")).unwrap();
        let names = ["a]b", "it's", "#n", "@x", "[y"];
        let columns: Vec<(&str, ColumnType)> =
            names.iter().map(|n| (*n, ColumnType::Number)).collect();
        let t = book.create_table("T", &columns).unwrap();
        let ids: Vec<ColumnId> = book
            .table(t)
            .unwrap()
            .columns
            .iter()
            .map(|c| c.id)
            .collect();
        for n in [1.0, 2.0, 6.0] {
            let row: Vec<(ColumnId, CellValue)> =
                ids.iter().map(|c| (*c, CellValue::Number(n))).collect();
            book.insert_row(t, &row).unwrap();
        }
        let aggregates = [
            (Aggregate::Sum, 9.0),
            (Aggregate::Average, 3.0),
            (Aggregate::Count, 3.0),
            (Aggregate::Min, 1.0),
            (Aggregate::Max, 6.0),
        ];
        for ((name, id), (aggregate, _)) in names.iter().zip(&ids).zip(aggregates) {
            book.set_total(*id, Some(&aggregate.formula(name))).unwrap();
        }
        let want: Vec<Option<CellValue>> = aggregates.iter().map(|(_, n)| Some(num(*n))).collect();
        assert_eq!(totals(&book, t), want);

        // Count counts what is filled in, as Excel's total row does, not only numbers.
        let words = book
            .create_table("Words", &[("Word", ColumnType::Text)])
            .unwrap();
        let word = book.table(words).unwrap().columns[0].id;
        for w in ["a", "b"] {
            book.insert_row(words, &[(word, CellValue::Text(w.into()))])
                .unwrap();
        }
        book.insert_row(words, &[]).unwrap();
        book.set_total(word, Some(&Aggregate::Count.formula("Word")))
            .unwrap();
        assert_eq!(totals(&book, words), [Some(num(2.0))]);

        let d = book
            .create_table("Dates", &[("When", ColumnType::Date)])
            .unwrap();
        let when = book.table(d).unwrap().columns[0].id;
        book.set_total(when, Some(&Aggregate::Average.formula("When")))
            .unwrap();
        assert_eq!(
            totals(&book, d),
            [Some(CellValue::Error(ErrorKind::Div0))],
            "an empty column has no average"
        );
        book.set_total(when, Some(&Aggregate::Min.formula("When")))
            .unwrap();
        for day in [9, 3] {
            let date = Date::new(2026, 11, day).unwrap();
            book.insert_row(d, &[(when, CellValue::Date(date))])
                .unwrap();
        }
        assert_eq!(
            totals(&book, d),
            [Some(CellValue::Date(Date::new(2026, 11, 3).unwrap()))]
        );
        // A hand-written total keeps what its formula returns; formulas follow Excel.
        book.set_total(when, Some("=MIN([When])+0")).unwrap();
        assert_eq!(totals(&book, d), [Some(num(46329.0))]);
    }
}
