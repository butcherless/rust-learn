//! Puro, sin E/S: qué hacer con un mensaje de `escrutinio.mesas` (requisitos de escrutinio,
//! «Servicio»). Se lee como mesa y se valida con el contrato; lo que no se puede leer, es de una
//! provincia que no existe o no pasa la validación va al dead letter. `reference` tiene las mesas de
//! cada provincia y `snapshot` calcula lo que sirve la API.

pub mod reference;
pub mod snapshot;

use escrutinio_contrato::{DeadLetterMessage, Rejection, StationMessage, validate};

/// Lo que se hace con un mensaje.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Incoming {
    /// Una mesa válida, para registrar.
    Station(StationMessage),
    /// Al dead letter, con la clave de su provincia si se pudo leer.
    DeadLetter {
        key: Option<String>,
        message: DeadLetterMessage,
    },
}

pub fn classify(payload: &[u8]) -> Incoming {
    match serde_json::from_slice::<StationMessage>(payload) {
        Err(error) => Incoming::DeadLetter {
            key: None,
            message: DeadLetterMessage::malformed(payload, error.to_string()),
        },
        Ok(station) => match known_province(&station).and_then(|()| validate(&station)) {
            Ok(()) => Incoming::Station(station),
            Err(rejection) => Incoming::DeadLetter {
                key: Some(station.province.clone()),
                message: DeadLetterMessage::rejected(station, &rejection),
            },
        },
    }
}

/// La provincia de la mesa tiene que ser una de las 52 circunscripciones.
fn known_province(station: &StationMessage) -> Result<(), Rejection> {
    if reference::expected(&station.province).is_some() {
        Ok(())
    } else {
        Err(Rejection::UnknownProvince {
            province: station.province.clone(),
        })
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use escrutinio_contrato::{StationMessage, VoteMessage};

    /// Una mesa de la provincia 51 con 100 de censo, 1 blanco, 2 nulos y 30 + 20 votos.
    pub fn station(table: &str) -> StationMessage {
        StationMessage {
            province: "51".to_owned(),
            municipality: "001".to_owned(),
            district: "01".to_owned(),
            section: "001".to_owned(),
            table: table.to_owned(),
            census: 100,
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

    pub fn json(station: &StationMessage) -> Vec<u8> {
        serde_json::to_vec(station).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;

    #[test]
    fn a_valid_station_is_recorded() {
        assert_eq!(
            classify(&json(&station("A"))),
            Incoming::Station(station("A"))
        );
    }

    #[test]
    fn a_station_with_more_voters_than_census_goes_to_the_dead_letter() {
        let invalid = StationMessage {
            census: 10,
            ..station("A")
        };
        match classify(&json(&invalid)) {
            Incoming::DeadLetter { key, message } => {
                assert_eq!(key.as_deref(), Some("51"));
                assert_eq!(message.reason, "voters_over_census");
                assert_eq!(message.station, Some(invalid));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_station_of_an_unknown_province_goes_to_the_dead_letter() {
        let unknown = StationMessage {
            province: "99".to_owned(),
            ..station("A")
        };
        match classify(&json(&unknown)) {
            Incoming::DeadLetter { key, message } => {
                assert_eq!(key.as_deref(), Some("99"));
                assert_eq!(message.reason, "unknown_province");
                assert_eq!(message.station, Some(unknown));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_unreadable_message_goes_to_the_dead_letter_as_it_came() {
        match classify(b"{\"province\":") {
            Incoming::DeadLetter { key, message } => {
                assert_eq!(key, None);
                assert_eq!(message.reason, "malformed");
                assert_eq!(message.raw.as_deref(), Some("{\"province\":"));
                assert!(
                    message.detail.starts_with("mensaje no válido: "),
                    "{}",
                    message.detail
                );
            }
            other => panic!("{other:?}"),
        }
    }
}
