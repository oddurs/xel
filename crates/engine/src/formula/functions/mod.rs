//! Built-in functions, grouped by family. The registry below lists every one, sorted by
//! name for binary search; a test keeps it sorted.

mod criteria;
mod logic;
mod math;

use super::ast::ErrorKind;
use super::eval::{Arg, Args, Function};
use super::value::{Value, number_from_text};

/// A non-volatile function entry.
macro_rules! func {
    ($name:literal, $min:expr, $max:expr, $sig:literal, $desc:literal, $eval:expr) => {
        Function {
            name: $name,
            min_args: $min,
            max_args: $max,
            volatile: false,
            signature: $sig,
            description: $desc,
            eval: $eval,
        }
    };
}

pub(crate) static FUNCTIONS: &[Function] = &[
    func!(
        "ABS",
        1,
        Some(1),
        "ABS(number)",
        "Returns the absolute value of a number.",
        math::abs
    ),
    func!(
        "AND",
        1,
        None,
        "AND(logical1, [logical2], ...)",
        "Returns TRUE if all of its arguments are TRUE.",
        logic::and
    ),
    func!(
        "AVERAGE",
        1,
        None,
        "AVERAGE(number1, [number2], ...)",
        "Returns the average of its arguments.",
        math::average
    ),
    func!(
        "AVERAGEIF",
        2,
        Some(3),
        "AVERAGEIF(range, criteria, [average_range])",
        "Averages the cells that meet a criterion.",
        criteria::averageif
    ),
    func!(
        "COUNT",
        1,
        None,
        "COUNT(value1, [value2], ...)",
        "Counts how many numbers are in the list of arguments.",
        math::count
    ),
    func!(
        "COUNTA",
        1,
        None,
        "COUNTA(value1, [value2], ...)",
        "Counts how many values are in the list of arguments.",
        math::counta
    ),
    func!(
        "COUNTBLANK",
        1,
        Some(1),
        "COUNTBLANK(range)",
        "Counts the number of blank cells within a range.",
        math::countblank
    ),
    func!(
        "COUNTIF",
        2,
        Some(2),
        "COUNTIF(range, criteria)",
        "Counts the cells in a range that meet a criterion.",
        criteria::countif
    ),
    func!(
        "FALSE",
        0,
        Some(0),
        "FALSE()",
        "Returns the logical value FALSE.",
        |_| Arg::Value(Value::Bool(false))
    ),
    func!(
        "IF",
        1,
        Some(3),
        "IF(logical_test, [value_if_true], [value_if_false])",
        "Returns one value if a condition is TRUE and another if it is FALSE.",
        logic::if_
    ),
    func!(
        "IFERROR",
        2,
        Some(2),
        "IFERROR(value, value_if_error)",
        "Returns value_if_error if a value is an error, otherwise the value.",
        logic::iferror
    ),
    func!(
        "IFNA",
        2,
        Some(2),
        "IFNA(value, value_if_na)",
        "Returns value_if_na if a value is #N/A, otherwise the value.",
        logic::ifna
    ),
    func!(
        "IFS",
        2,
        None,
        "IFS(logical_test1, value_if_true1, ...)",
        "Returns the value for the first condition that is TRUE.",
        logic::ifs
    ),
    func!(
        "INT",
        1,
        Some(1),
        "INT(number)",
        "Rounds a number down to the nearest integer.",
        math::int
    ),
    func!(
        "MAX",
        1,
        None,
        "MAX(number1, [number2], ...)",
        "Returns the largest number in a list of arguments.",
        math::max
    ),
    func!(
        "MIN",
        1,
        None,
        "MIN(number1, [number2], ...)",
        "Returns the smallest number in a list of arguments.",
        math::min
    ),
    func!(
        "MOD",
        2,
        Some(2),
        "MOD(number, divisor)",
        "Returns the remainder after a number is divided by a divisor.",
        math::mod_
    ),
    func!(
        "NA",
        0,
        Some(0),
        "NA()",
        "Returns the error value #N/A.",
        |_| Arg::Value(Value::Error(ErrorKind::NA))
    ),
    func!(
        "NOT",
        1,
        Some(1),
        "NOT(logical)",
        "Reverses the logic of its argument.",
        logic::not
    ),
    func!(
        "OR",
        1,
        None,
        "OR(logical1, [logical2], ...)",
        "Returns TRUE if any argument is TRUE.",
        logic::or
    ),
    func!(
        "POWER",
        2,
        Some(2),
        "POWER(number, power)",
        "Returns the result of a number raised to a power.",
        math::power
    ),
    func!(
        "ROUND",
        2,
        Some(2),
        "ROUND(number, num_digits)",
        "Rounds a number to a specified number of digits.",
        math::round
    ),
    func!(
        "ROUNDDOWN",
        2,
        Some(2),
        "ROUNDDOWN(number, num_digits)",
        "Rounds a number down, toward zero.",
        math::rounddown
    ),
    func!(
        "ROUNDUP",
        2,
        Some(2),
        "ROUNDUP(number, num_digits)",
        "Rounds a number up, away from zero.",
        math::roundup
    ),
    func!(
        "SQRT",
        1,
        Some(1),
        "SQRT(number)",
        "Returns a positive square root.",
        math::sqrt
    ),
    func!(
        "SUM",
        1,
        None,
        "SUM(number1, [number2], ...)",
        "Adds its arguments.",
        math::sum
    ),
    func!(
        "SUMIF",
        2,
        Some(3),
        "SUMIF(range, criteria, [sum_range])",
        "Adds the cells that meet a criterion.",
        criteria::sumif
    ),
    func!(
        "SWITCH",
        3,
        None,
        "SWITCH(expression, value1, result1, ..., [default])",
        "Returns the result for the first value that matches the expression.",
        logic::switch
    ),
    func!(
        "TRUE",
        0,
        Some(0),
        "TRUE()",
        "Returns the logical value TRUE.",
        |_| Arg::Value(Value::Bool(true))
    ),
    func!(
        "XOR",
        1,
        None,
        "XOR(logical1, [logical2], ...)",
        "Returns TRUE if an odd number of arguments are TRUE.",
        logic::xor
    ),
];

