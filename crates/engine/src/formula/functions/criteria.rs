//! Functions that select cells by a criterion: SUMIF, COUNTIF, AVERAGEIF.
//!
//! A criterion is a value or text such as `">=100"`, `"<>done"`, `"a*"`, `""`. Text
//! criteria may use `*` and `?` wildcards, with `~` escaping them, and ignore case.

use std::cmp::Ordering;

use super::result;
use crate::formula::ast::ErrorKind;
use crate::formula::eval::{Arg, Args, Range};
use crate::formula::value::{Value, compare, number_from_text};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq)]
enum Target {
    Number(f64),
    Bool(bool),
    /// Lower-cased text, possibly with wildcards.
    Text(String),
    /// `""`, `"="`: blank cells. With `<>`: anything not blank.
    Blank,
}

#[derive(Debug, Clone, PartialEq)]
struct Criterion {
    op: Op,
    target: Target,
}

impl Criterion {
    fn parse(v: &Value) -> Result<Self, ErrorKind> {
        let text = match v {
            Value::Number(n) | Value::Date(n) => {
                return Ok(Self {
                    op: Op::Eq,
                    target: Target::Number(*n),
                });
            }
            Value::Bool(b) => {
                return Ok(Self {
                    op: Op::Eq,
                    target: Target::Bool(*b),
                });
            }
            Value::Empty => {
                return Ok(Self {
                    op: Op::Eq,
                    target: Target::Blank,
                });
            }
            Value::Error(e) => return Err(*e),
            Value::Text(s) => s.as_str(),
        };
        let ops = [
            ("<=", Op::Le),
            (">=", Op::Ge),
            ("<>", Op::Ne),
            ("<", Op::Lt),
            (">", Op::Gt),
            ("=", Op::Eq),
        ];
        let (op, rest) = ops
            .iter()
            .find_map(|(prefix, op)| text.strip_prefix(prefix).map(|rest| (*op, rest)))
            .unwrap_or((Op::Eq, text));
        let target = if rest.is_empty() {
            Target::Blank
        } else if let Some(n) = number_from_text(rest) {
            Target::Number(n)
        } else if rest.eq_ignore_ascii_case("TRUE") {
            Target::Bool(true)
        } else if rest.eq_ignore_ascii_case("FALSE") {
            Target::Bool(false)
        } else {
            Target::Text(rest.to_lowercase())
        };
        Ok(Self { op, target })
    }

    fn matches(&self, cell: &Value) -> bool {
        let blank = matches!(cell, Value::Empty) || matches!(cell, Value::Text(s) if s.is_empty());
        let ord = |o: Ordering| match self.op {
            Op::Eq => o == Ordering::Equal,
            Op::Ne => o != Ordering::Equal,
            Op::Lt => o == Ordering::Less,
            Op::Le => o != Ordering::Greater,
            Op::Gt => o == Ordering::Greater,
            Op::Ge => o != Ordering::Less,
        };
        match &self.target {
            Target::Blank => match self.op {
                Op::Ne => !blank,
                _ => blank,
            },
            Target::Number(n) => {
                // Numbers, and text that reads as a number, compare numerically; for
                // anything else only "not equal" holds.
                let cell_number = match cell {
                    Value::Number(x) | Value::Date(x) => Some(*x),
                    Value::Text(s) => number_from_text(s),
                    _ => None,
                };
                match cell_number {
                    Some(x) => compare(&Value::Number(x), &Value::Number(*n)).is_ok_and(ord),
                    None => self.op == Op::Ne,
                }
            }
            Target::Bool(b) => match cell {
                Value::Bool(x) => ord(x.cmp(b)),
                _ => self.op == Op::Ne,
            },
            Target::Text(pattern) => match cell {
                Value::Text(s) => match self.op {
                    Op::Eq => wildcard(pattern, &s.to_lowercase()),
                    Op::Ne => !wildcard(pattern, &s.to_lowercase()),
                    _ => ord(s.to_lowercase().as_str().cmp(pattern.as_str())),
                },
                _ => self.op == Op::Ne,
            },
        }
    }
}

/// Excel's wildcard match: `*` any run, `?` one character, `~` makes the next literal.
fn wildcard(pattern: &str, text: &str) -> bool {
    #[derive(Clone, Copy, PartialEq)]
    enum Tok {
        Any,
        One,
        Char(char),
    }
    let mut toks = Vec::new();
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        toks.push(match c {
            '*' => Tok::Any,
            '?' => Tok::One,
            '~' => chars.next().map_or(Tok::Char('~'), Tok::Char),
            c => Tok::Char(c),
        });
    }
    let text: Vec<char> = text.chars().collect();
    // Iterative matching with backtracking to the last `*`, linear in practice.
    let (mut t, mut p) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        match toks.get(p) {
            Some(Tok::Any) => {
                star = Some((p, t));
                p += 1;
            }
            Some(Tok::One) => {
                t += 1;
                p += 1;
            }
            Some(Tok::Char(c)) if *c == text[t] => {
                t += 1;
                p += 1;
            }
            _ => match star {
                Some((sp, st)) => {
                    p = sp + 1;
                    t = st + 1;
                    star = Some((sp, st + 1));
                }
                None => return false,
            },
        }
    }
    toks[p..].iter().all(|tok| *tok == Tok::Any)
}

