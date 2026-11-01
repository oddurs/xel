//! Formula values and Excel's rules for converting between them.
//!
//! Numbers follow the model decided in 0009: IEEE doubles, `#NUM!` for anything
//! non-finite, no negative zero, and equality at 15 significant digits.

use std::cmp::Ordering;

use super::ast::{BinaryOp, ErrorKind, UnaryOp};

/// A value produced or consumed by a formula.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// An empty cell. Reads as 0, "", or FALSE depending on what wants it.
    Empty,
    Number(f64),
    Text(String),
    Bool(bool),
    /// An Excel date serial (days since 1899-12-31, with Excel's phantom 1900-02-29).
    /// It is a number to every operator; it only remembers that it is a date, so
    /// `TODAY()+1` stays a date while `B2-A2` becomes a count of days.
    Date(f64),
    Error(ErrorKind),
}

impl Value {
    /// A number result, applying the number model: non-finite becomes `#NUM!` and
    /// negative zero becomes zero.
    #[must_use]
    pub fn number(n: f64) -> Self {
        Self::finite(n).map_or(Self::Error(ErrorKind::Num), Self::Number)
    }

    fn date(n: f64) -> Self {
        Self::finite(n).map_or(Self::Error(ErrorKind::Num), Self::Date)
    }

    fn finite(n: f64) -> Option<f64> {
        n.is_finite().then_some(if n == 0.0 { 0.0 } else { n })
    }

    /// The value as a number, as arithmetic sees it.
    pub fn to_number(&self) -> Result<f64, ErrorKind> {
        match self {
            Self::Empty => Ok(0.0),
            Self::Number(n) | Self::Date(n) => Ok(*n),
            Self::Bool(b) => Ok(f64::from(u8::from(*b))),
            Self::Text(s) => number_from_text(s).ok_or(ErrorKind::Value),
            Self::Error(e) => Err(*e),
        }
    }

    /// The value as text, as `&` sees it.
    pub fn to_text(&self) -> Result<String, ErrorKind> {
        match self {
            Self::Empty => Ok(String::new()),
            Self::Number(n) | Self::Date(n) => Ok(general(*n)),
            Self::Bool(true) => Ok("TRUE".into()),
            Self::Bool(false) => Ok("FALSE".into()),
            Self::Text(s) => Ok(s.clone()),
            Self::Error(e) => Err(*e),
        }
    }
}

/// Applies a unary operator.
#[must_use]
pub fn unary(op: UnaryOp, v: &Value) -> Value {
    match op {
        // Excel's unary plus returns its operand untouched: =+"a" is "a".
        UnaryOp::Plus => v.clone(),
        UnaryOp::Minus => v
            .to_number()
            .map_or_else(Value::Error, |n| Value::number(-n)),
        UnaryOp::Percent => v
            .to_number()
            .map_or_else(Value::Error, |n| Value::number(n / 100.0)),
    }
}

/// Applies a binary operator. The left operand is converted first, so its error wins:
/// `="a"+NA()` is `#VALUE!` and `=NA()+"a"` is `#N/A`, as in Excel.
#[must_use]
pub fn binary(op: BinaryOp, a: &Value, b: &Value) -> Value {
    match op {
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Pow => {
            let (x, y) = match (a.to_number(), b.to_number()) {
                (Err(e), _) | (Ok(_), Err(e)) => return Value::Error(e),
                (Ok(x), Ok(y)) => (x, y),
            };
            let a_date = matches!(a, Value::Date(_));
            let b_date = matches!(b, Value::Date(_));
            match op {
                // A date moved by days is still a date; the distance between dates is not.
                BinaryOp::Add if a_date != b_date => Value::date(x + y),
                BinaryOp::Sub if a_date && !b_date => Value::date(x - y),
                BinaryOp::Add => Value::number(x + y),
                BinaryOp::Sub => Value::number(x - y),
                BinaryOp::Mul => Value::number(x * y),
                BinaryOp::Div if y == 0.0 => Value::Error(ErrorKind::Div0),
                BinaryOp::Div => Value::number(x / y),
                BinaryOp::Pow => power(x, y),
                _ => unreachable!("arithmetic operators only"),
            }
        }
        BinaryOp::Concat => match (a.to_text(), b.to_text()) {
            (Err(e), _) | (Ok(_), Err(e)) => Value::Error(e),
            (Ok(x), Ok(y)) => Value::Text(x + &y),
        },
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
            match compare(a, b) {
                Err(e) => Value::Error(e),
                Ok(ord) => Value::Bool(match op {
                    BinaryOp::Eq => ord == Ordering::Equal,
                    BinaryOp::Ne => ord != Ordering::Equal,
                    BinaryOp::Lt => ord == Ordering::Less,
                    BinaryOp::Le => ord != Ordering::Greater,
                    BinaryOp::Gt => ord == Ordering::Greater,
                    BinaryOp::Ge => ord != Ordering::Less,
                    _ => unreachable!("comparison operators only"),
                }),
            }
        }
    }
}

