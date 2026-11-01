//! Formula values and Excel's rules for converting between them.
//!
//! Numbers follow the model decided in 0009: IEEE doubles, `#NUM!` for anything Excel
//! cannot hold, no negative zero, and equality at 15 significant digits.

use std::cmp::Ordering;

use super::ast::{BinaryOp, ErrorKind, UnaryOp};

/// Excel's largest number, 9.99999999999999E+307. Larger results are `#NUM!`.
const MAX_NUMBER: f64 = 9.999_999_999_999_99e307;

/// A value produced or consumed by a formula.
///
/// `PartialEq` is structural (`Text("a") != Text("A")`); Excel's `=` is [`compare`].
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// An empty cell. Reads as 0, "", or FALSE depending on what wants it.
    Empty,
    Number(f64),
    Text(String),
    Bool(bool),
    /// An Excel date serial (days since 1899-12-31, with Excel's phantom 1900-02-29).
    /// It is a number to every operator; it only remembers that it is a date, so
    /// `TODAY()+1` stays a date while `B2-A2` becomes a count of days, and a formula
    /// column can infer that its results are dates.
    Date(f64),
    Error(ErrorKind),
}

impl Value {
    /// A number result, applying the number model: beyond Excel's range or non-finite
    /// becomes `#NUM!`; subnormals and negative zero become zero.
    #[must_use]
    pub fn number(n: f64) -> Self {
        normalize(n).map_or(Self::Error(ErrorKind::Num), Self::Number)
    }

