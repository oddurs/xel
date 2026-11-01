//! Evaluating a parsed formula against whatever holds the data.
//!
//! The evaluator knows nothing about storage. A [`Context`] answers references: the
//! current row's value of a column, a whole column, a cell or range on a sheet, a name.
//! Table formula columns, calc blocks, and the Python package each supply their own.

use std::borrow::Cow;

use super::ast::{A1, BinaryOp, ErrorKind, Expr, Reference, UnaryOp};
use super::value::{Value, binary, unary};

/// Where a formula's references are looked up. Table and column names compare without
/// regard to ASCII case, as SQLite identifiers do.
pub trait Context {
    /// `[@Col]` or `Table[@Col]`: this row's value of a column. `table` is `None` for the
    /// formula's own table; a context decides whether a named table is its own.
    fn this_row(&self, table: Option<&str>, column: &str) -> Result<Value, ErrorKind>;

    /// `[Col]` or `Table[Col]`: every value of a column, top to bottom. Borrowed where the
    /// context can, so a formula that sums a column in every row does not copy it each time.
    fn column(&self, table: Option<&str>, column: &str) -> Result<Cow<'_, [Value]>, ErrorKind>;

    /// `A1`. Only calc blocks have cells; a table formula referring to one is `#REF!`.
    fn cell(&self, _cell: A1) -> Result<Value, ErrorKind> {
        Err(ErrorKind::Ref)
    }

    /// `A1:B2`.
    fn range(&self, _from: A1, _to: A1) -> Result<Range<'_>, ErrorKind> {
        Err(ErrorKind::Ref)
    }

    /// A defined name, passed upper-cased because Excel names ignore case. A name can
    /// stand for a value, a cell, or a range, so it answers with an [`Arg`].
    fn name(&self, _name: &str) -> Result<Arg<'_>, ErrorKind> {
        Err(ErrorKind::Name)
    }
}

/// A rectangle of values, row by row. Its length always equals rows × columns.
#[derive(Debug, Clone, PartialEq)]
pub struct Range<'a> {
    rows: usize,
    cols: usize,
    values: Cow<'a, [Value]>,
}

impl<'a> Range<'a> {
    /// A range of `rows` × `cols`, or `None` if `values` is not exactly that long.
    #[must_use]
    pub fn new(rows: usize, cols: usize, values: Cow<'a, [Value]>) -> Option<Self> {
        (rows.checked_mul(cols)? == values.len()).then_some(Self { rows, cols, values })
    }

    /// A single column, as a structured reference produces.
    #[must_use]
    pub fn column(values: Cow<'a, [Value]>) -> Self {
        Self {
            rows: values.len(),
            cols: 1,
            values,
        }
    }

    pub(crate) fn single(value: Value) -> Self {
        Self {
            rows: 1,
            cols: 1,
            values: Cow::Owned(vec![value]),
        }
    }

    #[must_use]
    pub fn rows(&self) -> usize {
        self.rows
    }

    #[must_use]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Every value, row by row.
    #[must_use]
    pub fn values(&self) -> &[Value] {
        &self.values
    }

    /// The value at a row and column, or `None` outside the range.
    #[must_use]
    pub fn get(&self, row: usize, col: usize) -> Option<&Value> {
        (row < self.rows && col < self.cols).then(|| &self.values[row * self.cols + col])
    }
}

/// A function argument or result.
#[derive(Debug, Clone, PartialEq)]
pub enum Arg<'a> {
    /// A value written or computed in the formula: `1`, `"a"`, `2*3`.
    Value(Value),
    /// One cell reached through a reference: `A1`, `[@Qty]`. Excel treats these like
    /// ranges in some functions, so `COUNT(A1)` skips text that `COUNT("1")` counts.
    Ref(Value),
    /// A range, a column, or an array computed from one (`[Qty]*2`, `FILTER(…)`).
    Range(Range<'a>),
}

/// A function's arguments, evaluated only when the function asks for them, so `IF` can
/// skip the branch it does not take and `IFERROR` can catch an error.
pub struct Args<'a> {
    exprs: &'a [Expr],
    ctx: &'a dyn Context,
}

impl<'a> Args<'a> {
    #[must_use]
    pub fn len(&self) -> usize {
        self.exprs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.exprs.is_empty()
    }

    /// Whether argument `i` was left out, as the middle one in `IF(A1,,1)`.
    #[must_use]
    pub fn is_missing(&self, i: usize) -> bool {
        matches!(self.exprs.get(i), None | Some(Expr::Missing))
    }

