use super::ast::{A1, BinaryOp, ErrorKind, Expr, Reference, UnaryOp};
use crate::CellRef;

/// Excel's limit on formula length, in characters.
const MAX_LEN: usize = 8192;
/// Deeper nesting than this is refused rather than risking the stack. Excel allows 64
/// levels of nested functions; parentheses and unary chains count here too.
const MAX_DEPTH: usize = 256;

/// Why a formula could not be parsed, and where.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{kind} (at character {})", .offset + 1)]
pub struct ParseError {
    /// Offset in characters from the start of the text passed to [`parse`].
    pub offset: usize,
    pub kind: ParseErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ParseErrorKind {
    #[error("formula is longer than {MAX_LEN} characters")]
    TooLong,
    #[error("formula is nested too deeply")]
    TooDeep,
    #[error("expected a value")]
    ExpectedValue,
    #[error("unexpected '{0}'")]
    Unexpected(char),
    #[error("expected ')'")]
    ExpectedCloseParen,
    #[error("expected ',' or ')'")]
    ExpectedArgSeparator,
    #[error("this parenthesis is never closed")]
    UnclosedParen,
    #[error("this function call is never closed")]
    UnclosedCall,
    #[error("this text is never closed with \"")]
    UnclosedText,
    #[error("unknown error value")]
    UnknownError,
    #[error("'{0}' is too large")]
    NumberTooLarge(String),
    #[error("expected a cell after ':'")]
    ExpectedCell,
    #[error("'{0}' is not a valid cell reference")]
    BadCellReference(String),
    #[error("this table reference is never closed with ']'")]
    UnclosedTableReference,
    #[error("unexpected '[' in a column name")]
    BracketInColumnName,
    #[error("unsupported table reference")]
    UnsupportedTableReference,
}

/// Parses formula text in Excel syntax. A leading `=` is optional.
pub fn parse(text: &str) -> Result<Expr, ParseError> {
    if text.chars().count() > MAX_LEN {
        return Err(ParseError {
            offset: MAX_LEN,
            kind: ParseErrorKind::TooLong,
        });
    }
    let mut p = Parser {
        src: text,
        pos: 0,
        depth: 0,
    };
    p.skip_ws();
    if p.peek() == Some('=') {
        p.pos += 1;
    }
    let expr = p.expr(0)?;
    p.skip_ws();
    match p.peek() {
        None => Ok(expr),
        Some(c) => Err(p.error_at(p.pos, ParseErrorKind::Unexpected(c))),
    }
}

struct Parser<'a> {
    src: &'a str,
    /// Byte offset into `src`; converted to characters only when reporting.
    pos: usize,
    depth: usize,
}

/// Binding powers. Excel's precedence, lowest first: comparison, `&`, `+ -`, `* /`,
/// `^`, `%`, then unary minus, which binds tighter than `^` (`-2^2` is 4).
const BP_POSTFIX_PERCENT: u8 = 11;
const BP_PREFIX: u8 = 13;

fn infix(op: BinaryOp) -> u8 {
    match op {
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
            1
        }
        BinaryOp::Concat => 3,
        BinaryOp::Add | BinaryOp::Sub => 5,
        BinaryOp::Mul | BinaryOp::Div => 7,
        BinaryOp::Pow => 9,
    }
}

/// Excel separates tokens with ASCII spaces and line breaks only.
fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

