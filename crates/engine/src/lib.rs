//! The xel engine: storage, formulas, and recalculation for xel workbooks.

mod cell_ref;
mod error;
#[cfg(test)]
mod test_support;
mod workbook;

pub use cell_ref::{CellRef, ParseCellRefError};
pub use error::{Error, Result};
pub use workbook::{FORMAT_VERSION, Workbook};
