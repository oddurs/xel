//! What a table cell holds, as the app and the Python package see it.

use std::fmt;

use crate::formula::{date_from_serial, serial_from_date};

/// A real calendar date within Excel's range, 1900-01-01 to 9999-12-31, so every stored
/// date has an Excel serial for formulas. Excel's phantom 1900-02-29 is not a date a
/// column can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    year: u16,
    month: u8,
    day: u8,
    serial: u32,
}

impl Date {
    #[must_use]
    pub fn new(year: u16, month: u8, day: u8) -> Option<Self> {
        if (year, month, day) == (1900, 2, 29) {
            return None;
        }
        let serial = serial_from_date(year.into(), month.into(), day.into())?;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "serials are whole numbers in 1..=2958465"
        )]
        let serial = serial as u32;
        Some(Self {
            year,
            month,
            day,
            serial,
        })
    }

    /// Parses `YYYY-MM-DD`, the form date columns store.
    #[must_use]
    pub fn from_iso(s: &str) -> Option<Self> {
        let b = s.as_bytes();
        if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
            return None;
        }
        let digits = |r: std::ops::Range<usize>| {
            let part = &s[r];
            part.bytes()
                .all(|c| c.is_ascii_digit())
                .then(|| part.parse().ok())
                .flatten()
        };
        Self::new(
            digits(0..4)?,
            u8::try_from(digits(5..7)?).ok()?,
            u8::try_from(digits(8..10)?).ok()?,
        )
    }

    /// The date of an Excel serial's whole part.
    #[must_use]
    pub fn from_serial(serial: f64) -> Option<Self> {
        let (y, m, d) = date_from_serial(serial)?;
        Self::new(y.try_into().ok()?, m.try_into().ok()?, d.try_into().ok()?)
    }

    #[must_use]
    pub fn serial(self) -> f64 {
        f64::from(self.serial)
    }

    #[must_use]
    pub fn year(self) -> u16 {
        self.year
    }

    #[must_use]
    pub fn month(self) -> u8 {
        self.month
    }

    #[must_use]
    pub fn day(self) -> u8 {
        self.day
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// The value of one cell. Which variants a column accepts depends on its type; `Empty`
/// fits every column.
#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    Empty,
    /// For text and select columns.
    Text(String),
    Number(f64),
    Checkbox(bool),
    Date(Date),
}

impl CellValue {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Text(_) => "text",
            Self::Number(_) => "number",
            Self::Checkbox(_) => "checkbox",
            Self::Date(_) => "date",
        }
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, reason = "serials are exact whole numbers")]
mod tests {
    use super::*;

    #[test]
    fn dates_are_valid_calendar_dates_in_excels_range() {
        assert!(Date::new(2024, 2, 29).is_some());
        assert!(Date::new(2023, 2, 29).is_none());
        assert!(
            Date::new(1899, 12, 31).is_none(),
            "before Excel's first date"
        );
        assert!(Date::new(2024, 0, 1).is_none());
        assert!(Date::new(1900, 2, 29).is_none(), "Excel's phantom leap day");
        assert_eq!(Date::from_serial(60.0), None);
        assert_eq!(Date::new(1900, 3, 1).unwrap().serial(), 61.0);
        assert_eq!(
            Date::from_iso("2024-02-29")
                .map(|d| d.to_string())
                .as_deref(),
            Some("2024-02-29")
        );
        assert_eq!(Date::from_iso("2024-02-29").unwrap().serial(), 45351.0);
        assert_eq!(Date::from_serial(45351.9), Date::new(2024, 2, 29));
        for bad in [
            "2024-2-29",
            "2024-02-30",
            "24-02-29",
            "2024/02/29",
            "2024-02-2x",
        ] {
            assert!(Date::from_iso(bad).is_none(), "{bad}");
        }
    }
}
