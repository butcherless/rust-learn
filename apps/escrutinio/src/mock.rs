//! Un escrutinio simulado, hasta que exista el backend (RN-06).
//!
//! `Scrutiny::start` fija un resultado final para cada circunscripción, entre el 85 % y el 115 %
//! de su estimado, y `Scrutiny::advance` añade en cada paso entre el 0 % y el 6 % de ese final,
//! como si informaran unas cuantas mesas. Con la misma semilla, el escrutinio es el mismo.

use crate::census::District;
use crate::domain::{Progress, Pulse, Tally};

/// El resultado final, como fracción del estimado: entre `1 - FINAL_SPREAD` y `1 + FINAL_SPREAD`.
const FINAL_SPREAD: f64 = 0.15;
/// Lo máximo que avanza una circunscripción en un paso, como fracción de su final.
const MAX_STEP: f64 = 0.06;

/// Un generador pseudoaleatorio xorshift64: cada llamada devuelve el valor y el estado siguiente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rng(u64);

impl Rng {
    /// xorshift no admite el estado 0, así que se cambia por 1.
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    /// Un número en `[0, 1)` y el generador siguiente.
    pub fn unit(self) -> (f64, Self) {
        let x = self.0 ^ (self.0 << 13);
        let x = x ^ (x >> 7);
        let x = x ^ (x << 17);
        ((x >> 11) as f64 / (1_u64 << 53) as f64, Self(x))
    }
}

/// Una fila del escrutinio: la circunscripción, su recuento, su piloto y el final simulado.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Row {
    pub district: District,
    pub tally: Tally,
    /// Si su escrutado cambió en la última actualización (RN-12).
    pub pulse: Pulse,
    /// Los votos con los que acabará el escrutinio simulado.
    pub target: u64,
}

/// El estado del escrutinio simulado.
#[derive(Debug, Clone, PartialEq)]
pub struct Scrutiny {
    rows: Vec<Row>,
    turnout: u8,
    rng: Rng,
}

impl Scrutiny {
    /// Un escrutinio sin votos, en el orden de `districts`, con el estimado al `turnout` %.
    pub fn start(districts: &[District], turnout: u8, seed: u64) -> Self {
        let (rows, rng) =
            districts
                .iter()
                .fold((Vec::new(), Rng::new(seed)), |(rows, rng), &district| {
                    let tally = Tally::new(district.census, turnout);
                    let (unit, rng) = rng.unit();
                    let factor = 1.0 - FINAL_SPREAD + 2.0 * FINAL_SPREAD * unit;
                    let target =
                        ((tally.estimated as f64 * factor).round() as u64).min(tally.census);
                    (
                        push(
                            rows,
                            Row {
                                district,
                                tally,
                                pulse: Pulse::NoData,
                                target,
                            },
                        ),
                        rng,
                    )
                });
        Self { rows, turnout, rng }
    }

    /// Un paso más: cada circunscripción que no ha acabado suma al menos un voto, sin pasar de su
    /// final.
    pub fn advance(self) -> Self {
        let (rows, rng) = self
            .rows
            .into_iter()
            .fold((Vec::new(), self.rng), |(rows, rng), row| {
                let (unit, rng) = rng.unit();
                let step = ((row.target as f64 * MAX_STEP * unit).round() as u64).max(1);
                let counted = (row.tally.counted + step).min(row.target);
                let row = Row {
                    tally: row.tally.with_counted(counted),
                    pulse: Pulse::between(row.tally.counted, counted, counted >= row.target),
                    ..row
                };
                (push(rows, row), rng)
            });
        Self { rows, rng, ..self }
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// La participación estimada, en % del censo.
    pub fn turnout(&self) -> u8 {
        self.turnout
    }

    /// La suma de todas las filas (RN-07).
    pub fn total(&self) -> Tally {
        Tally::total(self.rows.iter().map(|row| &row.tally))
    }

    /// El piloto del total (RN-12).
    pub fn pulse(&self) -> Pulse {
        Pulse::total(self.rows.iter().map(|row| row.pulse))
    }

    /// Las mismas filas con lo escrutado de `counted`, por código de provincia; las que no
    /// aparecen, a 0. Para seguir al servicio (RN-11), que manda siempre el acumulado. Una fila
    /// está completa con todas sus mesas (RN-12).
    pub fn with_counted(self, counted: &std::collections::BTreeMap<String, Progress>) -> Self {
        let rows = self
            .rows
            .into_iter()
            .map(|row| {
                let progress = counted.get(row.district.code).copied().unwrap_or_default();
                let tally = row.tally.with_counted(progress.voters);
                let complete = progress.stations >= row.district.stations;
                Row {
                    pulse: Pulse::between(row.tally.counted, tally.counted, complete),
                    tally,
                    ..row
                }
            })
            .collect();
        Self { rows, ..self }
    }

    /// Si todas las circunscripciones han llegado a su final.
    pub fn finished(&self) -> bool {
        self.rows.iter().all(|row| row.tally.counted >= row.target)
    }
}

fn push<T>(items: Vec<T>, item: T) -> Vec<T> {
    items.into_iter().chain(std::iter::once(item)).collect()
}

#[cfg(test)]
pub mod fixtures {
    use super::*;
    use crate::census::{Election, ordered};
    use crate::domain::DEFAULT_TURNOUT;

    pub const SEED: u64 = 2026;

    /// El escrutinio de todas las circunscripciones, sin votos.
    pub fn scrutiny() -> Scrutiny {
        Scrutiny::start(&ordered(Election::Generales2026), DEFAULT_TURNOUT, SEED)
    }

