//! La validación de cada mesa antes de publicarla (requisitos de escrutinio, «Simulador»).
//!
//! A diferencia de las comprobaciones de `station`, que rechazan el fichero entero, una mesa que no
//! pasa la validación no detiene nada: va al dead letter, con su motivo, y no se cuenta. En 2023 son
//! 4 mesas de León con más votantes que censo de escrutinio.

use std::fmt;

use super::station::Station;

/// Por qué una mesa va al dead letter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    VotersOverCensus { voters: u32, census: u32 },
}

impl Rejection {
    /// El código del motivo en el mensaje del dead letter.
    pub fn code(&self) -> &'static str {
        match self {
            Rejection::VotersOverCensus { .. } => "voters_over_census",
        }
    }
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rejection::VotersOverCensus { voters, census } => f.pad(&format!(
                "{voters} votantes y {census} de censo de escrutinio"
            )),
        }
    }
}

/// `Ok` si la mesa se puede contar; si no, el motivo.
pub fn validate(station: &Station) -> Result<(), Rejection> {
    let voters = station.voters();
    if voters > station.census {
        Err(Rejection::VotersOverCensus {
            voters,
            census: station.census,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::station::fixtures::dataset;

    #[test]
    fn accepts_stations_whose_voters_fit_in_the_census() {
        let station = &dataset().stations[0];
        [(53, Ok(())), (100, Ok(()))]
            .into_iter()
            .for_each(|(census, expected)| {
                let station = Station {
                    census,
                    ..station.clone()
                };
                assert_eq!(validate(&station), expected, "census {census}");
            });
    }

    #[test]
    fn rejects_more_voters_than_census() {
        let station = Station {
            census: 52,
            ..dataset().stations[0].clone()
        };
        let rejection = validate(&station);
        assert_eq!(
            rejection,
            Err(Rejection::VotersOverCensus {
                voters: 53,
                census: 52
            })
        );
        assert_eq!(
            rejection.map_err(|r| (r.code(), r.to_string())),
            Err((
                "voters_over_census",
                "53 votantes y 52 de censo de escrutinio".to_owned()
            ))
        );
    }
}
