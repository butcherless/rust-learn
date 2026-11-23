//! El mensaje de una mesa en Kafka (requisitos de escrutinio, «Kafka»): la clave es el código de
//! provincia y el valor, la mesa en JSON. Es el contrato con el servicio.

use std::collections::BTreeMap;

use serde::Serialize;

use super::station::{Candidacy, Station};

/// La mesa tal como viaja en el topic.
#[derive(Debug, Serialize)]
pub struct StationMessage<'a> {
    pub province: &'a str,
    pub municipality: &'a str,
    pub district: &'a str,
    pub section: &'a str,
    pub table: &'a str,
    pub census: u32,
    pub blank: u32,
    pub invalid: u32,
    pub votes: Vec<VoteMessage<'a>>,
}

/// Los votos de una candidatura, con sus siglas para que el servicio no necesite el fichero `03`.
#[derive(Debug, Serialize)]
pub struct VoteMessage<'a> {
    pub candidacy: u32,
    pub acronym: &'a str,
    pub votes: u32,
}

impl<'a> StationMessage<'a> {
    pub fn new(station: &'a Station, candidacies: &'a BTreeMap<u32, Candidacy>) -> Self {
        Self {
            province: &station.key.province,
            municipality: &station.key.municipality,
            district: &station.key.district,
            section: &station.key.section,
            table: &station.key.table,
            census: station.census,
            blank: station.blank,
            invalid: station.invalid,
            votes: station
                .votes
                .iter()
                .map(|v| VoteMessage {
                    candidacy: v.candidacy,
                    acronym: candidacies
                        .get(&v.candidacy)
                        .map_or("", |c| c.acronym.as_str()),
                    votes: v.votes,
                })
                .collect(),
        }
    }

    /// La clave del mensaje: el código de provincia, para que sus mesas vayan a la misma partición.
    pub fn key(&self) -> &'a str {
        self.province
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::station::fixtures::dataset;

    #[test]
    fn the_message_is_the_station_in_json_keyed_by_province() {
        let dataset = dataset();
        let message = StationMessage::new(&dataset.stations[0], &dataset.candidacies);
        assert_eq!(message.key(), "51");
        assert_eq!(
            serde_json::to_string(&message).ok().as_deref(),
            Some(
                r#"{"province":"51","municipality":"001","district":"01","section":"001","table":"A","census":100,"blank":1,"invalid":2,"votes":[{"candidacy":1,"acronym":"AAA","votes":30},{"candidacy":2,"acronym":"BBB","votes":20}]}"#
            )
        );
    }
}
