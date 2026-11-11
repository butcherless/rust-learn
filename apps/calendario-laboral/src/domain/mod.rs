//! Dominio puro: tipos, reglas y resolución. Sin E/S y sin estado mutable.

mod calendar;
mod date;
mod error;
mod kind;
mod month;
mod resolve;

pub use calendar::{Calendar, DEFAULT_VACATION_QUOTA, Hours, Mark, MarkId, Marking, WEEK, Year};
pub use date::{DateRange, check_year, days_of_year, parse_date};
pub use error::DomainError;
pub use kind::{Category, DayKind, MarkKind, Shift};
pub use month::{Month, MonthExt};
pub use resolve::{
    MonthTotal, Origin, ResolvedDay, Summary, resolve_day, resolve_year, summarize, summary_of,
};

/// Constructores compartidos por los tests.
#[cfg(test)]
pub mod fixtures {
    use chrono::NaiveDate;

    use super::{Calendar, DateRange, MarkKind};

    /// Una fecha que existe.
    pub fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    /// Un rango válido.
    pub fn range(from: NaiveDate, to: NaiveDate) -> DateRange {
        DateRange::new(from, to).expect("valid range")
    }

    /// El calendario por defecto con 2026.
    pub fn calendar() -> Calendar {
        Calendar::default().with_year(2026).expect("new year")
    }

    /// [`calendar`] con `Mark` de 2026, dados como tipo, desde y hasta; sus ids empiezan en 1.
    pub fn marked(marks: &[(MarkKind, &str, &str)]) -> Calendar {
        marks.iter().fold(calendar(), |calendar, (kind, from, to)| {
            let range = range(
                super::parse_date(from).expect("from"),
                super::parse_date(to).expect("to"),
            );
            calendar.with_mark(*kind, range, None).expect("mark").0
        })
    }
}
