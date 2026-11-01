//! Evaluating a parsed formula against whatever holds the data.
//!
//! The evaluator knows nothing about storage. A [`Context`] answers references: the
//! current row's value of a column, a whole column, a cell or range on a sheet, a name.
//! Table formula columns, calc blocks, and the Python package each supply their own.

use super::ast::{A1, ErrorKind, Expr, Reference};
use super::value::{Value, binary, unary};

/// Where a formula's references are looked up.
pub trait Context {
    /// `[@Col]`: this row's value of a column in the formula's own table.
    fn this_row(&self, column: &str) -> Result<Value, ErrorKind>;

    /// `[Col]` (own table, `table` is `None`) or `Table[Col]`: every value of a column.
    fn column(&self, table: Option<&str>, column: &str) -> Result<Vec<Value>, ErrorKind>;

    /// `A1`. Only calc blocks have cells; a table formula referring to one is `#REF!`.
    fn cell(&self, _cell: A1) -> Result<Value, ErrorKind> {
        Err(ErrorKind::Ref)
    }

    /// `A1:B2`, row by row.
    fn range(&self, _from: A1, _to: A1) -> Result<Vec<Value>, ErrorKind> {
        Err(ErrorKind::Ref)
    }

    /// A defined name, such as a named LAMBDA or constant.
    fn name(&self, _name: &str) -> Result<Value, ErrorKind> {
        Err(ErrorKind::Name)
    }
}

/// A function argument as the function sees it: one value, or the values of a range.
#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    Value(Value),
    Range(Vec<Value>),
}

/// A function's arguments, evaluated only when the function asks for them, so `IF` can
/// skip the branch it does not take and `IFERROR` can catch an error.
pub struct Args<'a> {
    exprs: &'a [Expr],
    ctx: &'a dyn Context,
}

impl Args<'_> {
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

    /// Argument `i` as one value. A range where a value is wanted is `#VALUE!`; a
    /// missing argument is `Empty`.
    #[must_use]
    pub fn value(&self, i: usize) -> Value {
        match self.arg(i) {
            Arg::Value(v) => v,
            Arg::Range(_) => Value::Error(ErrorKind::Value),
        }
    }

    /// Argument `i` as written: a single value or a range.
    #[must_use]
    pub fn arg(&self, i: usize) -> Arg {
        self.exprs
            .get(i)
            .map_or(Arg::Value(Value::Empty), |e| eval_arg(e, self.ctx))
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
    pub eval: fn(&Args<'_>) -> Value,
}

/// Every built-in function, sorted by name. Function families add themselves here.
static FUNCTIONS: &[Function] = &[
    Function {
        name: "FALSE",
        min_args: 0,
        max_args: Some(0),
        volatile: false,
        signature: "FALSE()",
        description: "Returns the logical value FALSE.",
        eval: |_| Value::Bool(false),
    },
    Function {
        name: "NA",
        min_args: 0,
        max_args: Some(0),
        volatile: false,
        signature: "NA()",
        description: "Returns the error value #N/A.",
        eval: |_| Value::Error(ErrorKind::NA),
    },
    Function {
        name: "TRUE",
        min_args: 0,
        max_args: Some(0),
        volatile: false,
        signature: "TRUE()",
        description: "Returns the logical value TRUE.",
        eval: |_| Value::Bool(true),
    },
];

/// All built-in functions, sorted by name.
#[must_use]
pub fn functions() -> &'static [Function] {
    FUNCTIONS
}

/// Looks up a built-in function by its upper-case name.
#[must_use]
pub fn function(name: &str) -> Option<&'static Function> {
    FUNCTIONS
        .binary_search_by(|f| f.name.cmp(name))
        .ok()
        .map(|i| &FUNCTIONS[i])
}

/// Evaluates a formula to a single value.
#[must_use]
pub fn evaluate(expr: &Expr, ctx: &dyn Context) -> Value {
    match eval_arg(expr, ctx) {
        Arg::Value(v) => v,
        // A whole range where one value is wanted. Excel would spill or intersect;
        // neither applies to a table cell.
        Arg::Range(_) => Value::Error(ErrorKind::Value),
    }
}

fn eval_arg(expr: &Expr, ctx: &dyn Context) -> Arg {
    let value = |r: Result<Value, ErrorKind>| Arg::Value(r.unwrap_or_else(Value::Error));
    let range = |r: Result<Vec<Value>, ErrorKind>| match r {
        Ok(values) => Arg::Range(values),
        Err(e) => Arg::Value(Value::Error(e)),
    };
    match expr {
        Expr::Number(n) => Arg::Value(Value::number(*n)),
        Expr::Text(s) => Arg::Value(Value::Text(s.clone())),
        Expr::Bool(b) => Arg::Value(Value::Bool(*b)),
        Expr::Error(e) => Arg::Value(Value::Error(*e)),
        Expr::Missing => Arg::Value(Value::Empty),
        Expr::Name(n) => value(ctx.name(n)),
        Expr::Ref(Reference::Structured {
            table,
            column,
            this_row,
        }) => {
            if *this_row {
                if table.is_some() {
                    // `Other[@Col]` names a row in a table the formula is not in.
                    return Arg::Value(Value::Error(ErrorKind::Value));
                }
                value(ctx.this_row(column))
            } else {
                range(ctx.column(table.as_deref(), column))
            }
        }
        Expr::Ref(Reference::Cell(a)) => value(ctx.cell(*a)),
        Expr::Ref(Reference::Range(a, b)) => range(ctx.range(*a, *b)),
        Expr::Unary { op, expr } => Arg::Value(unary(*op, &evaluate(expr, ctx))),
        Expr::Binary { op, left, right } => {
            Arg::Value(binary(*op, &evaluate(left, ctx), &evaluate(right, ctx)))
        }
        Expr::Call { name, args } => Arg::Value(call(name, args, ctx)),
    }
}

