//! Logic: IF, IFS, IFERROR, IFNA, AND, OR, XOR, NOT, SWITCH.

use std::cmp::Ordering;

use super::{each_value, result, truth, value};
use crate::formula::ast::ErrorKind;
use crate::formula::eval::{Arg, Args, Range, as_range, broadcast};
use crate::formula::value::{Value, compare};

/// A branch or fallback as a range for broadcasting. An empty range stands for a blank:
/// it must not shrink the result, which follows the test or value being checked.
fn secondary(arg: Arg<'_>) -> Range<'_> {
    let r = as_range(arg);
    if r.values().is_empty() {
        Range::single(Value::Empty)
    } else {
        r
    }
}

/// `IF(test, [if_true], [if_false])`. Only the branch taken is evaluated, and it passes
/// through as written, so a reference or array stays one. A test that is a range picks
/// element by element, as in `SUM(IF([Qty]>1, [Price], 0))`.
pub(super) fn if_<'a>(args: &Args<'a>) -> Arg<'a> {
    // An omitted branch gives 0 when its argument slot exists but is empty, and FALSE
    // when if_false is left out altogether.
    let branch = |i: usize| -> Arg<'a> {
        if i >= args.len() {
            value(Value::Bool(false))
        } else if args.is_missing(i) {
            value(Value::Number(0.0))
        } else {
            args.arg(i)
        }
    };
    match args.arg(0) {
        Arg::Range(test) => {
            let ranges = [test, secondary(branch(1)), secondary(branch(2))];
            Arg::Range(broadcast(&ranges, |v| match truth(&v[0]) {
                Ok(true) => v[1].clone(),
                Ok(false) => v[2].clone(),
                Err(e) => Value::Error(e),
            }))
        }
        Arg::Value(test) | Arg::Ref(test) => match truth(&test) {
            Ok(true) => branch(1),
            Ok(false) => branch(2),
            Err(e) => value(Value::Error(e)),
        },
    }
}

/// `IFS(test1, value1, test2, value2, …)`: the value for the first TRUE test; `#N/A` when
/// none is.
pub(super) fn ifs<'a>(args: &Args<'a>) -> Arg<'a> {
    if !args.len().is_multiple_of(2) {
        return value(Value::Error(ErrorKind::Value));
    }
    for i in (0..args.len()).step_by(2) {
        match truth(&args.value(i)) {
            Ok(true) => return args.arg(i + 1),
            Ok(false) => {}
            Err(e) => return value(Value::Error(e)),
        }
    }
    value(Value::Error(ErrorKind::NA))
}

/// Replaces errors that `catches` accepts, element by element for an array.
fn replace_errors<'a>(args: &Args<'a>, catches: fn(ErrorKind) -> bool) -> Arg<'a> {
    let fallback = || {
        if args.is_missing(1) {
            value(Value::Number(0.0))
        } else {
            args.arg(1)
        }
    };
    match args.arg(0) {
        Arg::Range(r) => {
            let ranges = [r, secondary(fallback())];
            Arg::Range(broadcast(&ranges, |v| match &v[0] {
                Value::Error(e) if catches(*e) => v[1].clone(),
                other => other.clone(),
            }))
        }
        Arg::Value(Value::Error(e)) | Arg::Ref(Value::Error(e)) if catches(e) => fallback(),
        other => other,
    }
}

pub(super) fn iferror<'a>(args: &Args<'a>) -> Arg<'a> {
    replace_errors(args, |_| true)
}

pub(super) fn ifna<'a>(args: &Args<'a>) -> Arg<'a> {
    replace_errors(args, |e| e == ErrorKind::NA)
}

/// The logical values AND, OR, and XOR consider. Typed text must read as TRUE or FALSE;
/// text and blanks in cells are skipped. With nothing logical at all, the result is
/// `#VALUE!`, as in Excel.
fn logicals(args: &Args<'_>) -> Result<Vec<bool>, ErrorKind> {
    let mut out = Vec::new();
    each_value(args, |v, from_cell| {
        match (v, from_cell) {
            (Value::Empty, _) | (Value::Text(_), true) => {}
            (Value::Bool(b), _) => out.push(*b),
            (Value::Number(n) | Value::Date(n), _) => out.push(*n != 0.0),
            (Value::Error(e), _) => return Err(*e),
            (v, false) => out.push(truth(v)?),
        }
        Ok(())
    })?;
    if out.is_empty() {
        return Err(ErrorKind::Value);
    }
    Ok(out)
}

pub(super) fn and<'a>(args: &Args<'a>) -> Arg<'a> {
    result(logicals(args).map(|bs| Value::Bool(bs.iter().all(|b| *b))))
}

pub(super) fn or<'a>(args: &Args<'a>) -> Arg<'a> {
    result(logicals(args).map(|bs| Value::Bool(bs.iter().any(|b| *b))))
}

pub(super) fn xor<'a>(args: &Args<'a>) -> Arg<'a> {
    result(logicals(args).map(|bs| Value::Bool(bs.iter().filter(|b| **b).count() % 2 == 1)))
}

