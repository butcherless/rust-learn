//! El contrato entre las apps del escrutinio (requisitos de escrutinio, «Backend»): los mensajes de
//! Kafka, la validación de una mesa y el snapshot que el servicio sirve a la TUI.
//!
//! Puro, sin E/S: solo tipos con serde y reglas. Los campos del JSON están en inglés, como el código.

pub mod districts;

use std::fmt;

use serde::{Deserialize, Serialize};

/// Una mesa escrutada, tal como viaja en `escrutinio.mesas`. La clave del mensaje es `province`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StationMessage {
    pub province: String,
    pub municipality: String,
    pub district: String,
    pub section: String,
    pub table: String,
    pub census: u32,
    pub blank: u32,
    pub invalid: u32,
    pub votes: Vec<VoteMessage>,
}

/// Los votos de una candidatura en una mesa, con sus siglas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoteMessage {
    pub candidacy: u32,
    pub acronym: String,
    pub votes: u32,
}

impl StationMessage {
    /// La clave del mensaje: el código de provincia, para que sus mesas vayan a la misma partición.
    pub fn key(&self) -> &str {
        &self.province
    }

    /// La mesa como texto: `provincia-municipio-distrito-sección-mesa`.
    pub fn id(&self) -> String {
        format!(
            "{}-{}-{}-{}-{}",
            self.province, self.municipality, self.district, self.section, self.table
        )
    }

    pub fn candidacy_votes(&self) -> u32 {
        self.votes.iter().map(|v| v.votes).sum()
    }

    /// Votantes = blancos + nulos + votos a candidaturas.
    pub fn voters(&self) -> u32 {
        self.blank + self.invalid + self.candidacy_votes()
    }
}

/// Por qué una mesa va al dead letter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// Más votantes que censo de escrutinio (4 mesas de León en 2023).
    VotersOverCensus { voters: u32, census: u32 },
    /// De una provincia que no está entre las 52 circunscripciones.
    UnknownProvince { province: String },
    /// El mensaje no es una mesa válida en JSON.
    Malformed { detail: String },
}

impl Rejection {
    /// El código del motivo en el mensaje del dead letter.
    pub fn code(&self) -> &'static str {
        match self {
            Rejection::VotersOverCensus { .. } => "voters_over_census",
            Rejection::UnknownProvince { .. } => "unknown_province",
            Rejection::Malformed { .. } => "malformed",
        }
    }
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rejection::VotersOverCensus { voters, census } => f.pad(&format!(
                "{voters} votantes y {census} de censo de escrutinio"
            )),
            Rejection::UnknownProvince { province } => {
                f.pad(&format!("la provincia {province} no existe"))
            }
            Rejection::Malformed { detail } => f.pad(&format!("mensaje no válido: {detail}")),
        }
    }
}

/// La regla de los votantes: no pueden pasar del censo de escrutinio.
pub fn check_voters(voters: u32, census: u32) -> Result<(), Rejection> {
    if voters > census {
        Err(Rejection::VotersOverCensus { voters, census })
    } else {
        Ok(())
    }
}

/// `Ok` si la mesa se puede contar; si no, el motivo.
pub fn validate(station: &StationMessage) -> Result<(), Rejection> {
    check_voters(station.voters(), station.census)
}

/// Un mensaje de `escrutinio.mesas.dlq`: el motivo, su texto y la mesa o, si no se pudo leer, el
/// mensaje tal cual.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeadLetterMessage {
    pub reason: String,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub station: Option<StationMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
}

impl DeadLetterMessage {
    /// Una mesa que no pasó la validación.
    pub fn rejected(station: StationMessage, rejection: &Rejection) -> Self {
        Self {
            reason: rejection.code().to_owned(),
            detail: rejection.to_string(),
            station: Some(station),
            raw: None,
        }
    }

    /// Un mensaje que no se pudo leer como mesa.
    pub fn malformed(raw: &[u8], detail: String) -> Self {
        let rejection = Rejection::Malformed { detail };
        Self {
            reason: rejection.code().to_owned(),
            detail: rejection.to_string(),
            station: None,
            raw: Some(String::from_utf8_lossy(raw).into_owned()),
        }
    }
}