/// Visits every value an aggregate sees, with Excel's rule for which ones count: values
/// typed in the formula are converted (`SUM("1", TRUE)` is 2), while text, booleans, and
/// blanks inside references and ranges are skipped. `visit` gets each value and whether
/// it came from a cell; returning an error stops the walk.
fn each_value(
    args: &Args<'_>,
    mut visit: impl FnMut(&Value, bool) -> Result<(), ErrorKind>,
) -> Result<(), ErrorKind> {
    for i in 0..args.len() {
        match args.arg(i) {
            Arg::Value(v) => visit(&v, false)?,
            Arg::Ref(v) => visit(&v, true)?,
            Arg::Range(r) => {
                for v in r.values() {
                    visit(v, true)?;
                }
            }
        }
    }
    Ok(())
}

/// The numbers an aggregate like SUM adds, by Excel's rules. Errors anywhere stop it.
fn numbers(args: &Args<'_>) -> Result<Vec<f64>, ErrorKind> {
    let mut out = Vec::new();
    each_value(args, |v, from_cell| {
        match (v, from_cell) {
            (Value::Number(n) | Value::Date(n), _) => out.push(*n),
            (Value::Error(e), _) => return Err(*e),
            (Value::Bool(b), false) => out.push(f64::from(u8::from(*b))),
            (Value::Text(s), false) => out.push(number_from_text(s).ok_or(ErrorKind::Value)?),
            _ => {}
        }
        Ok(())
    })?;
    Ok(out)
}

/// A value as a whole number of digits or a count, truncated toward zero as Excel does.
fn integer(v: &Value) -> Result<i64, ErrorKind> {
    let n = v.to_number()?.trunc();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "clamped to a sane range first"
    )]
    Ok(n.clamp(-1e15, 1e15) as i64)
}