/// Excel's `^`: 0^0 and a negative base with a fractional exponent are `#NUM!`; zero to
/// a negative power is `#DIV/0!`.
fn power(x: f64, y: f64) -> Value {
    if x == 0.0 && y == 0.0 {
        return Value::Error(ErrorKind::Num);
    }
    if x == 0.0 && y < 0.0 {
        return Value::Error(ErrorKind::Div0);
    }
    Value::number(x.powf(y))
}

/// Excel's comparison: numbers sort before text, text before booleans; text compares
/// without regard to case; numbers are equal when they agree to 15 significant digits.
/// An empty cell takes the type of the other side (0, "", or FALSE).
pub fn compare(a: &Value, b: &Value) -> Result<Ordering, ErrorKind> {
    use Value::{Bool, Date, Empty, Error, Number, Text};
    if let Error(e) = a {
        return Err(*e);
    }
    if let Error(e) = b {
        return Err(*e);
    }
    let rank = |v: &Value| match v {
        Number(_) | Date(_) => 0,
        Text(_) => 1,
        Bool(_) => 2,
        Empty | Error(_) => unreachable!("handled before ranking"),
    };
    Ok(match (a, b) {
        (Empty, Empty) => Ordering::Equal,
        (Empty, Number(_) | Date(_)) => compare_numbers(0.0, b.to_number()?),
        (Number(_) | Date(_), Empty) => compare_numbers(a.to_number()?, 0.0),
        (Empty, Text(t)) => compare_text("", t),
        (Text(t), Empty) => compare_text(t, ""),
        (Empty, Bool(x)) => false.cmp(x),
        (Bool(x), Empty) => x.cmp(&false),
        (Number(_) | Date(_), Number(_) | Date(_)) => {
            compare_numbers(a.to_number()?, b.to_number()?)
        }
        (Text(x), Text(y)) => compare_text(x, y),
        (Bool(x), Bool(y)) => x.cmp(y),
        _ => rank(a).cmp(&rank(b)),
    })
}

fn compare_numbers(x: f64, y: f64) -> Ordering {
    round15(x).total_cmp(&round15(y))
}

fn compare_text(x: &str, y: &str) -> Ordering {
    x.to_lowercase().cmp(&y.to_lowercase())
}

/// Rounds to 15 significant digits, the precision Excel shows and compares at.
#[must_use]
pub fn round15(n: f64) -> f64 {
    if n == 0.0 || !n.is_finite() {
        return n;
    }
    // Scientific formatting with 14 fraction digits rounds to 15 significant digits,
    // and Rust always parses what it formats.
    format!("{n:.14e}").parse().unwrap_or(n)
}

/// Excel's General format, as used when a number becomes text: up to 15 significant
/// digits, trailing zeros dropped, scientific notation for very large or very small
/// magnitudes (`1E+15`, `1.5E-10`).
#[must_use]
pub fn general(n: f64) -> String {
    let r = round15(n);
    if r == 0.0 {
        return "0".into();
    }
    let magnitude = r.abs();
    if !(1e-9..1e15).contains(&magnitude) {
        // `{:E}` always yields "<mantissa>E<exponent>".
        let s = format!("{r:.14E}");
        let (mantissa, exponent) = s.split_once('E').unwrap_or((&s, "0"));
        let mantissa = trim_fraction(mantissa);
        let exponent: i32 = exponent.parse().unwrap_or(0);
        let sign = if exponent < 0 { '-' } else { '+' };
        return format!("{mantissa}E{sign}{:02}", exponent.abs());
    }
    // Digits after the point that keep 15 significant digits in total. The magnitude
    // range above bounds this to 0..=23.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "log10 of a value in 1e-9..1e15"
    )]
    let int_digits = magnitude.log10().floor() as i32 + 1;
    let decimals = usize::try_from(15 - int_digits).unwrap_or(0);
    trim_fraction(&format!("{r:.decimals$}")).to_string()
}