/// Lo que sirve `GET /snapshots`: las 52 circunscripciones y el total de España. Todo está
/// calculado: quien lo pinta no suma ni compara nada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshots {
    /// En el orden de la tabla (RN-03).
    pub provinces: Vec<ProvinceSnapshot>,
    pub total: Total,
}

/// El piloto de una circunscripción o del total (RN-12): qué pasó con su escrutado hace poco. Lo
/// decide el servicio, igual para todos los clientes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pulse {
    /// Aún sin mesas escrutadas.
    #[default]
    NoData,
    /// Recibió una mesa en los últimos segundos.
    Updated,
    /// Con mesas, pero ninguna reciente.
    Steady,
    /// Con todas sus mesas posibles escrutadas.
    Complete,
}

/// Las mesas del total de España. `stations` de `possible` es el N'/N' que se pinta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Total {
    /// Mesas escrutadas, válidas.
    pub stations: u32,
    /// Las que se pueden escrutar: todas menos las rechazadas.
    pub possible: u32,
    /// Las rechazadas, que van al dead letter.
    pub rejected: u32,
    /// Se escrutaron todas las posibles.
    pub complete: bool,
    /// Votantes de las mesas escrutadas.
    pub voters: u64,
    /// El censo de escrutinio de las 52 circunscripciones (RN-10).
    pub electorate: u64,
    /// Los votos que se estiman en España (RN-04).
    pub estimated: u64,
    pub pulse: Pulse,
}

/// El acumulado de una circunscripción: siempre están las 52, también sin mesas, en el orden de la
/// tabla (RN-03).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvinceSnapshot {
    /// El código de provincia del INE.
    pub province: String,
    /// El nombre oficial: `A Coruña`.
    pub name: String,
    /// Mesas escrutadas, válidas.
    pub stations: u32,
    /// Las que se pueden escrutar: todas las de la circunscripción menos las rechazadas.
    pub possible: u32,
    /// Las rechazadas, que van al dead letter.
    pub rejected: u32,
    /// Se escrutaron todas las posibles.
    pub complete: bool,
    /// Censo de las mesas escrutadas.
    pub census: u64,
    /// El censo de escrutinio de la circunscripción entera (RN-10).
    pub electorate: u64,
    /// Los votos que se estiman en ella (RN-04).
    pub estimated: u64,
    pub voters: u64,
    pub blank: u64,
    pub invalid: u64,
    pub pulse: Pulse,
    /// De más a menos votos.
    pub votes: Vec<CandidacyTotal>,
}

