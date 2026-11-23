//! Las mesas de 2023 y sus comprobaciones (requisitos de escrutinio, «Resultados de 2023 por
//! mesa»).
//!
//! `build` une las filas de mesas (`09`) con sus votos por candidatura (`10`), deja fuera el CERA
//! y comprueba que el fichero cuadra antes de publicar nada. Lo que solo afecta a una mesa, como
//! tener más votantes que censo, no rechaza el fichero: lo valida el contrato (`check_voters`).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use thiserror::Error;

use escrutinio_contrato::check_voters;

/// El municipio con el que Interior marca las filas del voto exterior (CERA).
pub const CERA_MUNICIPALITY: &str = "999";

/// La clave de una mesa: provincia, municipio, distrito, sección y mesa.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StationKey {
    pub province: String,
    pub municipality: String,
    pub district: String,
    pub section: String,
    pub table: String,
}

impl StationKey {
    pub fn is_cera(&self) -> bool {
        self.municipality == CERA_MUNICIPALITY
    }
}

impl fmt::Display for StationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(&format!(
            "{}-{}-{}-{}-{}",
            self.province, self.municipality, self.district, self.section, self.table
        ))
    }
}

/// Una fila del fichero de mesas (`09`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StationRow {
    pub key: StationKey,
    pub census: u32,
    pub blank: u32,
    pub invalid: u32,
    pub candidacy_votes: u32,
}

/// Una fila del fichero de votos por mesa y candidatura (`10`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoteRow {
    pub key: StationKey,
    pub candidacy: u32,
    pub votes: u32,
}

/// Una candidatura (`03`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidacy {
    pub code: u32,
    pub acronym: String,
    pub name: String,
}

/// Los votos de una candidatura en una mesa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CandidacyVotes {
    pub candidacy: u32,
    pub votes: u32,
}

/// Una mesa comprobada, con sus votos por candidatura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Station {
    pub key: StationKey,
    pub census: u32,
    pub blank: u32,
    pub invalid: u32,
    pub votes: Vec<CandidacyVotes>,
}

impl Station {
    /// Votantes = blancos + nulos + votos a candidaturas: el fichero no los trae.
    pub fn voters(&self) -> u32 {
        self.blank + self.invalid + self.votes.iter().map(|v| v.votes).sum::<u32>()
    }
}

/// Las mesas y las candidaturas, ya comprobadas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dataset {
    /// Ordenadas por su clave.
    pub stations: Vec<Station>,
    pub candidacies: BTreeMap<u32, Candidacy>,
}

/// Los totales que se muestran al comprobar. `rejected` son las mesas que irán al dead letter; las
/// demás cifras las incluyen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub stations: usize,
    pub rejected: usize,
    pub provinces: usize,
    pub census: u64,
    pub voters: u64,
}

