//! The xel engine: storage, formulas, and recalculation for xel workbooks.

// Fallible calls return one documented error type per module (`Error`, `ParseError`); an
// `# Errors` section on each function would repeat it.
#![allow(clippy::missing_errors_doc)]

mod cell_ref;
mod error;
pub mod formula;
mod schema;
#[cfg(test)]
mod test_support;
mod workbook;

pub use cell_ref::{CellRef, ParseCellRefError};
pub use error::{Error, Result};
pub use schema::{Column, ColumnId, ColumnType, Table, TableId};
pub use workbook::{FORMAT_VERSION, Workbook};
