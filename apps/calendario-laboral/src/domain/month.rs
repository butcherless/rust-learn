//! Los meses del año como enumeración de negocio, en lugar de números de 1 a 12 (RNF-08).

use chrono::{Datelike, NaiveDate};

/// Un mes del año. El orden de las variantes es el del calendario, así que `Ord` sirve para
/// comparar meses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Month {
    /// Enero.
    January,
    /// Febrero.
    February,
    /// Marzo.
    March,
    /// Abril.
    April,
    /// Mayo.
    May,
    /// Junio.
    June,
    /// Julio.
    July,
    /// Agosto.
    August,
    /// Septiembre.
    September,
    /// Octubre.
    October,
    /// Noviembre.
    November,
    /// Diciembre.
    December,
}

impl Month {
    /// Todos los meses, de enero a diciembre.
    pub const ALL: [Self; 12] = [
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

    /// El mes de `date`.
    pub fn of(date: NaiveDate) -> Self {
        match date.month() {
            1 => Self::January,
            2 => Self::February,
            3 => Self::March,
            4 => Self::April,
            5 => Self::May,
            6 => Self::June,
            7 => Self::July,
            8 => Self::August,
            9 => Self::September,
            10 => Self::October,
            11 => Self::November,
            // chrono solo da meses de 1 a 12.
            _ => Self::December,
        }
    }

    /// El número del mes, de 1 a 12. Solo para construir fechas con chrono.
    pub const fn number(self) -> u32 {
        match self {
            Self::January => 1,
            Self::February => 2,
            Self::March => 3,
            Self::April => 4,
            Self::May => 5,
            Self::June => 6,
            Self::July => 7,
            Self::August => 8,
            Self::September => 9,
            Self::October => 10,
            Self::November => 11,
            Self::December => 12,
        }
    }

    /// Si el mes es del `Summer` fijo, julio y agosto (RN-08).
    pub const fn is_summer(self) -> bool {
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
                Month::of(date(2026, month.number(), 15)),
                month,
                "{month:?}"
            );
        });
        assert_eq!(
            Month::ALL.map(Month::number),
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