fn call(name: &str, args: &[Expr], ctx: &dyn Context) -> Value {
    let Some(f) = function(name) else {
        return Value::Error(ErrorKind::Name);
    };
    if args.len() < f.min_args || f.max_args.is_some_and(|max| args.len() > max) {
        return Value::Error(ErrorKind::Value);
    }
    (f.eval)(&Args { exprs: args, ctx })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::formula::parse;

    /// A table row in memory: column name to values, and the index of the current row.
    struct Rows {
        columns: HashMap<&'static str, Vec<Value>>,
        row: usize,
    }

    impl Context for Rows {
        fn this_row(&self, column: &str) -> Result<Value, ErrorKind> {
            self.columns
                .get(column)
                .map(|v| v[self.row].clone())
                .ok_or(ErrorKind::Ref)
        }

        fn column(&self, table: Option<&str>, column: &str) -> Result<Vec<Value>, ErrorKind> {
            if table.is_some_and(|t| t != "Orders") {
                return Err(ErrorKind::Ref);
            }
            self.columns.get(column).cloned().ok_or(ErrorKind::Ref)
        }
    }

    fn orders() -> Rows {
        let n = |v: &[f64]| v.iter().map(|x| Value::Number(*x)).collect::<Vec<_>>();
        Rows {
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
        ctx.row = 1;
        assert_eq!(eval("=[@Qty]*[@Price]", &ctx), Value::Number(30.0));
        assert_eq!(eval("=[@Note]&\"!\"", &ctx), Value::Text("!".into()));
    }

    #[test]
    fn missing_columns_are_ref_errors_and_unknown_names_are_name_errors() {
        let ctx = orders();
        assert_eq!(eval("=[@Gone]+1", &ctx), Value::Error(ErrorKind::Ref));
        assert_eq!(eval("=TaxRate*2", &ctx), Value::Error(ErrorKind::Name));
        assert_eq!(
            eval("=NOSUCHFUNCTION(1)", &ctx),
            Value::Error(ErrorKind::Name)
        );
        assert_eq!(
            eval("=B2", &ctx),
            Value::Error(ErrorKind::Ref),
            "tables have no cells"
        );
        assert_eq!(eval("=Other[@Qty]", &ctx), Value::Error(ErrorKind::Value));
    }

    #[test]
    fn whole_columns_where_one_value_is_wanted_are_value_errors() {
        let ctx = orders();
        assert_eq!(eval("=[Qty]", &ctx), Value::Error(ErrorKind::Value));
        assert_eq!(eval("=Orders[Qty]+1", &ctx), Value::Error(ErrorKind::Value));
        assert_eq!(eval("=Missing[Qty]", &ctx), Value::Error(ErrorKind::Ref));
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
    fn arguments_are_lazy() {
        struct Panics;
        impl Context for Panics {
            fn this_row(&self, _: &str) -> Result<Value, ErrorKind> {
                panic!("evaluated an argument nobody asked for")
            }
            fn column(&self, _: Option<&str>, _: &str) -> Result<Vec<Value>, ErrorKind> {
                panic!("evaluated an argument nobody asked for")
            }
        }
        let expr = parse("=TRUE()").unwrap();
        let Expr::Call { .. } = &expr else {
            panic!("a call")
        };
        let exprs = [parse("[@Qty]").unwrap()];
        let args = Args {
            exprs: &exprs,
            ctx: &Panics,
        };
        assert_eq!(args.len(), 1);
        assert!(!args.is_missing(0));
        assert!(args.is_missing(1));
        assert_eq!(evaluate(&expr, &Panics), Value::Bool(true));
    }

    /// 100,000 row formulas of the `[@Qty]*[@Price]` kind. Run with
    /// `cargo test --release -p xel-engine row_formula_benchmark -- --ignored --nocapture`.
    #[test]
    #[ignore = "benchmark"]
    fn row_formula_benchmark() {
        let n = 100_000;
        let col = |f: fn(usize) -> f64| (0..n).map(|i| Value::Number(f(i))).collect();
        #[expect(clippy::cast_precision_loss, reason = "small test indices")]
        let mut ctx = Rows {
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
        assert!(
            elapsed < std::time::Duration::from_millis(50),
            "{elapsed:?}"
        );
    }
}
