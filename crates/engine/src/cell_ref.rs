use std::fmt;
use std::str::FromStr;

/// Excel's grid limits. Calc blocks are free grids, and xlsx round-trips depend on
/// the same bounds Excel enforces.
const MAX_ROW: u32 = 1_048_576;
const MAX_COL: u32 = 16_384;

/// A cell address such as `B12`, stored zero-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellRef {
    pub row: u32,
    pub col: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseCellRefError {
    Malformed,
    OutOfRange,
}

impl fmt::Display for ParseCellRefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => f.write_str("expected a cell reference like A1"),
            Self::OutOfRange => write!(
                f,
                "cell reference is beyond {MAX_COL} columns or {MAX_ROW} rows"
            ),
        }
    }
}

impl std::error::Error for ParseCellRefError {}

impl FromStr for CellRef {
    type Err = ParseCellRefError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split = s
            .find(|c: char| c.is_ascii_digit())
            .ok_or(ParseCellRefError::Malformed)?;
        let (letters, digits) = s.split_at(split);
        if letters.is_empty()
            || !letters.bytes().all(|b| b.is_ascii_alphabetic())
            || !digits.bytes().all(|b| b.is_ascii_digit())
            || digits.starts_with('0')
        {
            return Err(ParseCellRefError::Malformed);
        }

        // Columns are bijective base 26: A=1 … Z=26, AA=27. There is no zero digit.
        let mut col: u32 = 0;
        for b in letters.bytes() {
            let digit = u32::from(b.to_ascii_uppercase() - b'A' + 1);
            col = col
                .checked_mul(26)
                .and_then(|c| c.checked_add(digit))
                .filter(|&c| c <= MAX_COL)
                .ok_or(ParseCellRefError::OutOfRange)?;
        }
        let row: u32 = digits.parse().map_err(|_| ParseCellRefError::OutOfRange)?;
        if row > MAX_ROW {
            return Err(ParseCellRefError::OutOfRange);
        }
        Ok(Self {
            row: row - 1,
            col: col - 1,
        })
    }
}

impl fmt::Display for CellRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut letters = Vec::new();
        let mut n = self.col + 1;
        while n > 0 {
            let rem = (n - 1) % 26;
            letters.push(b'A' + u8::try_from(rem).expect("remainder is below 26"));
            n = (n - 1) / 26;
        }
        letters.reverse();
        let letters = String::from_utf8(letters).expect("ASCII letters");
        write!(f, "{letters}{}", self.row + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<CellRef, ParseCellRefError> {
        s.parse()
    }

    #[test]
    fn parses_corners_and_wraparound_columns() {
        assert_eq!(parse("A1"), Ok(CellRef { row: 0, col: 0 }));
        assert_eq!(parse("z9"), Ok(CellRef { row: 8, col: 25 }));
        assert_eq!(parse("AA10"), Ok(CellRef { row: 9, col: 26 }));
        assert_eq!(
            parse("XFD1048576"),
            Ok(CellRef {
                row: MAX_ROW - 1,
                col: MAX_COL - 1
            })
        );
    }

    #[test]
    fn rejects_malformed_and_out_of_range() {
        for bad in ["", "A", "1", "A0", "A01", "1A", "A1B", "A-1", "Ä1"] {
            assert_eq!(parse(bad), Err(ParseCellRefError::Malformed), "{bad}");
        }
        for big in ["XFE1", "A1048577", "AAAAAAAAAAAA1", "A99999999999"] {
            assert_eq!(parse(big), Err(ParseCellRefError::OutOfRange), "{big}");
        }
    }

    #[test]
    fn display_round_trips_every_column() {
        for col in 0..MAX_COL {
            let r = CellRef { row: 41, col };
            assert_eq!(parse(&r.to_string()), Ok(r));
        }
    }
}