impl Dataset {
    pub fn summary(&self) -> Summary {
        Summary {
            stations: self.stations.len(),
            rejected: self
                .stations
                .iter()
                .filter(|s| check_voters(s.voters(), s.census).is_err())
                .count(),
            provinces: self
                .stations
                .iter()
                .map(|s| &s.key.province)
                .collect::<BTreeSet<_>>()
                .len(),
            census: self.stations.iter().map(|s| u64::from(s.census)).sum(),
            voters: self.stations.iter().map(|s| u64::from(s.voters())).sum(),
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CheckError {
    #[error("no hay ninguna mesa")]
    Empty,
    #[error("la mesa {0} está repetida")]
    Duplicate(Box<StationKey>),
    #[error("hay votos de la mesa {0}, que no existe")]
    UnknownStation(Box<StationKey>),
    #[error("la mesa {0} no tiene votos por candidatura")]
    NoVotes(Box<StationKey>),
    #[error("la mesa {key} tiene votos de la candidatura {candidacy}, que no existe")]
    UnknownCandidacy {
        key: Box<StationKey>,
        candidacy: u32,
    },
    #[error("los votos por candidatura de la mesa {key} suman {sum}, y no {expected}")]
    VotesMismatch {
        key: Box<StationKey>,
        expected: u32,
        sum: u32,
    },
    #[error("la provincia {province} tiene {found} mesas, y no {expected}")]
    StationCount {
        province: String,
        expected: u32,
        found: u32,
    },
}

/// Une y comprueba las filas. `expected` son las mesas de cada provincia según los totales
/// oficiales (`07`).
pub fn build(
    stations: Vec<StationRow>,
    votes: Vec<VoteRow>,
    candidacies: Vec<Candidacy>,
    expected: &BTreeMap<String, u32>,
) -> Result<Dataset, CheckError> {
    let rows = unique(stations.into_iter().filter(|row| !row.key.is_cera()))?;
    let grouped = votes.into_iter().filter(|vote| !vote.key.is_cera()).fold(
        BTreeMap::<StationKey, Vec<CandidacyVotes>>::new(),
        |mut map, vote| {
            map.entry(vote.key).or_default().push(CandidacyVotes {
                candidacy: vote.candidacy,
                votes: vote.votes,
            });
            map
        },
    );
    grouped
        .keys()
        .find(|key| !rows.contains_key(*key))
        .map_or(Ok(()), |key| {
            Err(CheckError::UnknownStation(Box::new(key.clone())))
        })?;
    let candidacies: BTreeMap<u32, Candidacy> =
        candidacies.into_iter().map(|c| (c.code, c)).collect();
    let stations = rows
        .into_values()
        .map(|row| {
            let votes = grouped.get(&row.key).cloned().unwrap_or_default();
            station(row, votes, &candidacies)
        })
        .collect::<Result<Vec<_>, _>>()?;
    check_counts(&stations, expected)?;
    Ok(Dataset {
        stations,
        candidacies,
    })
}

fn unique(
    mut rows: impl Iterator<Item = StationRow>,
) -> Result<BTreeMap<StationKey, StationRow>, CheckError> {
    rows.try_fold(BTreeMap::new(), |mut map, row| {
        let key = row.key.clone();
        match map.insert(key.clone(), row) {
            Some(_) => Err(CheckError::Duplicate(Box::new(key))),
            None => Ok(map),
        }
    })
}

fn station(
    row: StationRow,
    votes: Vec<CandidacyVotes>,
    candidacies: &BTreeMap<u32, Candidacy>,
) -> Result<Station, CheckError> {
    if votes.is_empty() {
        return Err(CheckError::NoVotes(Box::new(row.key)));
    }
    if let Some(unknown) = votes
        .iter()
        .find(|v| !candidacies.contains_key(&v.candidacy))
    {
        return Err(CheckError::UnknownCandidacy {
            key: Box::new(row.key),
            candidacy: unknown.candidacy,
        });
    }
    let sum = votes.iter().map(|v| v.votes).sum::<u32>();
    if sum != row.candidacy_votes {
        return Err(CheckError::VotesMismatch {
            key: Box::new(row.key),
            expected: row.candidacy_votes,
            sum,
        });
    }
    Ok(Station {
        key: row.key,
        census: row.census,
        blank: row.blank,
        invalid: row.invalid,
        votes,
    })
}

fn check_counts(stations: &[Station], expected: &BTreeMap<String, u32>) -> Result<(), CheckError> {
    if stations.is_empty() {
        return Err(CheckError::Empty);
    }
    let found = stations
        .iter()
        .fold(BTreeMap::<&str, u32>::new(), |mut map, s| {
            *map.entry(s.key.province.as_str()).or_default() += 1;
            map
        });
    found
        .keys()
        .copied()
        .chain(expected.keys().map(String::as_str))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|province| {
            let want = expected.get(province).copied().unwrap_or(0);
            let got = found.get(province).copied().unwrap_or(0);
            (province, want, got)
        })
        .find(|(_, want, got)| want != got)
        .map_or(Ok(()), |(province, expected, found)| {
            Err(CheckError::StationCount {
                province: province.to_owned(),
                expected,
                found,
            })
        })
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;

    pub fn key(province: &str, table: &str) -> StationKey {
        StationKey {
            province: province.to_owned(),
            municipality: "001".to_owned(),
            district: "01".to_owned(),
            section: "001".to_owned(),
            table: table.to_owned(),
        }
    }

    pub fn cera(province: &str) -> StationKey {
        StationKey {
            municipality: CERA_MUNICIPALITY.to_owned(),
            ..key(province, "U")
        }
    }

    /// Una mesa con 100 de censo, 1 blanco, 2 nulos y 30 + 20 votos a las candidaturas 1 y 2.
    pub fn row(key: StationKey) -> StationRow {
        StationRow {
            key,
            census: 100,
            blank: 1,
            invalid: 2,
            candidacy_votes: 50,
        }
    }

    pub fn votes(key: &StationKey) -> Vec<VoteRow> {
        [(1, 30), (2, 20)]
            .into_iter()
            .map(|(candidacy, votes)| VoteRow {
                key: key.clone(),
                candidacy,
                votes,
            })
            .collect()
    }

    pub fn candidacies() -> Vec<Candidacy> {
        [(1, "AAA"), (2, "BBB")]
            .into_iter()
            .map(|(code, acronym)| Candidacy {
                code,
                acronym: acronym.to_owned(),
                name: format!("Partido {acronym}"),
            })
            .collect()
    }

    pub fn expected(counts: &[(&str, u32)]) -> BTreeMap<String, u32> {
        counts.iter().map(|(p, n)| ((*p).to_owned(), *n)).collect()
    }

    /// Tres mesas, dos en la provincia 51 y una en la 52, y una fila de CERA en la 51.
    pub fn dataset() -> Dataset {
        let keys = [key("51", "A"), key("51", "B"), key("52", "U")];
        let rows = keys
            .iter()
            .cloned()
            .map(row)
            .chain([row(cera("51"))])
            .collect();
        let votes = keys.iter().chain([&cera("51")]).flat_map(votes).collect();
        build(
            rows,
            votes,
            candidacies(),
            &expected(&[("51", 2), ("52", 1)]),
        )
        .unwrap_or_else(|error| panic!("fixture: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;

    #[test]
    fn builds_the_stations_without_cera_sorted_by_key() {
        let dataset = dataset();
        assert_eq!(
            dataset
                .stations
                .iter()
                .map(|s| s.key.to_string())
                .collect::<Vec<_>>(),
            ["51-001-01-001-A", "51-001-01-001-B", "52-001-01-001-U"]
        );
        assert_eq!(dataset.stations[0].votes.len(), 2);
        assert_eq!(dataset.candidacies.len(), 2);
    }

    #[test]
    fn voters_are_blank_plus_invalid_plus_candidacy_votes() {
        assert_eq!(dataset().stations[0].voters(), 53);
    }

    #[test]
    fn summarises_stations_provinces_census_and_voters() {
        assert_eq!(
            dataset().summary(),
            Summary {
                stations: 3,
                rejected: 0,
                provinces: 2,
                census: 300,
                voters: 159,
            }
        );
    }

    #[test]
    fn the_key_pads_like_a_string() {
        assert_eq!(format!("{:>17}", key("51", "A")), "  51-001-01-001-A");
    }

    /// Nombre, mesas, votos, mesas oficiales por provincia y el error esperado.
    type Case = (
        &'static str,
        Vec<StationRow>,
        Vec<VoteRow>,
        BTreeMap<String, u32>,
        CheckError,
    );

    #[test]
    fn rejects_data_that_does_not_add_up() {
        let a = key("51", "A");
        let ok_rows = || vec![row(a.clone())];
        let ok_votes = || votes(&a);
        let one = expected(&[("51", 1)]);
        let cases: Vec<Case> = vec![
            ("empty", vec![], vec![], expected(&[]), CheckError::Empty),
            (
                "duplicate",
                vec![row(a.clone()), row(a.clone())],
                ok_votes(),
                one.clone(),
                CheckError::Duplicate(Box::new(a.clone())),
            ),
            (
                "votes of an unknown station",
                ok_rows(),
                [ok_votes(), votes(&key("51", "Z"))].concat(),
                one.clone(),
                CheckError::UnknownStation(Box::new(key("51", "Z"))),
            ),
            (
                "no votes",
                ok_rows(),
                vec![],
                one.clone(),
                CheckError::NoVotes(Box::new(a.clone())),
            ),
            (
                "unknown candidacy",
                ok_rows(),
                vec![VoteRow {
                    key: a.clone(),
                    candidacy: 9,
                    votes: 50,
                }],
                one.clone(),
                CheckError::UnknownCandidacy {
                    key: Box::new(a.clone()),
                    candidacy: 9,
                },
            ),
            (
                "votes mismatch",
                vec![StationRow {
                    candidacy_votes: 49,
                    ..row(a.clone())
                }],
                ok_votes(),
                one.clone(),
                CheckError::VotesMismatch {
                    key: Box::new(a.clone()),
                    expected: 49,
                    sum: 50,
                },
            ),
            (
                "fewer stations than the official total",
                ok_rows(),
                ok_votes(),
                expected(&[("51", 2)]),
                CheckError::StationCount {
                    province: "51".to_owned(),
                    expected: 2,
                    found: 1,
                },
            ),
            (
                "a province without stations",
                ok_rows(),
                ok_votes(),
                expected(&[("51", 1), ("52", 3)]),
                CheckError::StationCount {
                    province: "52".to_owned(),
                    expected: 3,
                    found: 0,
                },
            ),
        ];
        cases
            .into_iter()
            .for_each(|(name, rows, votes, expected, error)| {
                assert_eq!(
                    build(rows, votes, candidacies(), &expected),
                    Err(error),
                    "{name}"
                );
            });
    }

    #[test]
    fn check_errors_explain_the_problem() {
        assert_eq!(
            CheckError::VotesMismatch {
                key: Box::new(key("51", "A")),
                expected: 49,
                sum: 50
            }
            .to_string(),
            "los votos por candidatura de la mesa 51-001-01-001-A suman 50, y no 49"
        );
    }
}
