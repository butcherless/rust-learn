//! Los mercados de RN-32: el IBEX 35 y el Brent, con su última cotización. Los datos llegan de
//! `adapters::markets`.

use chrono::{NaiveDate, NaiveTime};

/// Un valor de mercado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Market {
    /// El índice IBEX 35, en puntos.
    Ibex35,
    /// El futuro más cercano del Brent, en dólares por barril.
    Brent,
}

impl Market {
    /// Los valores, en el orden en que se muestran.
    pub const ALL: [Self; 2] = [Self::Ibex35, Self::Brent];

    /// El nombre que se muestra.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ibex35 => "IBEX 35",
            Self::Brent => "Brent ($)",
        }
    }
}

/// La última cotización de un valor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quote {
    /// El precio.
    pub price: f64,
    /// El cierre anterior.
    pub previous: f64,
    /// El día de la cotización.
    pub date: NaiveDate,
    /// La hora de la cotización, si se conoce.
    pub time: Option<NaiveTime>,
}

impl Quote {
    /// La variación frente al cierre anterior, en porcentaje, o 0 si no hay cierre anterior.
    pub fn change_percent(&self) -> f64 {
        if self.previous == 0.0 {
            0.0
        } else {
            (self.price - self.previous) / self.previous * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::date;

    fn quote(price: f64, previous: f64) -> Quote {
        Quote {
            price,
            previous,
            date: date(2026, 11, 13),
            time: None,
        }
    }

    #[test]
    fn rn_32_names_are_fixed_and_in_order() {
        assert_eq!(Market::ALL.map(Market::name), ["IBEX 35", "Brent ($)"]);
    }

    #[test]
    fn rn_32_the_change_is_against_the_previous_close() {
        [
            ("up", quote(110.0, 100.0), 10.0),
            ("down", quote(95.0, 100.0), -5.0),
            ("unchanged", quote(100.0, 100.0), 0.0),
            ("without a previous close", quote(100.0, 0.0), 0.0),
        ]
        .into_iter()
        .for_each(|(case, quote, expected)| {
            assert!(
                (quote.change_percent() - expected).abs() < 1e-9,
                "{case}: {}",
                quote.change_percent()
            );
        });
    }
}
