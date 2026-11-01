//! Aggregates and arithmetic: SUM, AVERAGE, MIN, MAX, COUNT, COUNTA, COUNTBLANK, ROUND,
//! ROUNDUP, ROUNDDOWN, INT, MOD, ABS, POWER, SQRT.

use super::{each_value, integer, numbers, result, value};
use crate::formula::ast::{BinaryOp, ErrorKind};
use crate::formula::eval::{Arg, Args};
use crate::formula::value::{Value, binary, number_from_text, round15};

pub(super) fn sum<'a>(args: &Args<'a>) -> Arg<'a> {
    result(numbers(args).map(|ns| Value::number(ns.iter().sum())))
}

pub(super) fn average<'a>(args: &Args<'a>) -> Arg<'a> {
    result(numbers(args).and_then(|ns| {
        if ns.is_empty() {
            return Err(ErrorKind::Div0);
        }
        #[expect(clippy::cast_precision_loss, reason = "counts stay far below 2^52")]
        Ok(Value::number(ns.iter().sum::<f64>() / ns.len() as f64))
    }))
}

/// MIN and MAX of nothing are 0, as in Excel.
fn extreme(args: &Args<'_>, pick: fn(f64, f64) -> f64) -> Arg<'static> {
    result(numbers(args).map(|ns| Value::number(ns.into_iter().reduce(pick).unwrap_or(0.0))))
}

pub(super) fn min<'a>(args: &Args<'a>) -> Arg<'a> {
    extreme(args, f64::min)
}

pub(super) fn max<'a>(args: &Args<'a>) -> Arg<'a> {
    extreme(args, f64::max)
}

/// Counts numbers. Typed values that read as numbers (`"1"`, `TRUE`) count; in cells only
/// real numbers do. Errors are skipped, not passed on.
pub(super) fn count<'a>(args: &Args<'a>) -> Arg<'a> {
    let mut n = 0_u32;
    let counted = each_value(args, |v, from_cell| {
        let is_number = match (v, from_cell) {
            (Value::Number(_) | Value::Date(_), _) | (Value::Bool(_), false) => true,
            (Value::Text(s), false) => number_from_text(s).is_some(),
            _ => false,
        };
        n += u32::from(is_number);
        Ok(())
    });
    result(counted.map(|()| Value::Number(f64::from(n))))
}

/// Counts anything that is not blank, including errors and empty text.
pub(super) fn counta<'a>(args: &Args<'a>) -> Arg<'a> {
    let filled = |v: &Value| !matches!(v, Value::Empty);
    let mut n = 0_usize;
    for i in 0..args.len() {
        if args.is_missing(i) {
            continue;
        }
        n += match args.arg(i) {
            Arg::Value(_) => 1,
            Arg::Ref(v) => usize::from(filled(&v)),
            Arg::Range(r) => r.values().iter().filter(|v| filled(v)).count(),
        };
    }
    #[expect(clippy::cast_precision_loss, reason = "counts stay far below 2^52")]
    value(Value::Number(n as f64))
}

/// Counts blank cells and empty text in a range.
pub(super) fn countblank<'a>(args: &Args<'a>) -> Arg<'a> {
    let blank =
        |v: &Value| matches!(v, Value::Empty) || matches!(v, Value::Text(s) if s.is_empty());
    let n = match args.arg(0) {
        Arg::Range(r) => r.values().iter().filter(|v| blank(v)).count(),
        Arg::Ref(v) => usize::from(blank(&v)),
        // Excel only accepts a range here.
        Arg::Value(_) => return value(Value::Error(ErrorKind::Value)),
    };
    #[expect(clippy::cast_precision_loss, reason = "counts stay far below 2^52")]
    value(Value::Number(n as f64))
}

#[derive(Clone, Copy)]
enum Rounding {
    /// Half away from zero, as Excel's ROUND.
    Nearest,
    Up,
    Down,
}

/// Rounds at `digits` decimal places. Scaling is rounded to 15 significant digits
/// first, so `ROUND(2.675, 2)` sees 267.5, not 267.49999999999997, and gives 2.68 as Excel.
fn round_with(args: &Args<'_>, mode: Rounding) -> Arg<'static> {
    let run = || -> Result<Value, ErrorKind> {
        let n = args.value(0).to_number()?;
        let digits = integer(&args.value(1))?.clamp(-308, 308);
        let scale = 10_f64.powi(i32::try_from(digits.unsigned_abs()).unwrap_or(308));
        let scaled = round15(if digits >= 0 { n * scale } else { n / scale });
        // Rounding beyond a double's precision changes nothing; it must not overflow.
        if !scaled.is_finite() {
            return Ok(Value::number(n));
        }
        let rounded = match mode {
            Rounding::Nearest => scaled.round(),
            Rounding::Up => scaled.abs().ceil().copysign(scaled),
            Rounding::Down => scaled.trunc(),
        };
        Ok(Value::number(round15(if digits >= 0 {
            rounded / scale
        } else {
            rounded * scale
        })))
    };
    result(run())
}

pub(super) fn round<'a>(args: &Args<'a>) -> Arg<'a> {
    round_with(args, Rounding::Nearest)
}

pub(super) fn roundup<'a>(args: &Args<'a>) -> Arg<'a> {
    round_with(args, Rounding::Up)
}

pub(super) fn rounddown<'a>(args: &Args<'a>) -> Arg<'a> {
    round_with(args, Rounding::Down)
}

fn unary(args: &Args<'_>, f: fn(f64) -> Value) -> Arg<'static> {
    result(args.value(0).to_number().map(f))
}

pub(super) fn int<'a>(args: &Args<'a>) -> Arg<'a> {
    unary(args, |n| Value::number(n.floor()))
}