pub(super) fn not<'a>(args: &Args<'a>) -> Arg<'a> {
    result(truth(&args.value(0)).map(|b| Value::Bool(!b)))
}

/// `SWITCH(expr, value1, result1, …, [default])`: the result for the first value equal
/// to `expr` (compared as `=` compares); the default, or `#N/A`, when none is.
pub(super) fn switch<'a>(args: &Args<'a>) -> Arg<'a> {
    let subject = args.value(0);
    if let Value::Error(e) = subject {
        return value(Value::Error(e));
    }
    let pairs = (args.len() - 1) / 2;
    for p in 0..pairs {
        let candidate = args.value(1 + 2 * p);
        match compare(&subject, &candidate) {
            Ok(Ordering::Equal) => return args.arg(2 + 2 * p),
            Ok(_) => {}
            Err(e) => return value(Value::Error(e)),
        }
    }
    if args.len().is_multiple_of(2) {
        args.arg(args.len() - 1)
    } else {
        value(Value::Error(ErrorKind::NA))
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::check;
    use crate::formula::{ErrorKind as E, Value};

    fn n(x: f64) -> Value {
        Value::Number(x)
    }

    fn b(x: bool) -> Value {
        Value::Bool(x)
    }

    fn t(s: &str) -> Value {
        Value::Text(s.into())
    }

    fn err(e: E) -> Value {
        Value::Error(e)
    }

    #[test]
    fn if_takes_one_branch_and_fills_omitted_ones_like_excel() {
        check(&[
            ("=IF(TRUE,1,2)", n(1.0)),
            ("=IF(0,1,2)", n(2.0)),
            ("=IF(\"true\",1,2)", n(1.0)),
            ("=IF(\"x\",1,2)", err(E::Value)),
            ("=IF(FALSE,1)", b(false)),
            ("=IF(TRUE)", err(E::Value)),
            ("=IF(TRUE,,1)", n(0.0)),
            ("=IF(FALSE,1,)", n(0.0)),
            ("=IF(NA(),1,2)", err(E::NA)),
            ("=IF(TRUE,1,1/0)", n(1.0)),
            ("=IF([@Nums]>0,\"pos\",\"neg\")", t("pos")),
        ]);
    }

    #[test]
    fn if_over_a_range_picks_element_by_element() {
        check(&[
            ("=SUM(IF([Nums]>2,[Nums],0))", n(7.0)),
            ("=SUM(IF([Nums]>2,1))", n(2.0)),
            ("=SUM(IF([Empty]>2,1,0))", n(0.0)),
        ]);
    }

    #[test]
    fn ifs_and_switch() {
        check(&[
            ("=IFS(FALSE,1,TRUE,2)", n(2.0)),
            ("=IFS(FALSE,1)", err(E::NA)),
            ("=IFS(TRUE,1,FALSE)", err(E::Value)),
            ("=IFS(1/0,1,TRUE,2)", err(E::Div0)),
            ("=SWITCH(2,1,\"one\",2,\"two\")", t("two")),
            ("=SWITCH(\"B\",\"a\",1,\"b\",2)", n(2.0)),
            ("=SWITCH(3,1,\"one\",\"other\")", t("other")),
            ("=SWITCH(3,1,\"one\",2,\"two\")", err(E::NA)),
            ("=SWITCH(NA(),1,2)", err(E::NA)),
        ]);
    }

    #[test]
    fn iferror_and_ifna_catch_what_they_should() {
        check(&[
            ("=IFERROR(1/0,\"x\")", t("x")),
            ("=IFERROR(5,\"x\")", n(5.0)),
            ("=IFERROR(1/0,)", n(0.0)),
            ("=IFNA(NA(),1)", n(1.0)),
            ("=IFNA(1/0,1)", err(E::Div0)),
            ("=IFNA(,1)", n(0.0)),
            ("=SUM(IFERROR([Errs],0))", n(4.0)),
            ("=SUM(IFERROR([Nums]/[Zero],0))", n(1.0)),
            ("=SUM(IFERROR([Nums],[Empty]))", n(10.0)),
            ("=SUM(IF([Nums]>2,[Nums],[Empty]))", n(7.0)),
        ]);
    }

    #[test]
    fn and_or_xor_not() {
        check(&[
            ("=AND(TRUE,1)", b(true)),
            ("=AND(TRUE,0)", b(false)),
            ("=OR(FALSE,0)", b(false)),
            ("=OR([Bools])", b(true)),
            ("=AND([Bools])", b(false)),
            ("=AND([Words])", err(E::Value)),
            ("=AND(\"x\")", err(E::Value)),
            ("=AND(\"TRUE\",TRUE)", b(true)),
            ("=OR([Errs])", err(E::Div0)),
            ("=XOR(TRUE,TRUE,TRUE)", b(true)),
            ("=XOR(TRUE,TRUE)", b(false)),
            ("=NOT(0)", b(true)),
            ("=NOT(\"x\")", err(E::Value)),
            ("=NOT()", err(E::Value)),
            ("=NOT(NA())", err(E::NA)),
        ]);
    }
}
