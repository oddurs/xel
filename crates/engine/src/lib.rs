//! The xel engine: storage, formulas, and recalculation for xel workbooks.

// Every fallible call in this crate returns the one documented `Error` enum; an
// `# Errors` section on each function would repeat it.
#![allow(clippy::missing_errors_doc)]

mod cell_ref;
mod error;
#[cfg(test)]
mod test_support;
mod workbook;

pub use cell_ref::{CellRef, ParseCellRefError};
pub use error::{Error, Result};
pub use workbook::{FORMAT_VERSION, Workbook};
