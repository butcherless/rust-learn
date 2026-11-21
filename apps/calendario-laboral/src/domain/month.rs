//! El mes es [`Month`], una enumeración como `Weekday` (RNF-08). jiff representa el mes como un
//! número, así que este tipo lo envuelve.
//!
//! El dominio no compara números de mes: [`Month::number`] solo sirve para construir fechas.

use jiff::civil::Date;

/// Un mes del año.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Month {
    January,
    February,
    March,
    April,
    May,
    June,
    July,
    August,
    September,
    October,
    November,
    December,
}

impl Month {
    /// Los doce meses, en orden.
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

    /// El mes de una fecha.
    pub fn of(date: Date) -> Self {
        // Avanza desde enero, en lugar de convertir el número, para no tener un caso imposible.
        (1..date.month()).fold(Self::January, |month, _| month.succ())
    }

    /// El número del mes, de 1 a 12, para construir fechas.
    pub const fn number(self) -> i8 {
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

    /// El mes siguiente; tras diciembre, enero.
    pub const fn succ(self) -> Self {
        match self {
            Self::January => Self::February,
            Self::February => Self::March,
            Self::March => Self::April,
            Self::April => Self::May,
            Self::May => Self::June,
            Self::June => Self::July,
            Self::July => Self::August,
            Self::August => Self::September,
            Self::September => Self::October,
            Self::October => Self::November,
            Self::November => Self::December,
            Self::December => Self::January,
        }
    }

    /// Si el mes es `Summer` (RN-08).
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
        assert_eq!(
            Month::ALL.map(Month::succ),
            [
                Month::February,
                Month::March,
                Month::April,
                Month::May,
                Month::June,
                Month::July,
                Month::August,
                Month::September,
                Month::October,
                Month::November,
                Month::December,
                Month::January,
            ]
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
