//! El mes es `chrono::Month`, una enumeración como `Weekday` (RNF-08); [`MonthExt`] le añade lo
//! que necesita el dominio.

pub use chrono::Month;
use chrono::{Datelike, NaiveDate};

pub trait MonthExt {
    /// Los doce meses, en orden.
    const ALL: [Month; 12];

    /// El mes de una fecha.
    fn of(date: NaiveDate) -> Month;

    /// Si el mes es `Summer` (RN-08).
    fn is_summer(&self) -> bool;
}

impl MonthExt for Month {
    const ALL: [Self; 12] = [
        Self::January,
        Self::February,
        Self::March,
        Self::April,
        Self::May,
        Self::June,
        Self::July,
        Self::August,
        Self::September,
        Self::October,
        Self::November,
        Self::December,
    ];

    fn of(date: NaiveDate) -> Self {
        // Avanza desde enero, en lugar de convertir el número, para no tener un caso imposible.
        (0..date.month0()).fold(Self::January, |month, _| month.succ())
    }

    fn is_summer(&self) -> bool {
        match self {
            Self::July | Self::August => true,
            Self::January
            | Self::February
            | Self::March
            | Self::April
            | Self::May
            | Self::June
            | Self::September
            | Self::October
            | Self::November
            | Self::December => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::date;

    #[test]
    fn every_month_is_its_date_month_and_number() {
        Month::ALL.into_iter().for_each(|month| {
            assert_eq!(
                Month::of(date(2026, month.number_from_month(), 15)),
                month,
                "{month:?}"
            );
        });
        assert_eq!(
            Month::ALL.map(|month| month.number_from_month()),
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
        );
    }

    #[test]
    fn rn_08_only_july_and_august_are_summer() {
        let summer: Vec<Month> = Month::ALL
            .into_iter()
            .filter(|month| month.is_summer())
            .collect();
        assert_eq!(summer, [Month::July, Month::August]);
        assert!(
            Month::June < Month::July,
            "los meses se ordenan como en el calendario"
        );
    }
}