/// Los votos de una candidatura en una circunscripción.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidacyTotal {
    pub candidacy: u32,
    pub acronym: String,
    pub votes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn station(census: u32) -> StationMessage {
        StationMessage {
            province: "51".to_owned(),
            municipality: "001".to_owned(),
            district: "01".to_owned(),
            section: "001".to_owned(),
            table: "A".to_owned(),
            census,
            blank: 1,
            invalid: 2,
            votes: vec![
                VoteMessage {
                    candidacy: 1,
                    acronym: "AAA".to_owned(),
                    votes: 30,
                },
                VoteMessage {
                    candidacy: 2,
                    acronym: "BBB".to_owned(),
                    votes: 20,
                },
            ],
        }
    }

    const STATION_JSON: &str = r#"{"province":"51","municipality":"001","district":"01","section":"001","table":"A","census":100,"blank":1,"invalid":2,"votes":[{"candidacy":1,"acronym":"AAA","votes":30},{"candidacy":2,"acronym":"BBB","votes":20}]}"#;

    #[test]
    fn the_station_message_is_this_json_both_ways() {
        assert_eq!(
            serde_json::to_string(&station(100)).ok().as_deref(),
            Some(STATION_JSON)
        );
        assert_eq!(
            serde_json::from_str::<StationMessage>(STATION_JSON).ok(),
            Some(station(100))
        );
    }

    #[test]
    fn a_station_has_its_key_id_and_voters() {
        let station = station(100);
        assert_eq!(station.key(), "51");
        assert_eq!(station.id(), "51-001-01-001-A");
        assert_eq!(station.candidacy_votes(), 50);
        assert_eq!(station.voters(), 53);
    }

    #[test]
    fn validates_that_voters_fit_in_the_census() {
        [
            (100, Ok(())),
            (53, Ok(())),
            (
                52,
                Err(Rejection::VotersOverCensus {
                    voters: 53,
                    census: 52,
                }),
            ),
        ]
        .into_iter()
        .for_each(|(census, expected)| {
            assert_eq!(validate(&station(census)), expected, "{census}")
        });
    }

    #[test]
    fn rejections_have_a_code_and_a_text() {
        [
            (
                Rejection::VotersOverCensus {
                    voters: 53,
                    census: 52,
                },
                "voters_over_census",
                "53 votantes y 52 de censo de escrutinio",
            ),
            (
                Rejection::UnknownProvince {
                    province: "99".to_owned(),
                },
                "unknown_province",
                "la provincia 99 no existe",
            ),
            (
                Rejection::Malformed {
                    detail: "EOF".to_owned(),
                },
                "malformed",
                "mensaje no válido: EOF",
            ),
        ]
        .into_iter()
        .for_each(|(rejection, code, text)| {
            assert_eq!(
                (rejection.code(), rejection.to_string().as_str()),
                (code, text)
            );
        });
    }

    #[test]
    fn the_dead_letter_carries_the_station_or_the_raw_message() {
        let rejection = Rejection::VotersOverCensus {
            voters: 53,
            census: 52,
        };
        let rejected = serde_json::to_string(&DeadLetterMessage::rejected(station(52), &rejection));
        assert_eq!(
            rejected.ok(),
            Some(format!(
                r#"{{"reason":"voters_over_census","detail":"53 votantes y 52 de censo de escrutinio","station":{}}}"#,
                STATION_JSON.replace("\"census\":100", "\"census\":52")
            ))
        );
        let malformed = DeadLetterMessage::malformed(b"{oops", "EOF".to_owned());
        assert_eq!(
            serde_json::to_string(&malformed).ok().as_deref(),
            Some(r#"{"reason":"malformed","detail":"mensaje no válido: EOF","raw":"{oops"}"#)
        );
        let back = serde_json::to_string(&malformed)
            .ok()
            .and_then(|json| serde_json::from_str::<DeadLetterMessage>(&json).ok());
        assert_eq!(back, Some(malformed));
    }

    #[test]
    fn the_pulse_is_written_in_snake_case() {
        [
            (Pulse::NoData, r#""no_data""#),
            (Pulse::Updated, r#""updated""#),
            (Pulse::Steady, r#""steady""#),
            (Pulse::Complete, r#""complete""#),
        ]
        .into_iter()
        .for_each(|(pulse, json)| {
            assert_eq!(serde_json::to_string(&pulse).ok().as_deref(), Some(json));
        });
        assert_eq!(Pulse::default(), Pulse::NoData);
    }

    #[test]
    fn the_snapshots_are_this_json() {
        let province = ProvinceSnapshot {
            province: "51".to_owned(),
            name: "Ceuta".to_owned(),
            stations: 2,
            possible: 3,
            rejected: 1,
            complete: false,
            census: 200,
            electorate: 300,
            estimated: 210,
            voters: 106,
            blank: 2,
            invalid: 4,
            pulse: Pulse::Updated,
            votes: vec![CandidacyTotal {
                candidacy: 1,
                acronym: "AAA".to_owned(),
                votes: 60,
            }],
        };
        let snapshots = Snapshots {
            total: Total {
                stations: 2,
                possible: 3,
                rejected: 1,
                complete: false,
                voters: 106,
                electorate: 300,
                estimated: 210,
                pulse: Pulse::Steady,
            },
            provinces: vec![province],
        };
        let json = r#"{"provinces":[{"province":"51","name":"Ceuta","stations":2,"possible":3,"rejected":1,"complete":false,"census":200,"electorate":300,"estimated":210,"voters":106,"blank":2,"invalid":4,"pulse":"updated","votes":[{"candidacy":1,"acronym":"AAA","votes":60}]}],"total":{"stations":2,"possible":3,"rejected":1,"complete":false,"voters":106,"electorate":300,"estimated":210,"pulse":"steady"}}"#;
        assert_eq!(
            serde_json::to_string(&snapshots).ok().as_deref(),
            Some(json)
        );
        assert_eq!(
            serde_json::from_str::<Snapshots>(json).ok(),
            Some(snapshots)
        );
    }
}
