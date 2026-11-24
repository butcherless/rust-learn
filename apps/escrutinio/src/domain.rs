//! Los recuentos y su formato.
//!
//! - `Tally` guarda el censo, el estimado (RN-04) y el escrutado (RN-05) de una circunscripción;
//!   `Tally::total` suma varios (RN-07).
//! - `sort_key` ordena los nombres sin tildes ni mayúsculas, con la `ñ` detrás de la `n` (RN-03).
//! - `thousands` y `percent` dan los números en formato español (RN-08).
//! - `Pulse` es el piloto de cada fila: si su escrutado cambió en la última actualización (RN-12).
//! - `Stations`, `Progress` y `Update` son lo que llega del servicio, ya calculado: la TUI lo pinta
//!   sin sumar ni comparar mesas (RN-13).

/// La participación estimada por defecto, en % del censo (RN-04).
pub const DEFAULT_TURNOUT: u8 = 70;

/// Los votos de una circunscripción, o de varias sumadas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tally {
    pub census: u64,
    pub estimated: u64,
    pub counted: u64,
}

impl Tally {
    /// Un recuento sin votos escrutados, con el estimado al `turnout` % del censo, redondeado
    /// hacia abajo.
    pub fn new(census: u64, turnout: u8) -> Self {
        Self {
            census,
            estimated: census * u64::from(turnout) / 100,
            counted: 0,
        }
    }

    /// El mismo recuento con `counted` votos escrutados, sin pasar del censo (RN-05).
    pub fn with_counted(self, counted: u64) -> Self {
        Self {
            counted: counted.min(self.census),
            ..self
        }
    }

    /// Los votos escrutados, en % del censo; 0 sin censo.
    pub fn turnout(&self) -> f64 {
        ratio(self.counted, self.census) * 100.0
    }

    /// La suma de varios recuentos (RN-07).
    pub fn total<'a>(tallies: impl IntoIterator<Item = &'a Tally>) -> Self {
        tallies
            .into_iter()
            .fold(Self::default(), |sum, tally| Self {
                census: sum.census + tally.census,
                estimated: sum.estimated + tally.estimated,
                counted: sum.counted + tally.counted,
            })
    }
}

/// Las mesas escrutadas, N', de las posibles, N (RN-13). N' ≤ N: las rechazadas no cuentan en N.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stations {
    pub counted: u32,
    pub possible: u32,
}

/// Lo escrutado de una circunscripción según el servicio (RN-11, RN-13): votantes, mesas y si están
/// todas las posibles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Progress {
    pub voters: u64,
    pub stations: Stations,
    pub complete: bool,
}

/// Una respuesta del servicio, ya calculada: lo de cada circunscripción por código de provincia y
/// las mesas del total de España con su «completo».
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Update {
    pub provinces: std::collections::BTreeMap<String, Progress>,
    pub total: Stations,
    pub complete: bool,
}

/// El piloto de una fila (RN-12): qué pasó con su escrutado en la última actualización.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pulse {
    /// Aún sin votos escrutados.
    #[default]
    NoData,
    /// Cambió respecto a la actualización anterior.
    Updated,
    /// Igual que en la actualización anterior.
    Steady,
    /// Con todas sus mesas escrutadas (en la maqueta, en su resultado final).
    Complete,
}

impl Pulse {
    /// El piloto de un escrutado que pasa de `before` a `after`; `complete` manda sobre todo.
    pub fn between(before: u64, after: u64, complete: bool) -> Self {
        if complete {
            Self::Complete
        } else if after == 0 {
            Self::NoData
        } else if after == before {
            Self::Steady
        } else {
            Self::Updated
        }
    }

    /// El piloto del total mientras no esté completo: actualizado si alguna fila lo está, sin datos
    /// si ninguna tiene votos y sin cambios en el resto de casos. Que esté completo lo dice el
    /// servicio, no se deduce de las filas.
    pub fn total(pulses: impl IntoIterator<Item = Pulse>) -> Self {
        let pulses: Vec<Pulse> = pulses.into_iter().collect();
        let any = |wanted: Pulse| pulses.contains(&wanted);
        if any(Self::Updated) {
            Self::Updated
        } else if any(Self::Steady) || any(Self::Complete) {
            Self::Steady
        } else {
            Self::NoData
        }
    }
}

/// `part / whole`, o 0 si `whole` es 0.
pub fn ratio(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64
    }
}