    fn date(n: f64) -> Self {
        normalize(n).map_or(Self::Error(ErrorKind::Num), Self::Date)
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

/// `None` for what Excel cannot hold. Values below the smallest normal double flush to
/// zero, as Excel's documented minimum is 2.2251E-308.
fn normalize(n: f64) -> Option<f64> {
    if !n.is_finite() || n.abs() > MAX_NUMBER {
        return None;
    }
    Some(if n.abs() < f64::MIN_POSITIVE { 0.0 } else { n })
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

/// Excel's `^`: 0^0 is `#NUM!`, zero to a negative power is `#DIV/0!`, and anything
/// without a real result (a negative base with a fractional power) is `#NUM!`.
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
    // An empty side becomes the other side's blank value, then the general rules apply.
    let blank_like = |other: &Value| match other {
        Text(_) => Text(String::new()),
        Bool(_) => Bool(false),
        _ => Number(0.0),
    };
    let (a, b) = match (a, b) {
        (Empty, Empty) => return Ok(Ordering::Equal),
        (Empty, b) => (&blank_like(b), b),
        (a, Empty) => (a, &blank_like(a)),
        (a, b) => (a, b),
    };
    let rank = |v: &Value| match v {
        Number(_) | Date(_) | Empty | Error(_) => 0,
        Text(_) => 1,
        Bool(_) => 2,
    };
    Ok(match (a, b) {
        (Number(x) | Date(x), Number(y) | Date(y)) => compare_numbers(*x, *y),
        (Text(x), Text(y)) => compare_text(x, y),
        (Bool(x), Bool(y)) => x.cmp(y),
        _ => rank(a).cmp(&rank(b)),
    })
}

#[expect(clippy::float_cmp, reason = "exact equality is only the fast path")]
fn compare_numbers(x: f64, y: f64) -> Ordering {
    if x == y {
        return Ordering::Equal;
    }
    round15(x)
        .partial_cmp(&round15(y))
        .unwrap_or(Ordering::Equal)
}

/// Case-insensitive, with Excel's broad order of character classes: spaces and
/// punctuation, then digits, then letters. Within a class, characters compare by their
/// lowercase code point; Excel's accent-aware collation is a later refinement.
fn compare_text(x: &str, y: &str) -> Ordering {
    if x == y {
        return Ordering::Equal;
    }
    let key = |c: char| {
        let class = if c.is_alphabetic() {
            2
        } else {
            u8::from(c.is_numeric())
        };
        (class, c.to_lowercase().next().unwrap_or(c))
    };
    x.chars().map(key).cmp(y.chars().map(key))
}

/// Rounds to 15 significant digits, the precision Excel shows and compares at.
#[must_use]
pub fn round15(n: f64) -> f64 {
    if n == 0.0 {
        return 0.0;
    }
    if !n.is_finite() {
        return n;
    }
    // Scientific formatting with 14 fraction digits rounds to 15 significant digits.
    // Near f64::MAX the rounded text can parse as infinity; keep the original then.
    match format!("{n:.14e}").parse::<f64>() {
        Ok(r) if r.is_finite() => r,
        _ => n,
    }
}

/// Excel's General format, as used when a number becomes text: up to 15 significant
/// digits, trailing zeros dropped, scientific notation for magnitudes of 1E+15 and above
/// or below 1E-9 (`1E+15`, `1.5E-10`).
#[must_use]
pub fn general(n: f64) -> String {
    if !n.is_finite() {
        return ErrorKind::Num.as_str().into();
    }
    if n == 0.0 {
        return "0".into();
    }
    // The decimal exponent of the 15-digit rounding decides the layout. Taking it from
    // the formatted text, not log10, keeps 99999999999999.9 from becoming 1E+14.
    let sci = format!("{n:.14e}");
    let (mantissa, exponent) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    if !(-9..15).contains(&exponent) {
        let sign = if exponent < 0 { '-' } else { '+' };
        return format!("{}E{sign}{:02}", trim_fraction(mantissa), exponent.abs());
    }
    let decimals = usize::try_from(14 - exponent).unwrap_or(0);
    let r = round15(n);
    trim_fraction(&format!("{r:.decimals$}")).to_string()
}

fn trim_fraction(s: &str) -> &str {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.')
    } else {
        s
    }
}

/// Text that arithmetic accepts as a number, as Excel does in the en-US locale:
/// surrounding spaces, a sign or parentheses for negatives, a leading `$`, thousands
/// separators in groups of three, a decimal point, an exponent, a trailing `%`, a date
/// (`2024-01-31`, `2024-1-31`, `6/1/2001`), or a time of day (`12:00`, `08:30:15`).
#[must_use]
pub fn number_from_text(s: &str) -> Option<f64> {
    let s = s.trim_matches(' ');
    if let Some(n) = serial_from_date_text(s).or_else(|| fraction_from_time(s)) {
        return Some(n);
    }
    let (in_parens, s) = match s.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
        Some(inner) => (true, inner),
        None => (false, s),
    };
    let (sign, s) = match s.as_bytes().first() {
        // A sign inside parentheses is not a number: "(-5)" is #VALUE!.
        Some(b'-' | b'+') if in_parens => return None,
        Some(b'-') => (-1.0, &s[1..]),
        Some(b'+') => (1.0, &s[1..]),
        _ => (if in_parens { -1.0 } else { 1.0 }, s),
    };
    let s = s.strip_prefix('$').unwrap_or(s);
    let (percent, s) = match s.strip_suffix('%') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    let plain = strip_thousands(s)?;
    let starts_well = plain
        .bytes()
        .next()
        .is_some_and(|b| b.is_ascii_digit() || b == b'.');
    let only_number_chars = plain
        .bytes()
        .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-'));
    if !starts_well || !only_number_chars {
        return None;
    }
    let mut n: f64 = plain.parse().ok()?;
    if percent {
        n /= 100.0;
    }
    normalize(sign * n)
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

/// Splits `text` on `sep` into exactly `n` all-digit parts of at most `max` digits.
fn digit_parts(text: &str, sep: char, n: usize, max: usize) -> Option<Vec<i64>> {
    let parts: Vec<&str> = text.split(sep).collect();
    if parts.len() != n {
        return None;
    }
    parts
        .iter()
        .map(|p| {
            let ok = !p.is_empty() && p.len() <= max && p.bytes().all(|b| b.is_ascii_digit());
            ok.then(|| p.parse().ok()).flatten()
        })
        .collect()
}

/// `YYYY-MM-DD` (month and day may be one digit) or US `M/D/YYYY`.
fn serial_from_date_text(s: &str) -> Option<f64> {
    if let Some(p) = digit_parts(s, '-', 3, 4)
        && s.len() >= 8
        && s.find('-') == Some(4)
    {
        return serial_from_date(p[0], p[1], p[2]);
    }
    let p = digit_parts(s, '/', 3, 4)?;
    if p[2] < 1000 {
        return None;
    }
    serial_from_date(p[2], p[0], p[1])
}

/// `H:MM` or `H:MM:SS` on a 24-hour clock, as a fraction of a day.
fn fraction_from_time(s: &str) -> Option<f64> {
    let p = digit_parts(s, ':', 3, 2).or_else(|| digit_parts(s, ':', 2, 2))?;
    let (h, m, sec) = (p[0], p[1], p.get(2).copied().unwrap_or(0));
    if h > 23 || m > 59 || sec > 59 {
        return None;
    }
    #[expect(clippy::cast_precision_loss, reason = "under 86,400")]
    let seconds = (h * 3600 + m * 60 + sec) as f64;
    Some(seconds / 86_400.0)
}

/// Days from 1970-01-01 to a civil date (proleptic Gregorian). Howard Hinnant's
/// `days_from_civil`. Callers keep the year to 1900..=9999, so nothing overflows.
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
/// Excel's phantom 1900-02-29, kept for compatibility with Lotus 1-2-3.
const PHANTOM_LEAP_DAY: i64 = 60;
/// One past the last valid serial, for range checks on floats.
const SERIAL_END: f64 = 2_958_466.0;

/// The Excel serial for a calendar date, or `None` outside 1900-01-01..=9999-12-31.
/// Excel's phantom 1900-02-29 is accepted as serial 60.
#[must_use]
pub fn serial_from_date(y: i64, m: i64, d: i64) -> Option<f64> {
    if !(1900..=9999).contains(&y) || !(1..=12).contains(&m) {
        return None;
    }
    if (y, m, d) == (1900, 2, 29) {
        #[expect(clippy::cast_precision_loss, reason = "a small constant")]
        return Some(PHANTOM_LEAP_DAY as f64);
    }
    if d < 1 || d > days_in_month(y, m) {
        return None;
    }
    let days = days_from_civil(y, m, d) - days_from_civil(1899, 12, 31);
    // Excel counts the phantom day, so every later date is one higher than a true count.
    let serial = if days >= PHANTOM_LEAP_DAY {
        days + 1
    } else {
        days
    };
    #[expect(clippy::cast_precision_loss, reason = "serials stay below 2^22")]
    (1..=MAX_SERIAL).contains(&serial).then_some(serial as f64)
}

/// The calendar date of an Excel serial's whole part, or `None` outside Excel's date
/// range. Serial 60 is Excel's phantom 1900-02-29.
#[must_use]
pub fn date_from_serial(serial: f64) -> Option<(i64, i64, i64)> {
    if !serial.is_finite() || !(1.0..SERIAL_END).contains(&serial) {
        return None;
    }
    #[expect(clippy::cast_possible_truncation, reason = "range-checked above")]
    let n = serial.floor() as i64;
    if n == PHANTOM_LEAP_DAY {
        return Some((1900, 2, 29));
    }
    let days = if n > PHANTOM_LEAP_DAY { n - 1 } else { n };
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
    serial_from_date_text(s)
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
                "unverified in Excel; recorded by 0024",
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
            binary(B::Add, &d, &Number(1e308)),
            Error(E::Num),
            "beyond Excel's range"
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
        assert_eq!(
            iso_from_serial(60.0).as_deref(),
            Some("1900-02-29"),
            "the phantom leap day"
        );
        assert_eq!(serial_from_iso("1900-02-29"), Some(60.0));
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
    fn general_keeps_fifteen_digits_at_every_magnitude() {
        let cases = [
            (99_999_999_999_999.9, "99999999999999.9"),
            (99_999.999_999_999_9, "99999.9999999999"),
            (9_999_999.999_999_99, "9999999.99999999"),
            (9.999_999_999_999_99e-9, "0.00000000999999999999999"),
            (1.5e-10, "1.5E-10"),
            (0.1, "0.1"),
            (1.0 / 3.0, "0.333333333333333"),
            (1e14, "100000000000000"),
            (-123.45, "-123.45"),
            (1e-9, "0.000000001"),
            (9.999_999_999_999_99e307, "9.99999999999999E+307"),
        ];
        for (n, want) in cases {
            assert_eq!(general(n), want, "{n:e}");
        }
        assert_eq!(general(f64::NAN), "#NUM!");
    }

    #[test]
    fn dates_and_times_in_text_become_numbers() {
        assert_eq!(number_from_text("6/1/2001"), serial_from_iso("2001-06-01"));
        assert_eq!(
            binary(B::Sub, &text("6/1/2001"), &text("5/1/2001")),
            Number(31.0),
            "Microsoft's operators documentation example"
        );
        assert_eq!(number_from_text("2024-1-31"), Some(45322.0));
        assert_eq!(number_from_text("12:00"), Some(0.5));
        assert_eq!(number_from_text("06:00:00"), Some(0.25));
        for bad in [
            "24:00",
            "12:60",
            "1/2",
            "13/1/2024",
            "2024-02-30",
            "(-5)",
            "(+5)",
            "$-5x",
        ] {
            assert_eq!(number_from_text(bad), None, "{bad}");
        }
    }

    #[test]
    fn edge_numbers() {
        assert_eq!(
            Value::number(f64::MAX),
            Error(E::Num),
            "beyond 9.99999999999999E+307"
        );
        assert_eq!(
            Value::number(1e-310),
            Number(0.0),
            "subnormals flush to zero"
        );
        assert_eq!(compare(&Number(-0.0), &Number(0.0)), Ok(Ordering::Equal));
        assert_eq!(binary(B::Eq, &Number(-0.0), &Empty), Bool(true));
        for (y, m, d) in [
            (i64::MAX, 1, 1),
            (i64::MIN, 2, 1),
            (1899, 12, 31),
            (10_000, 1, 1),
        ] {
            assert_eq!(serial_from_date(y, m, d), None, "{y}-{m}-{d}");
        }
        for s in [f64::NAN, f64::INFINITY, -1.0, 0.5, 3e6] {
            assert_eq!(date_from_serial(s), None, "{s}");
        }
    }

    #[test]
    fn punctuation_sorts_before_digits_before_letters() {
        assert_eq!(compare(&text("~"), &text("a")), Ok(Ordering::Less));
        assert_eq!(compare(&text("9"), &text("a")), Ok(Ordering::Less));
        assert_eq!(compare(&text("_x"), &text("1")), Ok(Ordering::Less));
        assert_eq!(compare(&text("Apple"), &text("apple")), Ok(Ordering::Equal));
    }

    #[test]
    fn number_constructor_applies_the_number_model() {
        assert_eq!(Value::number(f64::NAN), Error(E::Num));
        assert_eq!(Value::number(f64::INFINITY), Error(E::Num));
        assert!(matches!(Value::number(-0.0), Number(z) if z.is_sign_positive()));
        assert_eq!(round15(0.1 + 0.2), 0.3);
    }
}