pub(super) fn abs<'a>(args: &Args<'a>) -> Arg<'a> {
    unary(args, |n| Value::number(n.abs()))
}

pub(super) fn sqrt<'a>(args: &Args<'a>) -> Arg<'a> {
    unary(args, |n| {
        if n < 0.0 {
            Value::Error(ErrorKind::Num)
        } else {
            Value::number(n.sqrt())
        }
    })
}

/// Excel's MOD takes the sign of the divisor: `MOD(-3, 2)` is 1.
pub(super) fn mod_<'a>(args: &Args<'a>) -> Arg<'a> {
    let run = || -> Result<Value, ErrorKind> {
        let n = args.value(0).to_number()?;
        let d = args.value(1).to_number()?;
        if d == 0.0 {
            return Err(ErrorKind::Div0);
        }
        Ok(Value::number(n - d * (n / d).floor()))
    };
    result(run())
}

pub(super) fn power<'a>(args: &Args<'a>) -> Arg<'a> {
    value(binary(BinaryOp::Pow, &args.value(0), &args.value(1)))
}

#[cfg(test)]
mod tests {
    use super::super::test_support::check;
    use crate::formula::{ErrorKind as E, Value};

    fn n(x: f64) -> Value {
        Value::Number(x)
    }

    fn err(e: E) -> Value {
        Value::Error(e)
    }

    #[test]
    fn sum_and_average_follow_excels_rules_for_typed_and_referenced_values() {
        check(&[
            ("=SUM(1,2,3)", n(6.0)),
            ("=SUM([Nums])", n(10.0)),
            ("=SUM([Nums],10)", n(20.0)),
            // Typed text and booleans count; in cells they are skipped.
            ("=SUM(\"2\",TRUE)", n(3.0)),
            ("=SUM([Mixed])", n(6.0)),
            ("=SUM(\"abc\")", err(E::Value)),
            ("=SUM([Errs])", err(E::Div0)),
            ("=SUM([Empty])", n(0.0)),
            ("=SUM(1,)", n(1.0)),
            ("=AVERAGE([Nums])", n(2.5)),
            ("=AVERAGE([Mixed])", n(3.0)),
            ("=AVERAGE([Words])", err(E::Div0)),
            ("=AVERAGE(1,\"3\")", n(2.0)),
        ]);
    }

    #[test]
    fn min_and_max() {
        check(&[
            ("=MIN([Nums])", n(1.0)),
            ("=MAX([Nums],10)", n(10.0)),
            ("=MAX([Words])", n(0.0)),
            ("=MIN(-1,\"-5\")", n(-5.0)),
            ("=MAX([Errs])", err(E::Div0)),
        ]);
    }

    #[test]
    fn counting() {
        check(&[
            ("=COUNT([Mixed])", n(2.0)),
            ("=COUNT(1,\"2\",TRUE,\"x\")", n(3.0)),
            ("=COUNT([Errs])", n(2.0)),
            ("=COUNT([@Mixed])", n(1.0)),
            ("=COUNTA([Mixed])", n(4.0)),
            ("=COUNTA([Words])", n(4.0)),
            ("=COUNTA([Errs])", n(3.0)),
            ("=COUNTA(1,\"\",NA())", n(3.0)),
            ("=COUNTBLANK([Words])", n(2.0)),
            ("=COUNTBLANK(1)", err(E::Value)),
        ]);
    }

    #[test]
    fn rounding_matches_excel_at_the_halfway_point() {
        check(&[
            ("=ROUND(2.675,2)", n(2.68)),
            ("=ROUND(2.5,0)", n(3.0)),
            ("=ROUND(-2.5,0)", n(-3.0)),
            ("=ROUND(1234.567,-2)", n(1200.0)),
            ("=ROUND(1.005,2)", n(1.01)),
            ("=ROUND(0.1+0.2,15)", n(0.3)),
            ("=ROUNDUP(3.2,0)", n(4.0)),
            ("=ROUNDUP(-3.2,0)", n(-4.0)),
            ("=ROUNDUP(1.2341,3)", n(1.235)),
            ("=ROUNDDOWN(3.9,0)", n(3.0)),
            ("=ROUNDDOWN(-3.9,0)", n(-3.0)),
            ("=ROUNDDOWN(1234.5,-2)", n(1200.0)),
            ("=ROUND(\"x\",1)", err(E::Value)),
            ("=ROUND(5,1e300)", n(5.0)),
        ]);
    }

    #[test]
    fn arithmetic_functions() {
        check(&[
            ("=INT(8.9)", n(8.0)),
            ("=INT(-8.9)", n(-9.0)),
            ("=MOD(3,2)", n(1.0)),
            ("=MOD(-3,2)", n(1.0)),
            ("=MOD(3,-2)", n(-1.0)),
            ("=MOD(3,0)", err(E::Div0)),
            ("=MOD(5.3,1)", n(0.3)),
            ("=ABS(-2)", n(2.0)),
            ("=SQRT(16)", n(4.0)),
            ("=SQRT(-1)", err(E::Num)),
            ("=POWER(2,10)", n(1024.0)),
            ("=POWER(0,0)", err(E::Num)),
            // Empty and error arguments.
            ("=ABS()", err(E::Value)),
            ("=ABS(\"x\")", err(E::Value)),
            ("=ABS(NA())", err(E::NA)),
            ("=INT(\"\")", err(E::Value)),
            ("=INT(1/0)", err(E::Div0)),
            ("=MOD(,1)", n(0.0)),
            ("=MOD(1,)", err(E::Div0)),
            ("=SQRT(,)", err(E::Value)),
            ("=POWER(,2)", n(0.0)),
        ]);
    }
}
