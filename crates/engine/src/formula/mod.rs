//! Excel formulas: the AST, the parser that produces it, the values it computes, and the
//! evaluator that computes them.

mod ast;
mod eval;
mod functions;
mod parser;
mod value;

pub use ast::{A1, BinaryOp, ErrorKind, Expr, Reference, UnaryOp};
pub(crate) use eval::eval_arg;
pub use eval::{Arg, Args, Context, Function, Range, evaluate, function, functions};
pub use parser::{ParseError, ParseErrorKind, parse};
pub use value::{
    Value, binary, compare, date_from_serial, general, iso_from_serial, number_from_text, round15,
    serial_from_date, serial_from_iso, unary,
};