/// A value as TRUE or FALSE, as `IF` and `NOT` read their conditions.
fn truth(v: &Value) -> Result<bool, ErrorKind> {
    match v {
        Value::Bool(b) => Ok(*b),
        Value::Number(n) | Value::Date(n) => Ok(*n != 0.0),
        Value::Empty => Ok(false),
        Value::Text(s) if s.eq_ignore_ascii_case("TRUE") => Ok(true),
        Value::Text(s) if s.eq_ignore_ascii_case("FALSE") => Ok(false),
        Value::Text(_) => Err(ErrorKind::Value),
        Value::Error(e) => Err(*e),
    }
}

fn value(v: Value) -> Arg<'static> {
    Arg::Value(v)
}

fn result(r: Result<Value, ErrorKind>) -> Arg<'static> {
    Arg::Value(r.unwrap_or_else(Value::Error))
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::borrow::Cow;
    use std::collections::HashMap;

    use crate::formula::{Context, ErrorKind, Value, evaluate, parse};

    /// A table named T with a few columns of mixed values, for function tests.
    pub(crate) struct Sheet(pub(crate) HashMap<&'static str, Vec<Value>>);

    impl Context for Sheet {
        fn this_row(&self, _: Option<&str>, column: &str) -> Result<Value, ErrorKind> {
            self.0
                .get(column)
                .map(|v| v[0].clone())
                .ok_or(ErrorKind::Ref)
        }

        fn column(&self, _: Option<&str>, column: &str) -> Result<Cow<'_, [Value]>, ErrorKind> {
            self.0
                .get(column)
                .map(|v| Cow::Borrowed(v.as_slice()))
                .ok_or(ErrorKind::Ref)
        }
    }

    pub(crate) fn sheet() -> Sheet {
        let n = Value::Number;
        let t = |s: &str| Value::Text(s.into());
        Sheet(HashMap::from([
            ("Nums", vec![n(1.0), n(2.0), n(3.0), n(4.0)]),
            (
                "Mixed",
                vec![n(1.0), t("2"), Value::Bool(true), Value::Empty, n(5.0)],
            ),
            (
                "Words",
                vec![t("apple"), t("Banana"), t("cherry"), t(""), Value::Empty],
            ),
            ("Errs", vec![n(1.0), Value::Error(ErrorKind::Div0), n(3.0)]),
            ("Zero", vec![n(0.0), n(2.0), n(0.0)]),
            (
                "Bools",
                vec![Value::Bool(true), Value::Bool(false), t("x"), Value::Empty],
            ),
            ("Empty", vec![]),
            ("One", vec![n(7.0)]),
        ]))
    }

    pub(crate) fn eval(text: &str) -> Value {
        evaluate(
            &parse(text).unwrap_or_else(|e| panic!("{text}: {e}")),
            &sheet(),
        )
    }

    /// Checks each `(formula, expected)` pair; numbers compare at 15 significant digits.
    pub(crate) fn check(cases: &[(&str, Value)]) {
        for (text, want) in cases {
            let got = eval(text);
            let same = match (&got, want) {
                (Value::Number(_), Value::Number(_)) => {
                    crate::formula::compare(&got, want) == Ok(std::cmp::Ordering::Equal)
                }
                _ => got == *want,
            };
            assert!(same, "{text}: got {got:?}, want {want:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_is_sorted_unique_and_complete() {
        let names: Vec<&str> = FUNCTIONS.iter().map(|f| f.name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(names, sorted);
        for f in FUNCTIONS {
            assert!(f.signature.starts_with(f.name), "{}: signature", f.name);
            assert!(f.description.ends_with('.'), "{}: description", f.name);
            assert!(
                f.max_args.is_none_or(|max| max >= f.min_args),
                "{}: arity",
                f.name
            );
        }
    }
}