    /// `scrutiny` tras `steps` pasos.
    pub fn advanced(steps: usize) -> Scrutiny {
        (0..steps).fold(scrutiny(), |scrutiny, _| scrutiny.advance())
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{SEED, advanced, scrutiny};
    use super::*;
    use crate::census::{Election, TOTAL_CENSUS};

    #[test]
    fn the_generator_stays_in_the_unit_interval_and_never_sticks_at_zero() {
        let (values, _) = (0..1_000).fold((Vec::new(), Rng::new(0)), |(values, rng), _| {
            let (value, rng) = rng.unit();
            (push(values, value), rng)
        });
        assert!(values.iter().all(|value| (0.0..1.0).contains(value)));
        assert!(values.iter().any(|&value| value > 0.5));
    }

    #[test]
    fn starts_with_no_votes_and_the_estimate_at_the_turnout() {
        let scrutiny = scrutiny();
        let total = scrutiny.total();
        assert_eq!(total.census, TOTAL_CENSUS);
        assert_eq!(total.counted, 0);
        assert!(
            (24_900_000..=25_200_000).contains(&total.estimated),
            "{total:?}"
        );
        assert_eq!(scrutiny.turnout(), 70);
        assert!(!scrutiny.finished());
    }

    #[test]
    fn every_final_is_within_the_spread_of_its_estimate() {
        scrutiny().rows().iter().for_each(|row| {
            let estimated = row.tally.estimated as f64;
            let target = row.target as f64;
            assert!(
                target >= estimated * (1.0 - FINAL_SPREAD) - 1.0
                    && target <= estimated * (1.0 + FINAL_SPREAD) + 1.0,
                "{}",
                row.district.name
            );
        });
    }

    #[test]
    fn each_step_counts_more_votes_without_passing_the_final() {
        let before = advanced(3);
        let after = before.clone().advance();
        before
            .rows()
            .iter()
            .zip(after.rows())
            .for_each(|(before, after)| {
                assert!(
                    after.tally.counted > before.tally.counted,
                    "{}",
                    after.district.name
                );
                assert!(
                    after.tally.counted <= after.target,
                    "{}",
                    after.district.name
                );
            });
    }

    #[test]
    fn finishes_with_every_district_at_its_final() {
        let finished = advanced(200);
        assert!(finished.finished());
        assert!(
            finished
                .rows()
                .iter()
                .all(|row| row.tally.counted == row.target)
        );
        assert_eq!(finished.clone().advance().rows(), finished.rows());
    }

    #[test]
    fn takes_the_counted_votes_by_province_code() {
        let progress = |voters| Progress {
            voters,
            stations: 1,
        };
        let counted = [
            ("28".to_owned(), progress(1_000)),
            ("51".to_owned(), progress(10_000_000)),
        ]
        .into();
        let scrutiny = scrutiny().with_counted(&counted);
        let by_code = |code| {
            scrutiny
                .rows()
                .iter()
                .find(|row| row.district.code == code)
                .map(|row| (row.tally.counted, row.tally.census))
        };
        assert_eq!(by_code("28").map(|(counted, _)| counted), Some(1_000));
        // Nunca pasa del censo (RN-05), y lo que no llega queda a 0.
        assert_eq!(
            by_code("51").map(|(counted, census)| counted == census),
            Some(true)
        );
        assert_eq!(by_code("02").map(|(counted, _)| counted), Some(0));
    }

    #[test]
    fn pulses_follow_the_changes_of_each_update() {
        let pulse_of = |scrutiny: &Scrutiny, code| {
            scrutiny
                .rows()
                .iter()
                .find(|row| row.district.code == code)
                .map(|row| row.pulse)
        };
        assert_eq!(scrutiny().pulse(), Pulse::NoData);
        let first = scrutiny().with_counted(
            &[(
                "28".to_owned(),
                Progress {
                    voters: 1_000,
                    stations: 1,
                },
            )]
            .into(),
        );
        assert_eq!(pulse_of(&first, "28"), Some(Pulse::Updated));
        assert_eq!(pulse_of(&first, "51"), Some(Pulse::NoData));
        assert_eq!(first.pulse(), Pulse::Updated);
        let same = first.clone().with_counted(
            &[(
                "28".to_owned(),
                Progress {
                    voters: 1_000,
                    stations: 1,
                },
            )]
            .into(),
        );
        assert_eq!(pulse_of(&same, "28"), Some(Pulse::Steady));
        assert_eq!(same.pulse(), Pulse::Steady);
        let one_step = advanced(1);
        assert!(
            one_step
                .rows()
                .iter()
                .all(|row| row.pulse == Pulse::Updated)
        );
        let finished = advanced(200);
        assert!(
            finished
                .rows()
                .iter()
                .all(|row| row.pulse == Pulse::Complete)
        );
        assert_eq!(finished.pulse(), Pulse::Complete);
        let all_stations = first.with_counted(
            &[(
                "28".to_owned(),
                Progress {
                    voters: 2_000,
                    stations: 7_608,
                },
            )]
            .into(),
        );
        assert_eq!(pulse_of(&all_stations, "28"), Some(Pulse::Complete));
    }

    #[test]
    fn the_same_seed_gives_the_same_scrutiny() {
        let other = Scrutiny::start(&crate::census::ordered(Election::Generales2026), 70, SEED);
        assert_eq!(other.advance(), scrutiny().advance());
    }
}