fn trim_fraction(s: &str) -> &str {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.')
    } else {
        s
    }
}

/// Text that arithmetic accepts as a number, as Excel does: surrounding spaces, a sign
/// or parentheses for negatives, a leading `$`, thousands separators in groups of three,
/// a decimal point, an exponent, a trailing `%`, or an ISO date (`2024-01-31`).
#[must_use]
pub fn number_from_text(s: &str) -> Option<f64> {
    let s = s.trim_matches(' ');
    if let Some(serial) = serial_from_iso(s) {
        return Some(serial);
    }
    let (negative, s) = match s.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
        Some(inner) => (true, inner),
        None => (false, s),
    };
    let (sign, s) = match s.as_bytes().first() {
        Some(b'-') => (-1.0, &s[1..]),
        Some(b'+') => (1.0, &s[1..]),
        _ => (1.0, s),
    };
    let s = s.strip_prefix('$').unwrap_or(s);
    let (percent, s) = match s.strip_suffix('%') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    let plain = strip_thousands(s)?;
    if plain.is_empty()
        || !plain
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-'))
        || !plain
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_digit() || b == b'.')
    {
        return None;
    }
    let mut n: f64 = plain.parse().ok()?;
    if percent {
        n /= 100.0;
    }
    if negative {
        n = -n;
    }
    Value::finite(sign * n)
}

/// Removes `,` separators if they group the integer part in threes; otherwise refuses.
fn strip_thousands(s: &str) -> Option<String> {
    if !s.contains(',') {
        return Some(s.to_string());
    }
    let int_end = s
        .find(|c: char| !(c.is_ascii_digit() || c == ','))
        .unwrap_or(s.len());
    let (int, rest) = s.split_at(int_end);
    let groups: Vec<&str> = int.split(',').collect();
    let first_ok = (1..=3).contains(&groups[0].len());
    if !first_ok || groups[1..].iter().any(|g| g.len() != 3) || rest.contains(',') {
        return None;
    }
    Some(groups.concat() + rest)
}

/// Days from 1970-01-01 to a civil date (proleptic Gregorian). Howard Hinnant's
/// `days_from_civil`.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (
        if m <= 2 {
            yoe + era * 400 + 1
        } else {
            yoe + era * 400
        },
        m,
        d,
    )
}

/// Excel's largest date, 9999-12-31.
const MAX_SERIAL: i64 = 2_958_465;

/// The Excel serial for a calendar date, or `None` outside 1900-01-01..=9999-12-31.
#[must_use]
pub fn serial_from_date(y: i64, m: i64, d: i64) -> Option<f64> {
    if !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) {
        return None;
    }
    let days = days_from_civil(y, m, d) - days_from_civil(1899, 12, 31);
    // Excel counts a 1900-02-29 that never existed (serial 60), so later dates are one
    // higher than a true day count.
    let serial = if days >= 60 { days + 1 } else { days };
    #[expect(clippy::cast_precision_loss, reason = "serials stay below 2^22")]
    (1..=MAX_SERIAL).contains(&serial).then_some(serial as f64)
}

/// The calendar date of an Excel serial's whole part, or `None` for serial 60 (the
/// phantom 1900-02-29) and anything outside Excel's date range.
#[must_use]
pub fn date_from_serial(serial: f64) -> Option<(i64, i64, i64)> {
    if !serial.is_finite() {
        return None;
    }
    #[expect(clippy::cast_possible_truncation, reason = "range-checked below")]
    let n = serial.floor() as i64;
    if !(1..=MAX_SERIAL).contains(&n) || n == 60 {
        return None;
    }
    let days = if n > 60 { n - 1 } else { n };
    Some(civil_from_days(days_from_civil(1899, 12, 31) + days))
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// `YYYY-MM-DD`, the form date columns store, as an Excel serial.
#[must_use]
pub fn serial_from_iso(s: &str) -> Option<f64> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let part = &s[r];
        part.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| part.parse().ok())
            .flatten()
    };
    serial_from_date(num(0..4)?, num(5..7)?, num(8..10)?)
}

