//! Fechas, rangos y la aritmética de calendario que necesitan las reglas.

use std::fmt;

use chrono::{Datelike, Months, NaiveDate};

use crate::domain::{DomainError, Month};

/// Lee una fecha ISO `YYYY-MM-DD` que exista (RN-01).
///
/// A diferencia del `%Y-%m-%d` de chrono, exige exactamente cuatro, dos y dos dígitos.
pub fn parse_date(text: &str) -> Result<NaiveDate, DomainError> {
    let well_formed = text.len() == 10
        && text.char_indices().all(|(index, character)| match index {
            4 | 7 => character == '-',
            _ => character.is_ascii_digit(),
        });
    well_formed
        .then(|| NaiveDate::parse_from_str(text, "%Y-%m-%d").ok())
        .flatten()
        .ok_or_else(|| DomainError::InvalidDate(text.to_owned()))
}

/// Comprueba que el año se puede escribir como año ISO de cuatro dígitos.
pub fn check_year(year: i32) -> Result<i32, DomainError> {
    (1..=9999)
        .contains(&year)
        .then_some(year)
        .ok_or(DomainError::InvalidYear(year))
}

/// Todos los días del año, 365 o 366 (RN-07).
pub fn days_of_year(year: i32) -> impl Iterator<Item = NaiveDate> {
    NaiveDate::from_ymd_opt(year, 1, 1)
        .into_iter()
        .flat_map(|first| first.iter_days())
        .take_while(move |date| date.year() == year)
}

/// Un rango de días con ambos extremos incluidos y `from` ≤ `to` (RN-03).
///
/// Un día suelto tiene `from` = `to`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DateRange {
    from: NaiveDate,
    to: NaiveDate,
}

impl DateRange {
    /// El rango de `from` a `to`, ambos incluidos.
    pub fn new(from: NaiveDate, to: NaiveDate) -> Result<Self, DomainError> {
        (from <= to)
            .then_some(Self { from, to })
            .ok_or(DomainError::ReversedRange { from, to })
    }

    /// El rango de un solo día.
    pub const fn day(date: NaiveDate) -> Self {
        Self {
            from: date,
            to: date,
        }
    }

    /// Todos los días de `month` en `year`.
    pub fn month(year: i32, month: Month) -> Result<Self, DomainError> {
        let from = NaiveDate::from_ymd_opt(year, month.number_from_month(), 1)
            .ok_or(DomainError::InvalidYear(year))?;
        from.checked_add_months(Months::new(1))
            .and_then(|next| next.pred_opt())
            .map(|to| Self { from, to })
            .ok_or(DomainError::InvalidYear(year))
    }

    /// Primer día.
    pub const fn from(self) -> NaiveDate {
        self.from
    }

    /// Último día.
    pub const fn to(self) -> NaiveDate {
        self.to
    }

    /// Si el rango cubre `date`.
    pub fn contains(self, date: NaiveDate) -> bool {
        (self.from..=self.to).contains(&date)
    }

    /// Si los dos rangos comparten algún día.
    pub fn overlaps(self, other: Self) -> bool {
        self.from <= other.to && other.from <= self.to
    }

    /// El año del rango, que no puede abarcar dos años (RN-02).
    pub fn year(self) -> Result<i32, DomainError> {
        (self.from.year() == self.to.year())
            .then_some(self.from.year())
            .ok_or(DomainError::CrossesYear {
                from: self.from,
                to: self.to,
            })
    }
}

impl fmt::Display for DateRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.from == self.to {
            write!(f, "{}", self.from)
        } else {
            write!(f, "{} → {}", self.from, self.to)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::MonthExt;
    use crate::domain::fixtures::{date, range};

    #[test]
    fn rn_01_dates_are_iso_and_must_exist() {
        [
            ("2026-06-05", Some(date(2026, 6, 5))),
            ("2024-02-29", Some(date(2024, 2, 29))),
            ("2026-02-30", None),
            ("2026-02-29", None),
            ("2026-13-01", None),
            ("2026-6-5", None),
            ("2026-06-5", None),
            ("05/06/2026", None),
            ("2026/06/05", None),
            ("+2026-06-0", None),
            ("", None),
        ]
        .into_iter()
        .for_each(|(text, expected)| {
            let expected = expected.ok_or_else(|| DomainError::InvalidDate(text.to_owned()));
            assert_eq!(parse_date(text), expected, "{text}");
        });
    }

    #[test]
    fn rn_02_ranges_that_cross_the_year_have_no_year() {
        [
            (date(2026, 1, 1), date(2026, 12, 31), Ok(2026)),
            (
                date(2026, 12, 28),
                date(2027, 1, 3),
                Err(DomainError::CrossesYear {
                    from: date(2026, 12, 28),
                    to: date(2027, 1, 3),
                }),
            ),
        ]
        .into_iter()
        .for_each(|(from, to, expected)| {
            assert_eq!(
                DateRange::new(from, to).and_then(DateRange::year),
                expected,
                "{from} {to}"
            );
        });
    }

    #[test]
    fn rn_03_ranges_start_before_they_end() {
        assert_eq!(
            DateRange::new(date(2026, 6, 12), date(2026, 6, 5)),
            Err(DomainError::ReversedRange {
                from: date(2026, 6, 12),
                to: date(2026, 6, 5)
            })
        );
        assert_eq!(
            DateRange::new(date(2026, 6, 5), date(2026, 6, 5)),
            Ok(DateRange::day(date(2026, 6, 5)))
        );
    }

    #[test]
    fn rn_07_leap_years_have_366_days() {
        [(2024, 366), (2026, 365), (2000, 366), (2100, 365)]
            .into_iter()
            .for_each(|(year, days)| assert_eq!(days_of_year(year).count(), days, "{year}"));
    }

    #[test]
    fn rn_08_summer_is_july_and_august() {
        [
            (date(2026, 6, 30), false),
            (date(2026, 7, 1), true),
            (date(2026, 8, 31), true),
            (date(2026, 9, 1), false),
        ]
        .into_iter()
        .for_each(|(day, summer)| assert_eq!(Month::of(day).is_summer(), summer, "{day}"));
    }

    #[test]
    fn months_and_years_are_checked() {
        assert_eq!(
            DateRange::month(2024, Month::February),
            DateRange::new(date(2024, 2, 1), date(2024, 2, 29))
        );
        assert_eq!(
            DateRange::month(2026, Month::December),
            DateRange::new(date(2026, 12, 1), date(2026, 12, 31))
        );
        assert_eq!(
            DateRange::month(262_142, Month::December),
            Err(DomainError::InvalidYear(262_142))
        );
        assert_eq!(check_year(2026), Ok(2026));
        assert_eq!(check_year(0), Err(DomainError::InvalidYear(0)));
        assert_eq!(check_year(10_000), Err(DomainError::InvalidYear(10_000)));
    }

    #[test]
    fn ranges_contain_overlap_and_display() {
        let june = range(date(2026, 6, 1), date(2026, 6, 30));
        assert!(june.contains(date(2026, 6, 30)));
        assert!(!june.contains(date(2026, 7, 1)));
        assert!(range(date(2026, 5, 30), date(2026, 6, 1)).overlaps(june));
        assert!(!DateRange::day(date(2026, 7, 1)).overlaps(june));
        assert_eq!(
            range(date(2026, 6, 5), date(2026, 6, 12)).to_string(),
            "2026-06-05 → 2026-06-12"
        );
        assert_eq!(DateRange::day(date(2026, 3, 3)).to_string(), "2026-03-03");
    }
}