    /// Argument `i` as one value. A multi-cell range where a value is wanted is
    /// `#VALUE!`; a missing argument is `Empty`.
    #[must_use]
    pub fn value(&self, i: usize) -> Value {
        scalar(self.arg(i))
    }

    /// Argument `i` as written: a value, a single-cell reference, or a range.
    #[must_use]
    pub fn arg(&self, i: usize) -> Arg<'a> {
        self.exprs
            .get(i)
            .map_or(Arg::Value(Value::Empty), |e| eval_arg(e, self.ctx))
    }

    /// The context, for functions that need more than their arguments (`ROW`, `INDIRECT`).
    #[must_use]
    pub fn context(&self) -> &'a dyn Context {
        self.ctx
    }
}

/// A built-in function.
pub struct Function {
    /// Upper-case, as the parser produces names.
    pub name: &'static str,
    pub min_args: usize,
    /// `None` for functions that take any number, such as `SUM`.
    pub max_args: Option<usize>,
    /// Recalculated on every recalculation, like `TODAY` and `NOW`.
    pub volatile: bool,
    /// For autocomplete: `IF(logical_test, [value_if_true], [value_if_false])`.
    pub signature: &'static str,
    pub description: &'static str,
    /// Returns an [`Arg`], not a [`Value`], so `IF` and `IFERROR` can pass an array or a
    /// reference through and `FILTER`-like functions can return arrays.
    pub eval: for<'a> fn(&Args<'a>) -> Arg<'a>,
}

/// All built-in functions, sorted by name.
#[must_use]
pub fn functions() -> &'static [Function] {
    super::functions::FUNCTIONS
}

/// Looks up a built-in function by its upper-case name.
#[must_use]
pub fn function(name: &str) -> Option<&'static Function> {
    let all = functions();
    all.binary_search_by(|f| f.name.cmp(name))
        .ok()
        .map(|i| &all[i])
}

/// Evaluates a formula to the single value a cell shows. An empty result shows as 0,
/// as in Excel (`=A1` with A1 blank).
#[must_use]
pub fn evaluate(expr: &Expr, ctx: &dyn Context) -> Value {
    match scalar(eval_arg(expr, ctx)) {
        Value::Empty => Value::Number(0.0),
        v => v,
    }
}

/// One value from an argument. A 1×1 range is its value; a larger one where one value
/// is wanted is `#VALUE!` (Excel would spill or intersect; neither applies to a table cell).
fn scalar(arg: Arg<'_>) -> Value {
    match arg {
        Arg::Value(v) | Arg::Ref(v) => v,
        Arg::Range(r) if r.rows == 1 && r.cols == 1 => r.values[0].clone(),
        Arg::Range(_) => Value::Error(ErrorKind::Value),
    }
}

/// A step on the left spine of an operator chain.
enum Step<'e> {
    Binary(BinaryOp, &'e Expr),
    Unary(UnaryOp),
}

/// Evaluates to an argument, keeping a range a range, for callers that must tell a
/// single value from an array.
pub(crate) fn eval_arg<'a>(expr: &'a Expr, ctx: &'a dyn Context) -> Arg<'a> {
    // `1+1+…+1` and `1%%…%` nest to the left one level per operator, deeper than the
    // parser's nesting limit counts. Walk that spine with a loop, not recursion.
    let mut spine = Vec::new();
    let mut node = expr;
    loop {
        match node {
            Expr::Binary { op, left, right } => {
                spine.push(Step::Binary(*op, right));
                node = left;
            }
            Expr::Unary { op, expr } => {
                spine.push(Step::Unary(*op));
                node = expr;
            }
            _ => break,
        }
    }
    let mut acc = eval_leaf(node, ctx);
    for step in spine.into_iter().rev() {
        acc = match step {
            Step::Binary(op, right) => apply_binary(op, acc, eval_arg(right, ctx)),
            Step::Unary(op) => map(acc, |v| unary(op, v)),
        };
    }
    acc
}

