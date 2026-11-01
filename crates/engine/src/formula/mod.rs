//! Excel formulas: the AST, the parser that produces it, and the values it computes.

mod ast;
mod parser;
mod value;

pub use ast::{A1, BinaryOp, ErrorKind, Expr, Reference, UnaryOp};
pub use parser::{ParseError, ParseErrorKind, parse};
pub use value::{
    Value, binary, compare, date_from_serial, general, iso_from_serial, number_from_text, round15,
    serial_from_date, serial_from_iso, unary,
};
