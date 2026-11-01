//! Functions that select cells by a criterion: SUMIF, COUNTIF, AVERAGEIF.
//!
//! A criterion is a value or text such as `">=100"`, `"<>done"`, `"a*"`, `""`. Text
//! criteria may use `*` and `?` wildcards, with `~` escaping them, and ignore case.

use std::cmp::Ordering;

use super::result;
use crate::formula::ast::ErrorKind;
use crate::formula::eval::{Arg, Args, Range, as_range};
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

impl Op {
    fn holds(self, o: Ordering) -> bool {
        match self {
            Self::Eq => o == Ordering::Equal,
            Self::Ne => o != Ordering::Equal,
            Self::Lt => o == Ordering::Less,
            Self::Le => o != Ordering::Greater,
            Self::Gt => o == Ordering::Greater,
            Self::Ge => o != Ordering::Less,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tok {
    Any,
    One,
    Char(char),
}

#[derive(Debug, Clone, PartialEq)]
enum Target {
    Number(f64),
    Bool(bool),
    /// The text as written (for ordering) and its lower-cased wildcard pattern.
    Text {
        text: String,
        pattern: Vec<Tok>,
    },
    /// `""` matches empty cells and empty text.
    BlankOrEmptyText,
    /// `"="` matches only truly empty cells; `"<>"` matches anything else, including "".
    Empty,
}

#[derive(Debug, Clone, PartialEq)]
struct Criterion {
    op: Op,
    target: Target,
}

impl Criterion {
    fn parse(v: &Value) -> Result<Self, ErrorKind> {
        let eq = |target| Ok(Self { op: Op::Eq, target });
        let text = match v {
            Value::Number(n) | Value::Date(n) => return eq(Target::Number(*n)),
            Value::Bool(b) => return eq(Target::Bool(*b)),
            // Excel's documentation: an empty criteria cell is treated as 0.
            Value::Empty => return eq(Target::Number(0.0)),
            Value::Error(e) => return Err(*e),
            Value::Text(s) if s.is_empty() => return eq(Target::BlankOrEmptyText),
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
            Target::Empty
        } else if let Some(n) = number_from_text(rest) {
            Target::Number(n)
        } else if rest.eq_ignore_ascii_case("TRUE") {
            Target::Bool(true)
        } else if rest.eq_ignore_ascii_case("FALSE") {
            Target::Bool(false)
        } else {
            Target::Text {
                text: rest.into(),
                pattern: tokens(&rest.to_lowercase()),
            }
        };
        Ok(Self { op, target })
    }

    fn matches(&self, cell: &Value) -> bool {
        let empty = matches!(cell, Value::Empty);
        match &self.target {
            Target::BlankOrEmptyText => empty || matches!(cell, Value::Text(s) if s.is_empty()),
            Target::Empty => (self.op == Op::Ne) != empty,
            Target::Number(n) => {
                let cell_number = match cell {
                    Value::Number(x) | Value::Date(x) => Some(*x),
                    // Text that reads as a number matches an equality, not an inequality.
                    Value::Text(s) if self.op == Op::Eq => number_from_text(s),
                    _ => None,
                };
                match cell_number {
                    Some(x) => compare(&Value::Number(x), &Value::Number(*n))
                        .is_ok_and(|o| self.op.holds(o)),
                    None => self.op == Op::Ne,
                }
            }
            Target::Bool(b) => match cell {
                Value::Bool(x) => self.op.holds(x.cmp(b)),
                _ => self.op == Op::Ne,
            },
            Target::Text { text, pattern } => match (cell, self.op) {
                (Value::Text(s), Op::Eq) => wildcard(pattern, &s.to_lowercase()),
                (Value::Text(s), Op::Ne) => !wildcard(pattern, &s.to_lowercase()),
                // Ordering text compares as the < and > operators do.
                (Value::Text(s), op) => {
                    compare(&Value::Text(s.clone()), &Value::Text(text.clone()))
                        .is_ok_and(|o| op.holds(o))
                }
                (_, op) => op == Op::Ne,
            },
        }
    }
}

/// A wildcard pattern: `*` any run, `?` one character, `~` makes the next literal.
fn tokens(pattern: &str) -> Vec<Tok> {
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
    toks
}

/// Matches `text` against a tokenized pattern, backtracking to the last `*`.
fn wildcard(toks: &[Tok], text: &str) -> bool {
    let text: Vec<char> = text.chars().collect();
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
        Arg::Value(Value::Error(e)) => Err(e),
        Arg::Value(_) => Err(ErrorKind::Value),
        reference => Ok(as_range(reference)),
    }
}

/// The values of the sum range (or of `range` itself) where `range` matches, keeping only
/// numbers, as SUMIF and AVERAGEIF do. Errors in matched cells propagate.
fn matched_numbers(args: &Args<'_>) -> Result<Vec<f64>, ErrorKind> {
    let range = cells(args.arg(0))?;
    let criterion = Criterion::parse(&args.value(1))?;
    let target = if args.is_missing(2) {
        range.clone()
    } else {
        cells(args.arg(2))?
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
    use super::{tokens, wildcard};
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
            assert_eq!(
                wildcard(&tokens(pattern), text),
                want,
                "{pattern} vs {text}"
            );
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
            ("=COUNTIF([Words],\"<>\")", n(4.0)),
            ("=COUNTIF([Words],\"=\")", n(1.0)),
            ("=COUNTIF([Mixed],\">1\")", n(1.0)),
            ("=COUNTIF([Zero],)", n(2.0)),
            ("=COUNTIF([Words],\">_\")", n(3.0)),
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