fn eval_leaf<'a>(expr: &'a Expr, ctx: &'a dyn Context) -> Arg<'a> {
    let reference = |r: Result<Value, ErrorKind>| Arg::Ref(r.unwrap_or_else(Value::Error));
    let or_error =
        |r: Result<Arg<'a>, ErrorKind>| r.unwrap_or_else(|e| Arg::Value(Value::Error(e)));
    match expr {
        Expr::Number(n) => Arg::Value(Value::number(*n)),
        Expr::Text(s) => Arg::Value(Value::Text(s.clone())),
        Expr::Bool(b) => Arg::Value(Value::Bool(*b)),
        Expr::Error(e) => Arg::Value(Value::Error(*e)),
        Expr::Missing => Arg::Value(Value::Empty),
        Expr::Name(n) => or_error(ctx.name(&n.to_uppercase())),
        Expr::Ref(Reference::Structured {
            table,
            column,
            this_row: true,
        }) => reference(ctx.this_row(table.as_deref(), column)),
        Expr::Ref(Reference::Structured {
            table,
            column,
            this_row: false,
        }) => or_error(
            ctx.column(table.as_deref(), column)
                .map(|v| Arg::Range(Range::column(v))),
        ),
        Expr::Ref(Reference::Cell(a)) => reference(ctx.cell(*a)),
        Expr::Ref(Reference::Range(a, b)) => or_error(ctx.range(*a, *b).map(Arg::Range)),
        Expr::Call { name, args } => call(name, args, ctx),
        // The spine walk in eval_arg consumes these before reaching a leaf.
        Expr::Unary { .. } | Expr::Binary { .. } => eval_arg(expr, ctx),
    }
}

