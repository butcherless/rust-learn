//! Un escrutinio simulado, hasta que exista el backend (RN-06).
//!
//! `Scrutiny::start` fija un resultado final para cada circunscripción, entre el 85 % y el 115 %
//! de su estimado, y `Scrutiny::advance` añade en cada paso entre el 0 % y el 6 % de ese final,
//! como si informaran unas cuantas mesas. Con la misma semilla, el escrutinio es el mismo.

use crate::census::District;
use crate::domain::{Pulse, Stations, Tally, Update};

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
    /// Qué pasó con su escrutado hace poco (RN-12).
    pub pulse: Pulse,
    /// Las mesas N'/N' que dice el servicio (RN-13); la maqueta no tiene mesas.
    pub stations: Option<Stations>,
    /// Los votos con los que acabará el escrutinio simulado.
    pub target: u64,
}

/// El estado del escrutinio simulado.
#[derive(Debug, Clone, PartialEq)]
pub struct Scrutiny {
    rows: Vec<Row>,
    turnout: u8,
    rng: Rng,
    /// Las mesas del total de España que dice el servicio (RN-13).
    stations: Option<Stations>,
    /// Si el escrutinio está completo: lo dice el servicio y, en la maqueta, `advance`.
    complete: bool,
    /// El piloto del total que dice el servicio (RN-12); la maqueta lo deduce de sus filas.
    total_pulse: Option<Pulse>,
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
                                stations: None,
                                target,
                            },
                        ),
                        rng,
                    )
                });
        Self {
            rows,
            turnout,
            rng,
            stations: None,
            complete: false,
            total_pulse: None,
        }
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
        let complete = rows.iter().all(|row| row.tally.counted >= row.target);
        Self {
            rows,
            rng,
            complete,
            ..self
        }
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

    /// El piloto del total (RN-12): el que dice el servicio; en la maqueta, verde si el escrutinio
    /// está completo y, si no, según sus filas.
    pub fn pulse(&self) -> Pulse {
        self.total_pulse.unwrap_or_else(|| {
            if self.complete {
                Pulse::Complete
            } else {
                Pulse::total(self.rows.iter().map(|row| row.pulse))
            }
        })
    }

    /// Las mesas del total de España, N'/N', si las dice el servicio (RN-13).
    pub fn stations(&self) -> Option<Stations> {
        self.stations
    }

    /// Las mismas filas con lo que dice el servicio (RN-11, RN-12, RN-13): sus votantes, sus mesas,
    /// si están completas y su piloto, por código de provincia, y lo mismo del total. Las filas que
    /// no aparecen se quedan sin votos. El servicio manda siempre el acumulado y aquí no se calcula
    /// ni se compara nada.
    pub fn with_update(self, update: &Update) -> Self {
        let rows = self
            .rows
            .into_iter()
            .map(|row| {
                let progress = update.provinces.get(row.district.code).copied();
                let tally = row
                    .tally
                    .with_counted(progress.map_or(0, |progress| progress.voters));
                Row {
                    pulse: progress.map_or(Pulse::NoData, |progress| progress.pulse),
                    stations: progress.map(|progress| progress.stations),
                    tally,
                    ..row
                }
            })
            .collect();
        Self {
            rows,
            stations: Some(update.total),
            complete: update.complete,
            total_pulse: Some(update.pulse),
            ..self
        }
    }

    /// Si el escrutinio está completo.
    pub fn finished(&self) -> bool {
        self.complete
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
    use crate::domain::Progress;

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

    /// Una respuesta del servicio con estas circunscripciones: (provincia, votantes, mesas, posibles,
    /// completa, piloto), y el piloto del total.
    fn update(
        provinces: &[(&str, u64, u32, u32, bool, Pulse)],
        total: Stations,
        complete: bool,
        pulse: Pulse,
    ) -> Update {
        Update {
            provinces: provinces
                .iter()
                .map(|&(code, voters, counted, possible, complete, pulse)| {
                    let stations = Stations { counted, possible };
                    (
                        code.to_owned(),
                        Progress {
                            voters,
                            stations,
                            complete,
                            pulse,
                        },
                    )
                })
                .collect(),
            total,
            complete,
            pulse,
        }
    }

    fn row<'a>(scrutiny: &'a Scrutiny, code: &str) -> Option<&'a Row> {
        scrutiny.rows().iter().find(|row| row.district.code == code)
    }

    #[test]
    fn takes_what_the_service_says_for_each_district_and_the_total() {
        let total = Stations {
            counted: 10,
            possible: 20,
        };
        let scrutiny = scrutiny().with_update(&update(
            &[
                ("28", 1_000, 7, 9, false, Pulse::Updated),
                ("51", 10_000_000, 3, 3, true, Pulse::Complete),
            ],
            total,
            false,
            Pulse::Updated,
        ));
        assert_eq!(
            row(&scrutiny, "28").map(|row| (row.tally.counted, row.stations)),
            Some((
                1_000,
                Some(Stations {
                    counted: 7,
                    possible: 9
                })
            ))
        );
        // Nunca pasa del censo (RN-05), y lo que no llega queda sin votos ni mesas.
        assert_eq!(
            row(&scrutiny, "51").map(|row| row.tally.counted == row.tally.census),
            Some(true)
        );
        assert_eq!(
            row(&scrutiny, "02").map(|row| (row.tally.counted, row.stations)),
            Some((0, None))
        );
        assert_eq!(scrutiny.stations(), Some(total));
        assert!(!scrutiny.finished());
        // La maqueta no tiene mesas.
        assert_eq!(super::fixtures::scrutiny().stations(), None);
    }

    #[test]
    fn the_pulses_are_the_ones_the_service_says_without_comparing_anything() {
        let total = Stations::default();
        let pulse_of = |scrutiny: &Scrutiny, code| row(scrutiny, code).map(|row| row.pulse);
        assert_eq!(scrutiny().pulse(), Pulse::NoData);
        let first = scrutiny().with_update(&update(
            &[("28", 1_000, 1, 2, false, Pulse::Updated)],
            total,
            false,
            Pulse::Updated,
        ));
        assert_eq!(pulse_of(&first, "28"), Some(Pulse::Updated));
        assert_eq!(pulse_of(&first, "51"), Some(Pulse::NoData));
        assert_eq!(first.pulse(), Pulse::Updated);
        // El mismo escrutado: el piloto es el que dice el servicio, no el que saldría al comparar.
        let again = first.clone().with_update(&update(
            &[("28", 1_000, 1, 2, false, Pulse::Steady)],
            total,
            false,
            Pulse::Steady,
        ));
        assert_eq!(pulse_of(&again, "28"), Some(Pulse::Steady));
        assert_eq!(again.pulse(), Pulse::Steady);
        let also = first.with_update(&update(
            &[("28", 2_000, 2, 2, true, Pulse::Complete)],
            total,
            false,
            Pulse::Updated,
        ));
        assert_eq!(pulse_of(&also, "28"), Some(Pulse::Complete));
        assert_eq!(also.pulse(), Pulse::Updated);
        let all = also.with_update(&update(
            &[("28", 2_000, 2, 2, true, Pulse::Complete)],
            total,
            true,
            Pulse::Complete,
        ));
        assert_eq!(all.pulse(), Pulse::Complete);
        assert!(all.finished());
    }

    #[test]
    fn the_mock_pulses_follow_its_steps_and_end_complete() {
        let one_step = advanced(1);
        assert!(
            one_step
                .rows()
                .iter()
                .all(|row| row.pulse == Pulse::Updated)
        );
        assert_eq!(one_step.pulse(), Pulse::Updated);
        let finished = advanced(200);
        assert!(
            finished
                .rows()
                .iter()
                .all(|row| row.pulse == Pulse::Complete)
        );
        assert_eq!(finished.pulse(), Pulse::Complete);
    }

    #[test]
    fn the_same_seed_gives_the_same_scrutiny() {
        let other = Scrutiny::start(&crate::census::ordered(Election::Generales2026), 70, SEED);
        assert_eq!(other.advance(), scrutiny().advance());
    }
}