/// An Excel serial as `YYYY-MM-DD`.
#[must_use]
pub fn iso_from_serial(serial: f64) -> Option<String> {
    let (y, m, d) = date_from_serial(serial)?;
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

#[cfg(test)]
#[allow(
    clippy::too_many_lines,
    clippy::float_cmp,
    reason = "vector tables are long by design, and exact float results are what they assert"
)]
mod tests {
    use super::*;
    use BinaryOp as B;
    use ErrorKind as E;
    use Value::{Bool, Date, Empty, Error, Number, Text};

    fn text(s: &str) -> Value {
        Text(s.into())
    }

    /// Each vector: operator, left, right, expected, and where the expectation comes
    /// from. "docs" is Microsoft's "Calculation operators and precedence in Excel" and
    /// "How to correct a #VALUE! error"; "0009" is this project's number-model decision.
    /// The conformance harness (0024) adds vectors recorded in Excel itself.
    #[test]
    fn arithmetic_coercion() {
        let cases: Vec<(B, Value, Value, Value, &str)> = vec![
            (
                B::Add,
                text("1"),
                Number(1.0),
                Number(2.0),
                "docs: text that looks like a number is converted",
            ),
            (
                B::Add,
                Bool(true),
                Number(1.0),
                Number(2.0),
                "docs: TRUE is 1",
            ),
            (
                B::Add,
                Bool(false),
                Bool(true),
                Number(1.0),
                "docs: FALSE is 0",
            ),
            (
                B::Add,
                Empty,
                Number(5.0),
                Number(5.0),
                "docs: an empty cell is 0",
            ),
            (
                B::Add,
                text("abc"),
                Number(1.0),
                Error(E::Value),
                "docs: #VALUE! for non-numeric text",
            ),
            (
                B::Add,
                text(""),
                Number(1.0),
                Error(E::Value),
                "docs: empty text is not a number",
            ),
            (
                B::Add,
                text(" 2 "),
                Number(1.0),
                Number(3.0),
                "docs: surrounding spaces are ignored",
            ),
            (
                B::Add,
                text("1,000"),
                Number(1.0),
                Number(1001.0),
                "docs: thousands separators",
            ),
            (
                B::Add,
                text("1,00"),
                Number(1.0),
                Error(E::Value),
                "groups of three only",
            ),
            (
                B::Add,
                text("$5"),
                Number(1.0),
                Number(6.0),
                "docs: currency symbol",
            ),
            (
                B::Add,
                text("50%"),
                Number(0.0),
                Number(0.5),
                "docs: percent",
            ),
            (
                B::Add,
                text("(5)"),
                Number(0.0),
                Number(-5.0),
                "docs: accounting negative",
            ),
            (
                B::Add,
                text("1e3"),
                Number(0.0),
                Number(1000.0),
                "docs: scientific notation",
            ),
            (
                B::Add,
                text("2024-01-31"),
                Number(0.0),
                Number(45322.0),
                "docs: dates in text become serials",
            ),
            (
                B::Sub,
                Number(0.3),
                Number(0.1),
                Number(0.3 - 0.1),
                "0009: plain doubles",
            ),
            (
                B::Div,
                Number(1.0),
                Number(0.0),
                Error(E::Div0),
                "docs: #DIV/0!",
            ),
            (
                B::Div,
                Number(1.0),
                Empty,
                Error(E::Div0),
                "docs: empty divisor is 0",
            ),
            (
                B::Pow,
                Number(0.0),
                Number(0.0),
                Error(E::Num),
                "docs: 0^0 is #NUM!",
            ),
            (
                B::Pow,
                Number(0.0),
                Number(-1.0),
                Error(E::Div0),
                "docs: 0 to a negative power",
            ),
            (
                B::Pow,
                Number(-8.0),
                Number(1.0 / 3.0),
                Error(E::Num),
                "docs: negative base, fractional power",
            ),
            (B::Pow, Number(-2.0), Number(2.0), Number(4.0), "docs"),
            (
                B::Mul,
                Number(1e308),
                Number(10.0),
                Error(E::Num),
                "0009: overflow is #NUM!",
            ),
            (
                B::Mul,
                Number(-1.0),
                Number(0.0),
                Number(0.0),
                "0009: no negative zero",
            ),
            (
                B::Add,
                text("a"),
                Error(E::NA),
                Error(E::Value),
                "docs: left operand converted first",
            ),
            (
                B::Add,
                Error(E::NA),
                text("a"),
                Error(E::NA),
                "docs: left error wins",
            ),
            (
                B::Add,
                Error(E::Ref),
                Error(E::NA),
                Error(E::Ref),
                "docs: left error wins",
            ),
        ];
        for (op, a, b, want, source) in cases {
            assert_eq!(binary(op, &a, &b), want, "{a:?} {op:?} {b:?} ({source})");
        }
        assert!(
            matches!(binary(B::Mul, &Number(-1.0), &Number(0.0)), Number(z) if z.is_sign_positive())
        );
    }