impl Parser<'_> {
    fn rest(&self) -> &str {
        &self.src[self.pos..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(is_space) {
            self.bump();
        }
    }

    fn error_at(&self, byte: usize, kind: ParseErrorKind) -> ParseError {
        ParseError {
            offset: self.src[..byte].chars().count(),
            kind,
        }
    }

    fn expr(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.error_at(self.pos, ParseErrorKind::TooDeep));
        }
        let mut left = self.prefix()?;
        loop {
            self.skip_ws();
            if self.peek() == Some('%') {
                if BP_POSTFIX_PERCENT < min_bp {
                    break;
                }
                self.bump();
                left = Expr::Unary {
                    op: UnaryOp::Percent,
                    expr: Box::new(left),
                };
                continue;
            }
            let Some((op, len)) = self.binary_op() else {
                break;
            };
            let bp = infix(op);
            if bp < min_bp {
                break;
            }
            self.pos += len;
            // Left-associative, including `^`: Excel evaluates 2^3^2 as 64.
            let right = self.expr(bp + 1)?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        self.depth -= 1;
        Ok(left)
    }

    fn binary_op(&self) -> Option<(BinaryOp, usize)> {
        let r = self.rest();
        let two = [
            ("<>", BinaryOp::Ne),
            ("<=", BinaryOp::Le),
            (">=", BinaryOp::Ge),
        ];
        if let Some((_, op)) = two.iter().find(|(s, _)| r.starts_with(s)) {
            return Some((*op, 2));
        }
        let op = match r.chars().next()? {
            '+' => BinaryOp::Add,
            '-' => BinaryOp::Sub,
            '*' => BinaryOp::Mul,
            '/' => BinaryOp::Div,
            '^' => BinaryOp::Pow,
            '&' => BinaryOp::Concat,
            '=' => BinaryOp::Eq,
            '<' => BinaryOp::Lt,
            '>' => BinaryOp::Gt,
            _ => return None,
        };
        Some((op, 1))
    }

    fn prefix(&mut self) -> Result<Expr, ParseError> {
        self.skip_ws();
        let start = self.pos;
        let Some(c) = self.peek() else {
            return Err(self.error_at(start, ParseErrorKind::ExpectedValue));
        };
        match c {
            '+' | '-' => {
                self.bump();
                let op = if c == '-' {
                    UnaryOp::Minus
                } else {
                    UnaryOp::Plus
                };
                let expr = self.expr(BP_PREFIX)?;
                Ok(Expr::Unary {
                    op,
                    expr: Box::new(expr),
                })
            }
            '(' => {
                self.bump();
                let inner = self.expr(0)?;
                self.skip_ws();
                match self.peek() {
                    Some(')') => {
                        self.bump();
                        Ok(inner)
                    }
                    Some(_) => Err(self.error_at(self.pos, ParseErrorKind::ExpectedCloseParen)),
                    None => Err(self.error_at(start, ParseErrorKind::UnclosedParen)),
                }
            }
            '"' => self.string(),
            '#' => self.error_literal(),
            '[' => Ok(Expr::Ref(self.structured(None)?)),
            c if c.is_ascii_digit() || c == '.' => self.number(),
            c if c.is_alphabetic() || c == '_' || c == '$' || c == '\\' => self.word(),
            _ => Err(self.error_at(start, ParseErrorKind::Unexpected(c))),
        }
    }

    fn number(&mut self) -> Result<Expr, ParseError> {
        let start = self.pos;
        let digits = |p: &mut Self| {
            let from = p.pos;
            while p.peek().is_some_and(|c| c.is_ascii_digit()) {
                p.bump();
            }
            p.pos > from
        };
        let int = digits(self);
        let mut frac = false;
        if self.peek() == Some('.') {
            self.bump();
            frac = digits(self);
        }
        if !int && !frac {
            return Err(self.error_at(start, ParseErrorKind::Unexpected('.')));
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            let mark = self.pos;
            self.bump();
            if matches!(self.peek(), Some('+' | '-')) {
                self.bump();
            }
            if !digits(self) {
                // `1E` is not a number with an exponent; leave the letter for the caller.
                self.pos = mark;
            }
        }
        let text = &self.src[start..self.pos];
        // The scan above admits only shapes `f64::from_str` accepts.
        let value: f64 = text.parse().expect("scanned digits form a valid float");
        if !value.is_finite() {
            return Err(self.error_at(start, ParseErrorKind::NumberTooLarge(text.into())));
        }
        Ok(Expr::Number(value))
    }

    fn string(&mut self) -> Result<Expr, ParseError> {
        let start = self.pos;
        self.bump();
        let mut out = String::new();
        loop {
            match self.bump() {
                Some('"') if self.peek() == Some('"') => {
                    self.bump();
                    out.push('"');
                }
                Some('"') => return Ok(Expr::Text(out)),
                Some(c) => out.push(c),
                None => return Err(self.error_at(start, ParseErrorKind::UnclosedText)),
            }
        }
    }

    fn error_literal(&mut self) -> Result<Expr, ParseError> {
        let start = self.pos;
        let rest = self.rest();
        for kind in ErrorKind::ALL {
            let s = kind.as_str();
            if rest
                .get(..s.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(s))
            {
                self.pos += s.len();
                return Ok(Expr::Error(kind));
            }
        }
        Err(self.error_at(start, ParseErrorKind::UnknownError))
    }

    /// A function name, boolean, cell or range, table reference, or defined name.
    fn word(&mut self) -> Result<Expr, ParseError> {
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '$' | '\\'))
        {
            self.bump();
        }
        let word = &self.src[start..self.pos];
        let bad_cell = || ParseErrorKind::BadCellReference(word.into());

        match self.peek() {
            // Function names may look like cells (LOG10, ATAN2, DAYS360), but never hold `$`.
            Some('(' | '[') if word.contains('$') => {
                return Err(self.error_at(start, bad_cell()));
            }
            Some('(') => {
                self.bump();
                let args = self.args(start)?;
                return Ok(Expr::Call {
                    name: word.to_ascii_uppercase(),
                    args,
                });
            }
            Some('[') => return Ok(Expr::Ref(self.structured(Some(word.to_string()))?)),
            _ => {}
        }
        if word.eq_ignore_ascii_case("TRUE") {
            return Ok(Expr::Bool(true));
        }
        if word.eq_ignore_ascii_case("FALSE") {
            return Ok(Expr::Bool(false));
        }
        if let Some(a) = parse_a1(word) {
            if self.peek() == Some(':') {
                let colon = self.pos;
                self.bump();
                let from = self.pos;
                while self
                    .peek()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '$')
                {
                    self.bump();
                }
                let b = parse_a1(&self.src[from..self.pos])
                    .ok_or_else(|| self.error_at(colon, ParseErrorKind::ExpectedCell))?;
                let (a, b) = normalize(a, b);
                return Ok(Expr::Ref(Reference::Range(a, b)));
            }
            return Ok(Expr::Ref(Reference::Cell(a)));
        }
        if word.contains('$') {
            return Err(self.error_at(start, bad_cell()));
        }
        Ok(Expr::Name(word.to_string()))
    }

    fn args(&mut self, call_start: usize) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        self.skip_ws();
        if self.peek() == Some(')') {
            self.bump();
            return Ok(args);
        }
        loop {
            self.skip_ws();
            if matches!(self.peek(), Some(',' | ')')) {
                args.push(Expr::Missing);
            } else {
                args.push(self.expr(0)?);
            }
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.bump();
                }
                Some(')') => {
                    self.bump();
                    return Ok(args);
                }
                Some(_) => {
                    return Err(self.error_at(self.pos, ParseErrorKind::ExpectedArgSeparator));
                }
                None => return Err(self.error_at(call_start, ParseErrorKind::UnclosedCall)),
            }
        }
    }

    /// `[@Col]`, `[@[Col Name]]`, `[Col]`, `Table[Col]`, `Table[@Col]`. The cursor is on
    /// the opening `[`.
    fn structured(&mut self, table: Option<String>) -> Result<Reference, ParseError> {
        let start = self.pos;
        self.bump();
        let this_row = self.peek() == Some('@');
        if this_row {
            self.bump();
        }
        let nested = self.peek() == Some('[');
        if nested {
            self.bump();
        }
        // Special items such as `[#This Row]` start with an unescaped `#`; `'#` is a
        // literal `#` in a column name.
        if self.peek() == Some('#') {
            return Err(self.error_at(start, ParseErrorKind::UnsupportedTableReference));
        }
        let column = self.bracketed(start)?;
        if nested && self.bump() != Some(']') {
            return Err(self.error_at(start, ParseErrorKind::UnclosedTableReference));
        }
        if column.is_empty() {
            return Err(self.error_at(start, ParseErrorKind::UnsupportedTableReference));
        }
        Ok(Reference::Structured {
            table,
            column,
            this_row,
        })
    }

    /// Reads a column name up to and including the closing `]`. `'` escapes the next
    /// character, as in Excel (`[Q1'[est']]`).
    fn bracketed(&mut self, start: usize) -> Result<String, ParseError> {
        let mut out = String::new();
        loop {
            match self.bump() {
                Some(']') => return Ok(out),
                Some('[') => {
                    return Err(self.error_at(start, ParseErrorKind::BracketInColumnName));
                }
                Some('\'') => match self.bump() {
                    Some(c) => out.push(c),
                    None => break,
                },
                Some(c) => out.push(c),
                None => break,
            }
        }
        Err(self.error_at(start, ParseErrorKind::UnclosedTableReference))
    }
}

