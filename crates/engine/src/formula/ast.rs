use std::fmt;

/// Excel's error values. They are values, not failures: formulas produce and pass them on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    Null,
    Div0,
    Value,
    Ref,
    Name,
    Num,
    NA,
}

impl ErrorKind {
    pub const ALL: [Self; 7] = [
        Self::Null,
        Self::Div0,
        Self::Value,
        Self::Ref,
        Self::Name,
        Self::Num,
        Self::NA,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Null => "#NULL!",
            Self::Div0 => "#DIV/0!",
            Self::Value => "#VALUE!",
            Self::Ref => "#REF!",
            Self::Name => "#NAME?",
            Self::Num => "#NUM!",
            Self::NA => "#N/A",
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An A1 cell address with Excel's `$` anchors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct A1 {
    pub row: u32,
    pub col: u32,
    pub row_absolute: bool,
    pub col_absolute: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Reference {
    Cell(A1),
    Range(A1, A1),
    /// `[@Col]` (this row) or `Table[Col]` / `[Col]` (whole column). `table` is `None`
    /// when the formula lives in the table it refers to.
    Structured {
        table: Option<String>,
        column: String,
        this_row: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Plus,
    Minus,
    Percent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Concat,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Text(String),
    Bool(bool),
    Error(ErrorKind),
    /// An omitted argument, as in `IF(A1,,1)`.
    Missing,
    Ref(Reference),
    /// A bare name that is not a function call, cell, or boolean. Resolved at evaluation.
    Name(String),
    Call {
        name: String,
        args: Vec<Expr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
}