    #[test]
    fn dates_stay_dates_when_moved_and_become_days_when_subtracted() {
        let d = Date(45322.0);
        assert_eq!(binary(B::Add, &d, &Number(1.0)), Date(45323.0));
        assert_eq!(binary(B::Add, &Number(1.0), &d), Date(45323.0));
        assert_eq!(binary(B::Sub, &d, &Number(1.0)), Date(45321.0));
        assert_eq!(binary(B::Sub, &d, &Date(45300.0)), Number(22.0));
        assert_eq!(binary(B::Add, &d, &d), Number(90644.0));
        assert_eq!(binary(B::Mul, &d, &Number(1.0)), Number(45322.0));
        assert_eq!(
            binary(B::Add, &d, &Number(f64::MAX)),
            Date(f64::MAX),
            "still finite"
        );
        assert_eq!(
            binary(B::Add, &Date(f64::MAX), &Number(f64::MAX)),
            Error(E::Num)
        );
    }

    #[test]
    fn concatenation() {
        let cases = [
            (Number(1.5), text("x"), "1.5x"),
            (Bool(true), Empty, "TRUE"),
            (Number(1.0 / 3.0), Empty, "0.333333333333333"),
            (Number(0.1 + 0.2), Empty, "0.3"),
            (Number(1e15), Empty, "1E+15"),
            (
                Number(123_456_789_012_345_680.0),
                Empty,
                "1.23456789012346E+17",
            ),
            (Number(-0.000_000_000_15), Empty, "-1.5E-10"),
            (Number(0.000_1), Empty, "0.0001"),
            (Date(45322.0), Empty, "45322"),
        ];
        for (a, b, want) in cases {
            assert_eq!(binary(B::Concat, &a, &b), text(want), "{a:?} & {b:?}");
        }
        assert_eq!(binary(B::Concat, &text("a"), &Error(E::NA)), Error(E::NA));
    }

    /// Source for every row: Microsoft's "Calculation operators and precedence in Excel"
    /// (numbers < text < logical values; text comparison ignores case) and 0009 (equality
    /// at 15 significant digits).
    #[test]
    fn comparison_across_every_pair_of_types() {
        use Ordering::{Equal, Greater, Less};
        let cases = [
            (Number(1.0), Number(2.0), Less),
            (Number(0.1 + 0.2), Number(0.3), Equal),
            (Number(1.0), text("1"), Less),
            (Number(1e9), text("a"), Less),
            (text("z"), Bool(false), Less),
            (Number(1e9), Bool(false), Less),
            (text("abc"), text("ABC"), Equal),
            (text("a"), text("b"), Less),
            (Bool(false), Bool(true), Less),
            (Empty, Number(0.0), Equal),
            (Empty, Number(-1.0), Greater),
            (Empty, text(""), Equal),
            (Empty, text("a"), Less),
            (Empty, Bool(false), Equal),
            (Empty, Bool(true), Less),
            (Empty, Empty, Equal),
            (Date(45322.0), Number(45322.0), Equal),
            (Date(1.0), text("a"), Less),
        ];
        for (a, b, want) in cases {
            assert_eq!(compare(&a, &b), Ok(want), "{a:?} vs {b:?}");
            assert_eq!(compare(&b, &a), Ok(want.reverse()), "{b:?} vs {a:?}");
        }
        assert_eq!(compare(&Error(E::NA), &Error(E::Ref)), Err(E::NA));
        assert_eq!(compare(&Number(1.0), &Error(E::Ref)), Err(E::Ref));
        assert_eq!(binary(B::Eq, &Number(0.1 + 0.2), &Number(0.3)), Bool(true));
        assert_eq!(binary(B::Ne, &text("A"), &text("a")), Bool(false));
        assert_eq!(binary(B::Ge, &Bool(false), &Number(1e300)), Bool(true));
    }