/// `B12`, `$B12`, `B$12`, `$B$12`, `B012`, within Excel's grid.
fn parse_a1(word: &str) -> Option<A1> {
    let col_absolute = word.starts_with('$');
    let rest = if col_absolute { &word[1..] } else { word };
    let letters_end = rest
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(rest.len());
    let (letters, rest) = rest.split_at(letters_end);
    let row_absolute = rest.starts_with('$');
    let digits = if row_absolute { &rest[1..] } else { rest };
    if letters.len() > 3 {
        return None;
    }
    let cell = CellRef::from_parts(letters, digits).ok()?;
    Some(A1 {
        row: cell.row,
        col: cell.col,
        row_absolute,
        col_absolute,
    })
}

/// Orders a range's corners top-left to bottom-right, as Excel does on entry
/// (`B2:A1` becomes `A1:B2`). Each `$` stays with the row or column it anchors.
fn normalize(a: A1, b: A1) -> (A1, A1) {
    let (top, bottom) = if a.row <= b.row { (a, b) } else { (b, a) };
    let (left, right) = if a.col <= b.col { (a, b) } else { (b, a) };
    (
        A1 {
            row: top.row,
            row_absolute: top.row_absolute,
            col: left.col,
            col_absolute: left.col_absolute,
        },
        A1 {
            row: bottom.row,
            row_absolute: bottom.row_absolute,
            col: right.col,
            col_absolute: right.col_absolute,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ParseErrorKind as K;

    fn p(s: &str) -> Expr {
        parse(s).unwrap_or_else(|e| panic!("{s}: {e}"))
    }

    fn num(n: f64) -> Expr {
        Expr::Number(n)
    }

    fn bin(op: BinaryOp, l: Expr, r: Expr) -> Expr {
        Expr::Binary {
            op,
            left: Box::new(l),
            right: Box::new(r),
        }
    }

    fn un(op: UnaryOp, e: Expr) -> Expr {
        Expr::Unary {
            op,
            expr: Box::new(e),
        }
    }

    fn col(name: &str, this_row: bool) -> Expr {
        Expr::Ref(Reference::Structured {
            table: None,
            column: name.into(),
            this_row,
        })
    }

    fn a1(row: u32, col: u32, row_absolute: bool, col_absolute: bool) -> A1 {
        A1 {
            row,
            col,
            row_absolute,
            col_absolute,
        }
    }

    #[test]
    fn unary_minus_binds_tighter_than_power() {
        assert_eq!(
            p("=-2^2"),
            bin(BinaryOp::Pow, un(UnaryOp::Minus, num(2.0)), num(2.0))
        );
    }

    #[test]
    fn excel_precedence_and_associativity() {
        use BinaryOp::*;
        assert_eq!(p("1+2*3"), bin(Add, num(1.0), bin(Mul, num(2.0), num(3.0))));
        assert_eq!(p("2^3^2"), bin(Pow, bin(Pow, num(2.0), num(3.0)), num(2.0)));
        assert_eq!(p("1-2-3"), bin(Sub, bin(Sub, num(1.0), num(2.0)), num(3.0)));
        assert_eq!(
            p("1&2+3"),
            bin(Concat, num(1.0), bin(Add, num(2.0), num(3.0))),
            "+ binds tighter than &"
        );
        assert_eq!(
            p("1=2&3"),
            bin(Eq, num(1.0), bin(Concat, num(2.0), num(3.0))),
            "& binds tighter than ="
        );
        assert_eq!(
            p("2^50%"),
            bin(Pow, num(2.0), un(UnaryOp::Percent, num(50.0))),
            "% binds tighter than ^"
        );
        assert_eq!(
            p("-50%"),
            un(UnaryOp::Percent, un(UnaryOp::Minus, num(50.0))),
            "negation binds tighter than % in Excel"
        );
        assert_eq!(p("1--1"), bin(Sub, num(1.0), un(UnaryOp::Minus, num(1.0))));
        assert_eq!(
            p("2*-3^2"),
            bin(
                Mul,
                num(2.0),
                bin(Pow, un(UnaryOp::Minus, num(3.0)), num(2.0))
            )
        );
        assert_eq!(p("1<>2"), bin(Ne, num(1.0), num(2.0)));
        assert_eq!(p("1<=2"), bin(Le, num(1.0), num(2.0)));
    }

    #[test]
    fn literals() {
        assert_eq!(p("1.5e3"), num(1500.0));
        assert_eq!(p(".5"), num(0.5));
        assert_eq!(p("5."), num(5.0));
        assert_eq!(p(r#""say ""hi""""#), Expr::Text(r#"say "hi""#.into()));
        assert_eq!(p("true"), Expr::Bool(true));
        assert_eq!(p("#div/0!"), Expr::Error(ErrorKind::Div0));
        assert_eq!(p("#N/A"), Expr::Error(ErrorKind::NA));
    }

    #[test]
    fn references() {
        assert_eq!(
            p("B12"),
            Expr::Ref(Reference::Cell(a1(11, 1, false, false)))
        );
        assert_eq!(
            p("$B$12"),
            Expr::Ref(Reference::Cell(a1(11, 1, true, true)))
        );
        assert_eq!(
            p("B$12"),
            Expr::Ref(Reference::Cell(a1(11, 1, true, false)))
        );
        assert_eq!(
            p("A01"),
            p("A1"),
            "leading zeros are accepted, as Excel does"
        );
        assert_eq!(
            p("$B$007"),
            Expr::Ref(Reference::Cell(a1(6, 1, true, true)))
        );
        assert_eq!(
            p("A1:$C$3"),
            Expr::Ref(Reference::Range(
                a1(0, 0, false, false),
                a1(2, 2, true, true)
            ))
        );
        assert_eq!(p("[@Amount]"), col("Amount", true));
        assert_eq!(p("[@[Unit Price]]"), col("Unit Price", true));
        assert_eq!(p("[Amount]"), col("Amount", false));
        assert_eq!(p("[Q1'[est']]"), col("Q1[est]", false));
        assert_eq!(
            p("['#Items]"),
            col("#Items", false),
            "an escaped # is a literal"
        );
        assert_eq!(
            p("Orders[Amount]"),
            Expr::Ref(Reference::Structured {
                table: Some("Orders".into()),
                column: "Amount".into(),
                this_row: false
            })
        );
        assert_eq!(p("TaxRate"), Expr::Name("TaxRate".into()));
        assert_eq!(
            p("XFE1"),
            Expr::Name("XFE1".into()),
            "beyond the grid is a name"
        );
    }

    #[test]
    fn reversed_ranges_are_normalized() {
        assert_eq!(p("B2:A1"), p("A1:B2"));
        assert_eq!(
            p("B$2:$A1"),
            Expr::Ref(Reference::Range(
                a1(0, 0, false, true),
                a1(1, 1, true, false)
            )),
            "each $ stays with the row or column it anchors"
        );
        assert_eq!(
            p("A2:B1"),
            Expr::Ref(Reference::Range(
                a1(0, 0, false, false),
                a1(1, 1, false, false)
            ))
        );
    }

    #[test]
    fn calls() {
        assert_eq!(
            p("sum(1, 2)"),
            Expr::Call {
                name: "SUM".into(),
                args: vec![num(1.0), num(2.0)]
            }
        );
        assert_eq!(
            p("NOW()"),
            Expr::Call {
                name: "NOW".into(),
                args: vec![]
            }
        );
        assert_eq!(
            p("IF(A1,,1)"),
            Expr::Call {
                name: "IF".into(),
                args: vec![p("A1"), Expr::Missing, num(1.0)]
            }
        );
        assert_eq!(
            p("LOG10(100)"),
            Expr::Call {
                name: "LOG10".into(),
                args: vec![num(100.0)]
            },
            "a function whose name looks like a cell"
        );
        assert_eq!(
            p("STDEV.S(1)"),
            Expr::Call {
                name: "STDEV.S".into(),
                args: vec![num(1.0)]
            }
        );
        assert_eq!(
            p("=[@Qty] * [@Price]"),
            bin(BinaryOp::Mul, col("Qty", true), col("Price", true))
        );
    }

    #[test]
    fn errors_point_at_the_problem() {
        let bad_cell = |s: &str| K::BadCellReference(s.into());
        let cases = [
            ("=1+", 3, K::ExpectedValue),
            ("=(1+2", 1, K::UnclosedParen),
            ("=(1 2)", 4, K::ExpectedCloseParen),
            ("=\"abc", 1, K::UnclosedText),
            ("=SUM(1,2", 1, K::UnclosedCall),
            ("=SUM(1 2)", 7, K::ExpectedArgSeparator),
            ("=1 2", 3, K::Unexpected('2')),
            ("=#WAT", 1, K::UnknownError),
            ("=[@Amount", 1, K::UnclosedTableReference),
            ("=[#This Row]", 1, K::UnsupportedTableReference),
            ("=A1:", 3, K::ExpectedCell),
            ("=$foo", 1, bad_cell("$foo")),
            ("=$SUM(1)", 1, bad_cell("$SUM")),
            ("=$A[x]", 1, bad_cell("$A")),
            ("=ü+?", 3, K::Unexpected('?')),
            ("=\u{a0}1", 1, K::Unexpected('\u{a0}')),
        ];
        for (text, offset, kind) in cases {
            let e = parse(text).expect_err(text);
            assert_eq!((e.offset, &e.kind), (offset, &kind), "{text}: {e}");
        }
    }

    #[test]
    fn deep_nesting_is_an_error_not_a_crash() {
        let deep = format!("={}1{}", "(".repeat(3000), ")".repeat(3000));
        assert_eq!(parse(&deep).unwrap_err().kind, K::TooDeep);
        let minus = format!("={}1", "-".repeat(3000));
        assert_eq!(parse(&minus).unwrap_err().kind, K::TooDeep);
    }

    #[test]
    fn overlong_formulas_are_refused() {
        let long = format!("={}", "1+".repeat(5000) + "1");
        assert_eq!(parse(&long).unwrap_err().kind, K::TooLong);
    }

    /// Formulas of the shapes found in real workbooks. Each must parse; the function
    /// reference (generated in v1.0) will add its own examples to this list.
    #[test]
    fn real_world_formulas_parse() {
        let corpus = [
            "=SUM(A1:A10)",
            "=SUM(Orders[Amount])",
            "=[@Qty]*[@[Unit Price]]",
            "=IF([@Status]=\"Paid\",[@Amount],0)",
            "=ROUND(B2*(1+$F$1),2)",
            "=AVERAGEIF(C2:C50,\">=100\")",
            "=TEXT(TODAY(),\"yyyy-mm-dd\")",
            "=LEFT(A1,FIND(\" \",A1)-1)",
            "=A1&\" \"&B1",
            "=-B3^2+4*B2*B4",
            "=IF(AND(A1>0,B1<10),\"ok\",IF(OR(C1,D1),\"maybe\",\"no\"))",
            "=SUMIF(Orders[Region],\"West\",Orders[Amount])/COUNT(Orders[Amount])",
            "=DATE(YEAR(A1),MONTH(A1)+1,1)-1",
            "=MAX(0,MIN(100,A1))",
            "=IF(ISBLANK(A1),,A1)",
            "=15%*B2",
            "=\"He said \"\"hi\"\"\"",
            "=#N/A",
            "=SWITCH(A1,1,\"one\",2,\"two\",\"many\")",
            "=TRUE<>FALSE",
            "=1E-3+.5",
        ];
        for text in corpus {
            parse(text).unwrap_or_else(|e| panic!("{text}: {e}"));
        }
        // Not yet supported; each must fail cleanly until its follow-up item lands. A
        // space before `(` is Excel's intersection operator, which Excel also rejects.
        for text in [
            "=COUNTIFS(A:A,\"x\")",
            "=IFERROR(VLOOKUP($A2,Sheet2!$A$1:$C$100,3,FALSE),\"\")",
            "=SUM({1,2;3,4})",
            "=@A1:A10",
            "=SUM(A1#)",
            "=Orders[[#This Row],[Amount]]",
            "=#SPILL!",
            "=  SUM ( 1 , 2 )",
        ] {
            assert!(
                parse(text).is_err(),
                "{text} now parses; move it to the corpus"
            );
        }
    }

    /// A stand-in for fuzzing: random strings over the formula alphabet must parse or fail
    /// cleanly, never panic. Runs 200,000 inputs by default; set `XEL_FUZZ_SECONDS` to run
    /// for that long instead, with a seed from the clock (printed so failures replay).
    #[test]
    fn random_input_never_panics() {
        const ALPHABET: &[&str] = &[
            "=", "+", "-", "*", "/", "^", "&", "%", "<", ">", "<>", "<=", "(", ")", ",", ":", "\"",
            "[", "]", "@", "'", "#", "$", "!", "?", ".", "e", " ", "1", "9", "A", "Z", "SUM", "IF",
            "TRUE", "#N/A", "#DIV/0!", "A1", "$B$2", "Orders", "ü", "😀", "\u{a0}", "['", "[[",
            "{", ";", "0",
        ];
        let seconds: Option<u64> = std::env::var("XEL_FUZZ_SECONDS")
            .ok()
            .map(|s| s.parse().expect("a number"));
        let mut state: u64 = match seconds {
            Some(_) => {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("clock after 1970")
                    .as_secs()
                    .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    | 1
            }
            None => 0x9E37_79B9_7F4A_7C15,
        };
        eprintln!("fuzz seed {state:#x}");
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let deadline =
            seconds.map(|s| std::time::Instant::now() + std::time::Duration::from_secs(s));
        let mut runs: u64 = 0;
        loop {
            let len = next() % 24;
            let text: String = (0..len)
                .map(|_| ALPHABET[usize::try_from(next() % ALPHABET.len() as u64).unwrap()])
                .collect();
            if let Err(e) = parse(&text) {
                assert!(
                    e.offset <= text.chars().count(),
                    "{text:?}: offset past the end"
                );
            }
            runs += 1;
            let done = match deadline {
                Some(d) => runs.is_multiple_of(10_000) && std::time::Instant::now() >= d,
                None => runs == 200_000,
            };
            if done {
                break;
            }
        }
        eprintln!("fuzzed {runs} inputs");
    }
}