/// La clave de orden de un nombre: en minúsculas y sin tildes, con la `ñ` detrás de la `n`.
pub fn sort_key(name: &str) -> String {
    name.chars()
        .flat_map(char::to_lowercase)
        .map(|letter| match letter {
            'á' | 'à' => "a".to_owned(),
            'é' | 'è' => "e".to_owned(),
            'í' | 'ï' => "i".to_owned(),
            'ó' | 'ò' => "o".to_owned(),
            'ú' | 'ü' => "u".to_owned(),
            // `~` va detrás de todas las letras ASCII.
            'ñ' => "n~".to_owned(),
            other => other.to_string(),
        })
        .collect()
}

/// Un entero con puntos de millar: `35.953.542`.
pub fn thousands(value: u64) -> String {
    let digits = value.to_string();
    let first = digits.len() % 3;
    let groups = std::iter::once(&digits[..first])
        .filter(|group| !group.is_empty())
        .chain(
            digits.as_bytes()[first..]
                .chunks(3)
                .filter_map(|chunk| std::str::from_utf8(chunk).ok()),
        );
    groups.collect::<Vec<_>>().join(".")
}

/// Un porcentaje con un decimal y coma: `70,0 %`.
pub fn percent(value: f64) -> String {
    format!("{value:.1} %").replace('.', ",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_estimate_is_the_turnout_of_the_census_rounded_down() {
        [
            (2_000_000, 70, 1_400_000),
            (68_010, 70, 47_607),
            (99, 50, 49),
        ]
        .into_iter()
        .for_each(|(census, turnout, estimated)| {
            assert_eq!(
                Tally::new(census, turnout).estimated,
                estimated,
                "{census} at {turnout} %"
            );
        });
    }

    #[test]
    fn the_counted_votes_never_exceed_the_census() {
        let tally = Tally::new(1_000, 70);
        assert_eq!(tally.with_counted(800).counted, 800);
        assert_eq!(tally.with_counted(1_200).counted, 1_000);
    }

    #[test]
    fn turnout_is_the_counted_share_of_the_census() {
        assert!((Tally::new(2_000, 70).with_counted(500).turnout() - 25.0).abs() < 1e-9);
        assert!(Tally::default().turnout().abs() < 1e-9);
    }

    #[test]
    fn the_total_adds_every_field() {
        let tallies = [
            Tally::new(1_000, 70).with_counted(100),
            Tally::new(3_000, 70).with_counted(200),
        ];
        assert_eq!(
            Tally::total(&tallies),
            Tally {
                census: 4_000,
                estimated: 2_800,
                counted: 300,
            }
        );
    }

    #[test]
    fn sort_keys_ignore_case_and_accents_and_put_n_tilde_after_n() {
        [
            ("Ávila", "avila"),
            ("Araba/Álava", "araba/alava"),
            ("Cádiz", "cadiz"),
            ("A Coruña", "a corun~a"),
        ]
        .into_iter()
        .for_each(|(name, key)| assert_eq!(sort_key(name), key, "{name}"));
        assert!(sort_key("Coruña") > sort_key("Corunz"));
    }

    #[test]
    fn groups_thousands_with_dots() {
        [
            (0, "0"),
            (999, "999"),
            (1_000, "1.000"),
            (68_010, "68.010"),
            (35_953_542, "35.953.542"),
        ]
        .into_iter()
        .for_each(|(value, text)| assert_eq!(thousands(value), text, "{value}"));
    }

    #[test]
    fn percents_have_one_decimal_and_a_comma() {
        assert_eq!(percent(70.0), "70,0 %");
        assert_eq!(percent(12.345), "12,3 %");
    }

    #[test]
    fn the_pulse_says_whether_the_count_changed() {
        [
            (0, 0, false, Pulse::NoData),
            (5, 0, false, Pulse::NoData),
            (0, 5, false, Pulse::Updated),
            (5, 7, false, Pulse::Updated),
            (7, 7, false, Pulse::Steady),
            (5, 7, true, Pulse::Complete),
            (7, 7, true, Pulse::Complete),
        ]
        .into_iter()
        .for_each(|(before, after, complete, pulse)| {
            let case = format!("{before} → {after}, {complete}");
            assert_eq!(Pulse::between(before, after, complete), pulse, "{case}");
        });
        assert_eq!(Pulse::default(), Pulse::NoData);
    }

    #[test]
    fn the_total_pulse_never_decides_that_it_is_complete() {
        use Pulse::{Complete, NoData, Steady, Updated};
        [
            (vec![], NoData),
            (vec![NoData, NoData], NoData),
            (vec![NoData, Steady], Steady),
            (vec![Steady, Updated, NoData], Updated),
            (vec![Complete, Complete], Steady),
            (vec![Complete, NoData], Steady),
            (vec![Complete, Updated], Updated),
        ]
        .into_iter()
        .for_each(|(pulses, total)| assert_eq!(Pulse::total(pulses.clone()), total, "{pulses:?}"));
    }
}