/// Applies `f` to a value, or to every value of a range.
fn map(arg: Arg<'_>, f: impl Fn(&Value) -> Value) -> Arg<'_> {
    match arg {
        Arg::Value(v) | Arg::Ref(v) => Arg::Value(f(&v)),
        Arg::Range(r) => Arg::Range(Range {
            rows: r.rows,
            cols: r.cols,
            values: Cow::Owned(r.values.iter().map(f).collect()),
        }),
    }
}

/// A binary operator, element by element when either side is a range.
fn apply_binary<'a>(op: BinaryOp, a: Arg<'a>, b: Arg<'a>) -> Arg<'a> {
    if !matches!(a, Arg::Range(_)) && !matches!(b, Arg::Range(_)) {
        return Arg::Value(binary(op, &scalar(a), &scalar(b)));
    }
    Arg::Range(broadcast(&[as_range(a), as_range(b)], |v| {
        binary(op, &v[0], &v[1])
    }))
}

/// Any argument as a range; a single value is a 1×1 range.
pub(crate) fn as_range(arg: Arg<'_>) -> Range<'_> {
    match arg {
        Arg::Range(r) => r,
        Arg::Value(v) | Arg::Ref(v) => Range::single(v),
    }
}

/// Combines ranges element by element. Shapes broadcast as in Excel: a single value or a
/// single row or column stretches to fit; cells beyond a smaller range are `#N/A`; an
/// empty range gives an empty result.
pub(crate) fn broadcast(ranges: &[Range<'_>], f: impl Fn(&[Value]) -> Value) -> Range<'static> {
    let span = |dim: fn(&Range<'_>) -> usize| {
        if ranges.iter().any(|r| dim(r) == 0) {
            0
        } else {
            ranges.iter().map(dim).max().unwrap_or(0)
        }
    };
    let rows = span(|r| r.rows);
    let cols = span(|r| r.cols);
    let pick = |r: &Range<'_>, row: usize, col: usize| {
        let row = if r.rows == 1 { 0 } else { row };
        let col = if r.cols == 1 { 0 } else { col };
        r.get(row, col)
            .cloned()
            .unwrap_or(Value::Error(ErrorKind::NA))
    };
    let mut values = Vec::with_capacity(rows * cols);
    let mut cell = Vec::with_capacity(ranges.len());
    for row in 0..rows {
        for col in 0..cols {
            cell.clear();
            cell.extend(ranges.iter().map(|r| pick(r, row, col)));
            values.push(f(&cell));
        }
    }
    Range {
        rows,
        cols,
        values: Cow::Owned(values),
    }
}

fn call<'a>(name: &str, args: &'a [Expr], ctx: &'a dyn Context) -> Arg<'a> {
    let Some(f) = function(name) else {
        return Arg::Value(Value::Error(ErrorKind::Name));
    };
    if args.len() < f.min_args || f.max_args.is_some_and(|max| args.len() > max) {
        return Arg::Value(Value::Error(ErrorKind::Value));
    }
    (f.eval)(&Args { exprs: args, ctx })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::formula::parse;

    /// A table in memory, named Orders: column name to values, and the current row.
    struct Orders {
        columns: HashMap<&'static str, Vec<Value>>,
        row: usize,
    }

    impl Orders {
        fn own(table: Option<&str>) -> Result<(), ErrorKind> {
            match table {
                None | Some("Orders") => Ok(()),
                Some(_) => Err(ErrorKind::Ref),
            }
        }
    }

    impl Context for Orders {
        fn this_row(&self, table: Option<&str>, column: &str) -> Result<Value, ErrorKind> {
            Self::own(table)?;
            self.columns
                .get(column)
                .map(|v| v[self.row].clone())
                .ok_or(ErrorKind::Ref)
        }

        fn column(&self, table: Option<&str>, column: &str) -> Result<Cow<'_, [Value]>, ErrorKind> {
            Self::own(table)?;
            self.columns
                .get(column)
                .map(|v| Cow::Borrowed(v.as_slice()))
                .ok_or(ErrorKind::Ref)
        }

        fn name(&self, name: &str) -> Result<Arg<'_>, ErrorKind> {
            if name == "TAXRATE" {
                Ok(Arg::Value(Value::Number(0.25)))
            } else {
                Err(ErrorKind::Name)
            }
        }
    }

    fn orders() -> Orders {
        let n = |v: &[f64]| v.iter().map(|x| Value::Number(*x)).collect::<Vec<_>>();
        Orders {
            columns: HashMap::from([
                ("Qty", n(&[2.0, 3.0])),
                ("Price", n(&[1.5, 10.0])),
                ("Note", vec![Value::Text("x".into()), Value::Empty]),
            ]),
            row: 0,
        }
    }

    fn eval(text: &str, ctx: &dyn Context) -> Value {
        evaluate(&parse(text).unwrap(), ctx)
    }

    #[test]
    fn row_formulas_use_this_rows_values() {
        let mut ctx = orders();
        assert_eq!(eval("=[@Qty]*[@Price]", &ctx), Value::Number(3.0));
        assert_eq!(
            eval("=Orders[@Qty]*Orders[@Price]", &ctx),
            Value::Number(3.0),
            "own table by name"
        );
        ctx.row = 1;
        assert_eq!(eval("=[@Qty]*[@Price]", &ctx), Value::Number(30.0));
        assert_eq!(eval("=[@Note]&\"!\"", &ctx), Value::Text("!".into()));
        assert_eq!(
            eval("=[@Note]", &ctx),
            Value::Number(0.0),
            "an empty result shows as 0"
        );
        assert_eq!(
            eval("=taxrate*4", &ctx),
            Value::Number(1.0),
            "names ignore case"
        );
    }

    #[test]
    fn missing_things_give_excels_errors() {
        let ctx = orders();
        assert_eq!(eval("=[@Gone]+1", &ctx), Value::Error(ErrorKind::Ref));
        assert_eq!(eval("=Other[@Qty]", &ctx), Value::Error(ErrorKind::Ref));
        assert_eq!(eval("=Unknown*2", &ctx), Value::Error(ErrorKind::Name));
        assert_eq!(
            eval("=NOSUCHFUNCTION(1)", &ctx),
            Value::Error(ErrorKind::Name)
        );
        assert_eq!(
            eval("=B2", &ctx),
            Value::Error(ErrorKind::Ref),
            "tables have no cells"
        );
    }

    #[test]
    fn whole_columns_where_one_value_is_wanted_are_value_errors() {
        let ctx = orders();
        assert_eq!(eval("=[Qty]", &ctx), Value::Error(ErrorKind::Value));
        assert_eq!(eval("=Orders[Qty]+1", &ctx), Value::Error(ErrorKind::Value));
        assert_eq!(eval("=Missing[Qty]", &ctx), Value::Error(ErrorKind::Ref));
    }

    #[test]
    fn operators_on_columns_work_element_by_element() {
        let ctx = orders();
        let expr = parse("=([Qty]>2)*[Price]").unwrap();
        let arg = eval_arg(&expr, &ctx);
        let Arg::Range(r) = arg else {
            panic!("a range")
        };
        assert_eq!((r.rows(), r.cols()), (2, 1));
        assert_eq!(r.values(), [Value::Number(0.0), Value::Number(10.0)]);
        let short = Range::column(Cow::Owned(vec![Value::Number(1.0)]));
        let long = Range::new(1, 3, Cow::Owned(vec![Value::Number(1.0); 3])).unwrap();
        let Arg::Range(r) = apply_binary(BinaryOp::Add, Arg::Range(short), Arg::Range(long)) else {
            panic!("a range")
        };
        assert_eq!(
            (r.rows(), r.cols()),
            (1, 3),
            "a single cell stretches to fit"
        );
        let two_rows = Range::column(Cow::Owned(vec![Value::Number(1.0); 2]));
        let three_rows = Range::column(Cow::Owned(vec![Value::Number(1.0); 3]));
        let Arg::Range(r) =
            apply_binary(BinaryOp::Add, Arg::Range(two_rows), Arg::Range(three_rows))
        else {
            panic!("a range")
        };
        assert_eq!(
            r.values[2],
            Value::Error(ErrorKind::NA),
            "beyond the shorter range"
        );
    }

    #[test]
    fn references_are_told_apart_from_literals() {
        let ctx = orders();
        let exprs = [
            parse("[@Qty]").unwrap(),
            parse("2").unwrap(),
            parse("[Qty]").unwrap(),
        ];
        let args = Args {
            exprs: &exprs,
            ctx: &ctx,
        };
        assert_eq!(args.arg(0), Arg::Ref(Value::Number(2.0)));
        assert_eq!(args.arg(1), Arg::Value(Value::Number(2.0)));
        assert!(
            matches!(args.arg(2), Arg::Range(r) if matches!(r.values, Cow::Borrowed(_))),
            "columns are borrowed"
        );
    }

    #[test]
    fn functions_dispatch_through_the_registry() {
        let ctx = orders();
        assert_eq!(eval("=TRUE()", &ctx), Value::Bool(true));
        assert_eq!(eval("=na()", &ctx), Value::Error(ErrorKind::NA));
        assert_eq!(
            eval("=TRUE(1)", &ctx),
            Value::Error(ErrorKind::Value),
            "too many arguments"
        );
        let names: Vec<&str> = functions().iter().map(|f| f.name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            names, sorted,
            "the registry must stay sorted and unique for lookup"
        );
        assert!(
            functions()
                .iter()
                .all(|f| f.name == f.name.to_ascii_uppercase())
        );
    }

    #[test]
    fn arguments_are_not_evaluated_until_asked_for() {
        struct Panics;
        impl Context for Panics {
            fn this_row(&self, _: Option<&str>, _: &str) -> Result<Value, ErrorKind> {
                panic!("evaluated an argument nobody asked for")
            }
            fn column(&self, _: Option<&str>, _: &str) -> Result<Cow<'_, [Value]>, ErrorKind> {
                panic!("evaluated an argument nobody asked for")
            }
        }
        // Dispatch refuses the arity without touching the argument.
        assert_eq!(
            eval("=TRUE([@Qty])", &Panics),
            Value::Error(ErrorKind::Value)
        );
        let exprs = [parse("[@Qty]").unwrap()];
        let args = Args {
            exprs: &exprs,
            ctx: &Panics,
        };
        assert_eq!(args.len(), 1);
        assert!(!args.is_missing(0));
        assert!(args.is_missing(1));
    }

    #[test]
    fn long_operator_chains_do_not_overflow_the_stack() {
        // Run on a thread with the default 2 MiB stack, the size test threads get.
        std::thread::spawn(|| {
            let ctx = orders();
            let plus = format!("=1{}", "+1".repeat(4000));
            assert_eq!(eval(&plus, &ctx), Value::Number(4001.0));
            let percents = format!("=1{}", "%".repeat(8000));
            assert_eq!(eval(&percents, &ctx), Value::Number(0.0));
        })
        .join()
        .unwrap();
    }

    /// 100,000 row formulas of the `[@Qty]*[@Price]` kind. Run with
    /// `cargo test --release -p xel-engine row_formula_benchmark -- --ignored --nocapture`.
    #[test]
    #[ignore = "benchmark"]
    fn row_formula_benchmark() {
        let n = 100_000;
        let col = |f: fn(usize) -> f64| (0..n).map(|i| Value::Number(f(i))).collect();
        #[expect(clippy::cast_precision_loss, reason = "small test indices")]
        let mut ctx = Orders {
            columns: HashMap::from([
                ("Qty", col(|i| i as f64)),
                ("Price", col(|i| i as f64 / 2.0)),
            ]),
            row: 0,
        };
        let expr = parse("=[@Qty]*[@Price]+1").unwrap();
        let start = std::time::Instant::now();
        let mut total = 0.0;
        for row in 0..n {
            ctx.row = row;
            total += evaluate(&expr, &ctx).to_number().unwrap();
        }
        let elapsed = start.elapsed();
        eprintln!("{n} row formulas in {elapsed:?} (checksum {total})");
        // The budget is for optimized builds; debug builds only report.
        if !cfg!(debug_assertions) {
            assert!(
                elapsed < std::time::Duration::from_millis(50),
                "{elapsed:?}"
            );
        }
    }
}