/// The cells a criterion is tested against. Excel only accepts a reference here.
fn cells(arg: Arg<'_>) -> Result<Range<'_>, ErrorKind> {
    match arg {
        Arg::Range(r) => Ok(r),
        Arg::Ref(v) => Ok(Range::new(1, 1, vec![v].into()).expect("one value, 1×1")),
        Arg::Value(Value::Error(e)) => Err(e),
        Arg::Value(_) => Err(ErrorKind::Value),
    }
}

/// The values of `target` (or of `range` itself) at the positions where `range` matches,
/// keeping only numbers, as SUMIF and AVERAGEIF do. Errors in matched cells propagate.
fn matched_numbers(args: &Args<'_>) -> Result<Vec<f64>, ErrorKind> {
    let range = cells(args.arg(0))?;
    let criterion = Criterion::parse(&args.value(1))?;
    let target = if args.len() > 2 && !args.is_missing(2) {
        cells(args.arg(2))?
    } else {
        range.clone()
    };
    let mut out = Vec::new();
    for row in 0..range.rows() {
        for col in 0..range.cols() {
            let hit = range.get(row, col).is_some_and(|v| criterion.matches(v));
            // Excel reads the sum range from its top-left corner in the criteria range's
            // shape; cells beyond it are blank.
            match (hit, target.get(row, col)) {
                (true, Some(Value::Number(n) | Value::Date(n))) => out.push(*n),
                (true, Some(Value::Error(e))) => return Err(*e),
                _ => {}
            }
        }
    }
    Ok(out)
}

pub(super) fn sumif<'a>(args: &Args<'a>) -> Arg<'a> {
    result(matched_numbers(args).map(|ns| Value::number(ns.iter().sum())))
}

pub(super) fn averageif<'a>(args: &Args<'a>) -> Arg<'a> {
    result(matched_numbers(args).and_then(|ns| {
        if ns.is_empty() {
            return Err(ErrorKind::Div0);
        }
        #[expect(clippy::cast_precision_loss, reason = "counts stay far below 2^52")]
        Ok(Value::number(ns.iter().sum::<f64>() / ns.len() as f64))
    }))
}

pub(super) fn countif<'a>(args: &Args<'a>) -> Arg<'a> {
    let run = || -> Result<Value, ErrorKind> {
        let range = cells(args.arg(0))?;
        let criterion = Criterion::parse(&args.value(1))?;
        let n = range
            .values()
            .iter()
            .filter(|v| criterion.matches(v))
            .count();
        #[expect(clippy::cast_precision_loss, reason = "counts stay far below 2^52")]
        Ok(Value::Number(n as f64))
    };
    result(run())
}

#[cfg(test)]
mod tests {
    use super::super::test_support::check;
    use super::wildcard;
    use crate::formula::{ErrorKind as E, Value};

    fn n(x: f64) -> Value {
        Value::Number(x)
    }

    #[test]
    fn wildcards() {
        for (pattern, text, want) in [
            ("a*", "apple", true),
            ("*e", "apple", true),
            ("a?ple", "apple", true),
            ("a?le", "apple", false),
            ("*", "", true),
            ("~*", "*", true),
            ("~*", "a", false),
            ("a~?", "a?", true),
            ("*an*", "banana", true),
            ("b*n*a", "banana", true),
            ("", "", true),
            ("", "a", false),
        ] {
            assert_eq!(wildcard(pattern, text), want, "{pattern} vs {text}");
        }
    }

    #[test]
    fn countif_criteria() {
        check(&[
            ("=COUNTIF([Nums],\">2\")", n(2.0)),
            ("=COUNTIF([Nums],\"<=2\")", n(2.0)),
            ("=COUNTIF([Nums],2)", n(1.0)),
            ("=COUNTIF([Nums],\"<>2\")", n(3.0)),
            ("=COUNTIF([Mixed],2)", n(1.0)),
            ("=COUNTIF([Words],\"a*\")", n(1.0)),
            ("=COUNTIF([Words],\"BANANA\")", n(1.0)),
            ("=COUNTIF([Words],\"*e*\")", n(2.0)),
            ("=COUNTIF([Words],\"\")", n(2.0)),
            ("=COUNTIF([Words],\"<>\")", n(3.0)),
            ("=COUNTIF([Words],\">b\")", n(2.0)),
            ("=COUNTIF([Bools],TRUE)", n(1.0)),
            ("=COUNTIF([Errs],\">0\")", n(2.0)),
            ("=COUNTIF(5,5)", Value::Error(E::Value)),
        ]);
    }

    #[test]
    fn sumif_and_averageif() {
        check(&[
            ("=SUMIF([Nums],\">2\")", n(7.0)),
            ("=SUMIF([Words],\"<>apple\",[Nums])", n(9.0)),
            ("=SUMIF([Nums],\">10\")", n(0.0)),
            ("=SUMIF([Errs],\">2\")", n(3.0)),
            ("=SUMIF([Nums],\">0\",[Errs])", Value::Error(E::Div0)),
            ("=AVERAGEIF([Nums],\">=2\")", n(3.0)),
            ("=AVERAGEIF([Nums],\">10\")", Value::Error(E::Div0)),
            ("=SUMIF([Nums],NA())", Value::Error(E::NA)),
        ]);
    }
}