    #[test]
    fn unary_operators() {
        assert_eq!(
            unary(UnaryOp::Plus, &text("a")),
            text("a"),
            "Excel's + is a no-op"
        );
        assert_eq!(unary(UnaryOp::Minus, &text("2")), Number(-2.0));
        assert_eq!(unary(UnaryOp::Minus, &Number(0.0)), Number(0.0));
        assert_eq!(unary(UnaryOp::Minus, &text("x")), Error(E::Value));
        assert_eq!(unary(UnaryOp::Percent, &Number(50.0)), Number(0.5));
        assert_eq!(unary(UnaryOp::Percent, &Error(E::NA)), Error(E::NA));
    }

    #[test]
    fn errors_propagate_through_every_operator() {
        for op in [
            B::Add,
            B::Sub,
            B::Mul,
            B::Div,
            B::Pow,
            B::Concat,
            B::Eq,
            B::Lt,
        ] {
            assert_eq!(
                binary(op, &Error(E::Ref), &Number(1.0)),
                Error(E::Ref),
                "{op:?}"
            );
            assert_eq!(
                binary(op, &Number(1.0), &Error(E::Ref)),
                Error(E::Ref),
                "{op:?}"
            );
        }
        for op in [UnaryOp::Minus, UnaryOp::Percent, UnaryOp::Plus] {
            assert_eq!(unary(op, &Error(E::Num)), Error(E::Num), "{op:?}");
        }
    }

    #[test]
    fn excel_date_serials() {
        // Known Excel serials: 1 is 1900-01-01, 59 is 1900-02-28, 60 is the phantom
        // 1900-02-29, 61 is 1900-03-01, 45292 is 2024-01-01, 2958465 is 9999-12-31.
        let known = [
            ("1900-01-01", 1.0),
            ("1900-02-28", 59.0),
            ("1900-03-01", 61.0),
            ("2024-01-01", 45292.0),
            ("2024-02-29", 45351.0),
            ("9999-12-31", 2_958_465.0),
        ];
        for (iso, serial) in known {
            assert_eq!(serial_from_iso(iso), Some(serial), "{iso}");
            assert_eq!(iso_from_serial(serial).as_deref(), Some(iso), "{serial}");
        }
        assert_eq!(iso_from_serial(60.0), None, "the phantom 1900-02-29");
        assert_eq!(
            iso_from_serial(45292.75).as_deref(),
            Some("2024-01-01"),
            "time is dropped"
        );
        for bad in [
            "1899-12-31",
            "2023-02-29",
            "2024-13-01",
            "2024-1-01",
            "10000-01-01",
            "abcd-ef-gh",
        ] {
            assert_eq!(serial_from_iso(bad), None, "{bad}");
        }
        // Every day of a leap year round-trips.
        for serial in 45292..=45657 {
            let s = f64::from(serial);
            assert_eq!(serial_from_iso(&iso_from_serial(s).unwrap()), Some(s));
        }
    }

    #[test]
    fn number_constructor_applies_the_number_model() {
        assert_eq!(Value::number(f64::NAN), Error(E::Num));
        assert_eq!(Value::number(f64::INFINITY), Error(E::Num));
        assert!(matches!(Value::number(-0.0), Number(z) if z.is_sign_positive()));
        assert_eq!(round15(0.1 + 0.2), 0.3);
    }
}
