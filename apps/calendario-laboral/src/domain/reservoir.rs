//! Los embalses de la Comunidad de Madrid y su nivel (RN-31): el último dato semanal y la media
//! de la misma semana en los 10 años anteriores. Los datos llegan de `adapters::reservoirs`.

use chrono::{Datelike, NaiveDate};

/// Un embalse: su clave en la API y el nombre que se muestra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reservoir {
    /// La clave del embalse en la API (`atazar-el`).
    pub slug: &'static str,
    /// El nombre que se muestra.
    pub name: &'static str,
}

/// Los 13 embalses de la Comunidad de Madrid, de mayor a menor capacidad.
pub const MADRID_RESERVOIRS: [Reservoir; 13] = [
    reservoir("atazar-el", "El Atazar"),
    reservoir("san-juan", "San Juan"),
    reservoir("valmayor", "Valmayor"),
    reservoir("santillana", "Santillana"),
    reservoir("puentes-viejas-buitrago", "Puentes Viejas"),
    reservoir("riosequillo", "Riosequillo"),
    reservoir("pardo-el", "El Pardo"),
    reservoir("vellon-el-pedrezuela", "El Vellón"),
    reservoir("pinilla", "Pinilla"),
    reservoir("el-villar", "El Villar"),
    reservoir("picadas", "Picadas"),
    reservoir("navacerrada", "Navacerrada"),
    reservoir("jarosa-la", "La Jarosa"),
];

const fn reservoir(slug: &'static str, name: &'static str) -> Reservoir {
    Reservoir { slug, name }
}

/// Los años anteriores que entran en la media.
pub const AVERAGE_YEARS: i32 = 10;

/// Un dato semanal: la fecha, la capacidad y el volumen en hm³, y el porcentaje de llenado.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeeklyLevel {
    /// El día del dato.
    pub date: NaiveDate,
    /// La capacidad, en hm³.
    pub capacity_hm3: f64,
    /// El volumen, en hm³.
    pub volume_hm3: f64,
    /// El porcentaje de llenado.
    pub percent: f64,
}

/// El nivel de un embalse: el último dato y la media de 10 años, si hay datos de esa semana.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReservoirLevel {
    /// El dato más reciente.
    pub latest: WeeklyLevel,
    /// La media del porcentaje de la misma semana ISO en los [`AVERAGE_YEARS`] años anteriores.
    pub ten_year_percent: Option<f64>,
}

/// El nivel a partir del histórico, en cualquier orden, o nada si está vacío.
pub fn reservoir_level(history: &[WeeklyLevel]) -> Option<ReservoirLevel> {
    let latest = *history.iter().max_by_key(|level| level.date)?;
    let week = latest.date.iso_week();
    let years = week.year() - AVERAGE_YEARS..week.year();
    let (sum, count) = history
        .iter()
        .filter(|level| {
            let other = level.date.iso_week();
            other.week() == week.week() && years.contains(&other.year())
        })
        .fold((0.0, 0_u32), |(sum, count), level| {
            (sum + level.percent, count + 1)
        });
    Some(ReservoirLevel {
        latest,
        ten_year_percent: (count > 0).then(|| sum / f64::from(count)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::date;

    fn level(date: NaiveDate, percent: f64) -> WeeklyLevel {
        WeeklyLevel {
            date,
            capacity_hm3: 100.0,
            volume_hm3: percent,
            percent,
        }
    }

    #[test]
    fn rn_31_the_latest_level_and_the_mean_of_the_same_week_in_the_ten_previous_years() {
        // El 2026-11-10 es de la semana ISO 46; el 2025-11-11 y el 2016-11-15, también.
        let latest = level(date(2026, 11, 10), 63.0);
        [
            ("only the latest", vec![latest], None),
            (
                "same week of the previous years",
                vec![
                    level(date(2016, 11, 15), 40.0),
                    latest,
                    level(date(2025, 11, 11), 80.0),
                ],
                Some(60.0),
            ),
            (
                "other weeks and older years are left out",
                vec![
                    latest,
                    level(date(2025, 11, 4), 10.0),
                    level(date(2025, 11, 11), 80.0),
                    level(date(2015, 11, 10), 10.0),
                ],
                Some(80.0),
            ),
            (
                "earlier weeks of the same year are left out",
                vec![level(date(2026, 11, 3), 10.0), latest],
                None,
            ),
        ]
        .into_iter()
        .for_each(|(case, history, expected)| {
            assert_eq!(
                reservoir_level(&history),
                Some(ReservoirLevel {
                    latest,
                    ten_year_percent: expected
                }),
                "{case}"
            );
        });
        assert_eq!(reservoir_level(&[]), None);
    }

    #[test]
    fn rn_31_there_are_thirteen_reservoirs_with_distinct_slugs() {
        let mut slugs: Vec<&str> = MADRID_RESERVOIRS.iter().map(|r| r.slug).collect();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), 13);
        assert_eq!(MADRID_RESERVOIRS[0], reservoir("atazar-el", "El Atazar"));
    }
}
