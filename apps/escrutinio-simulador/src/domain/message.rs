//! Convierte una mesa en el mensaje de Kafka del contrato (`escrutinio-contrato`), con las siglas
//! de cada candidatura para que el servicio no necesite el fichero `03`.

use std::collections::BTreeMap;

use escrutinio_contrato::{StationMessage, VoteMessage};

use super::station::{Candidacy, Station};

pub fn station_message(
    station: &Station,
    candidacies: &BTreeMap<u32, Candidacy>,
) -> StationMessage {
    StationMessage {
        province: station.key.province.clone(),
        municipality: station.key.municipality.clone(),
        district: station.key.district.clone(),
        section: station.key.section.clone(),
        table: station.key.table.clone(),
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
                    .map_or_else(String::new, |c| c.acronym.clone()),
                votes: v.votes,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::station::fixtures::dataset;

    #[test]
    fn the_message_has_the_station_and_the_acronyms() {
        let dataset = dataset();
        let message = station_message(&dataset.stations[0], &dataset.candidacies);
        assert_eq!(message.id(), "51-001-01-001-A");
        assert_eq!(
            (message.census, message.blank, message.invalid),
            (100, 1, 2)
        );
        assert_eq!(
            message
                .votes
                .iter()
                .map(|v| (v.candidacy, v.acronym.as_str(), v.votes))
                .collect::<Vec<_>>(),
            [(1, "AAA", 30), (2, "BBB", 20)]
        );
    }
}
