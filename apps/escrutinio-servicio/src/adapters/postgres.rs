//! PostgreSQL (requisitos de escrutinio, «PostgreSQL»): las tablas, la transacción de cada mesa y
//! la lectura de los snapshots.
//!
//! `PgStore` y `PgSnapshots` usan conexiones distintas: la transacción necesita la conexión en
//! exclusiva y las lecturas de la API no deben esperarla. La lectura es una sola sentencia, así que
//! ve un estado coherente aunque el consumidor esté escribiendo. Sin tests contra una base de datos
//! real: la conversión de filas (`SnapshotRow`) es pura y sí se prueba.

use escrutinio_contrato::{CandidacyTotal, ProvinceSnapshot, StationMessage};
use thiserror::Error;
use tokio_postgres::{Client, NoTls};

use crate::application::{Recorded, SnapshotSource, StationStore};

/// Las tablas, si no existen.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS mesa_escrutada (
    provincia text NOT NULL,
    municipio text NOT NULL,
    distrito  text NOT NULL,
    seccion   text NOT NULL,
    mesa      text NOT NULL,
    censo     bigint NOT NULL,
    votantes  bigint NOT NULL,
    recibida  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (provincia, municipio, distrito, seccion, mesa)
);
CREATE TABLE IF NOT EXISTS provincia_snapshot (
    provincia text PRIMARY KEY,
    mesas     bigint NOT NULL,
    censo     bigint NOT NULL,
    votantes  bigint NOT NULL,
    blancos   bigint NOT NULL,
    nulos     bigint NOT NULL
);
CREATE TABLE IF NOT EXISTS votos_provincia (
    provincia   text NOT NULL,
    candidatura bigint NOT NULL,
    siglas      text NOT NULL,
    votos       bigint NOT NULL,
    PRIMARY KEY (provincia, candidatura)
);
";

/// Los límites de la sesión que escribe (requisitos, «PostgreSQL»): transacciones cortas.
const LIMITS: &str = "
SET transaction_timeout = '5s';
SET statement_timeout = '2s';
SET idle_in_transaction_session_timeout = '1s';
";

const INSERT_STATION: &str = "
INSERT INTO mesa_escrutada (provincia, municipio, distrito, seccion, mesa, censo, votantes)
VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT DO NOTHING";

const UPSERT_SNAPSHOT: &str = "
INSERT INTO provincia_snapshot (provincia, mesas, censo, votantes, blancos, nulos)
VALUES ($1, 1, $2, $3, $4, $5)
ON CONFLICT (provincia) DO UPDATE SET
    mesas    = provincia_snapshot.mesas + 1,
    censo    = provincia_snapshot.censo + EXCLUDED.censo,
    votantes = provincia_snapshot.votantes + EXCLUDED.votantes,
    blancos  = provincia_snapshot.blancos + EXCLUDED.blancos,
    nulos    = provincia_snapshot.nulos + EXCLUDED.nulos";

const UPSERT_VOTES: &str = "
INSERT INTO votos_provincia (provincia, candidatura, siglas, votos)
SELECT $1, c, s, v FROM unnest($2::bigint[], $3::text[], $4::bigint[]) AS t(c, s, v)
ON CONFLICT (provincia, candidatura) DO UPDATE SET votos = votos_provincia.votos + EXCLUDED.votos";

const SELECT_SNAPSHOTS: &str = "
SELECT p.provincia, p.mesas, p.censo, p.votantes, p.blancos, p.nulos,
       array_remove(array_agg(v.candidatura ORDER BY v.votos DESC, v.candidatura), NULL),
       array_remove(array_agg(v.siglas      ORDER BY v.votos DESC, v.candidatura), NULL),
       array_remove(array_agg(v.votos       ORDER BY v.votos DESC, v.candidatura), NULL)
FROM provincia_snapshot p LEFT JOIN votos_provincia v USING (provincia)
GROUP BY p.provincia, p.mesas, p.censo, p.votantes, p.blancos, p.nulos
ORDER BY p.provincia";

#[derive(Debug, Error)]
pub enum PgError {
    #[error("no se pudo conectar con PostgreSQL")]
    Connect(#[source] tokio_postgres::Error),
    #[error("falló una sentencia de PostgreSQL")]
    Query(#[from] tokio_postgres::Error),
    #[error("la fila de la provincia {0} tiene un número fuera de rango")]
    OutOfRange(String),
}

/// Abre una conexión y la atiende en una tarea de tokio.
async fn connect(url: &str) -> Result<Client, PgError> {
    let (client, connection) = tokio_postgres::connect(url, NoTls)
        .await
        .map_err(PgError::Connect)?;
    tokio::spawn(async move {
        if let Err(error) = connection.await {
            eprintln!("error: conexión con PostgreSQL: {error}");
        }
    });
    Ok(client)
}

/// La conexión que registra las mesas.
pub struct PgStore {
    client: Client,
}

impl PgStore {
    /// Conecta, crea las tablas si faltan y fija los límites de la sesión.
    pub async fn connect(url: &str) -> Result<Self, PgError> {
        let client = connect(url).await?;
        client.batch_execute(SCHEMA).await?;
        client.batch_execute(LIMITS).await?;
        Ok(Self { client })
    }
}

impl StationStore for PgStore {
    type Error = PgError;

    async fn record(&mut self, station: &StationMessage) -> Result<Recorded, PgError> {
        let transaction = self.client.transaction().await?;
        let inserted = transaction
            .execute(
                INSERT_STATION,
                &[
                    &station.province,
                    &station.municipality,
                    &station.district,
                    &station.section,
                    &station.table,
                    &i64::from(station.census),
                    &i64::from(station.voters()),
                ],
            )
            .await?;
        if inserted == 0 {
            transaction.commit().await?;
            return Ok(Recorded::Duplicate);
        }
        transaction
            .execute(
                UPSERT_SNAPSHOT,
                &[
                    &station.province,
                    &i64::from(station.census),
                    &i64::from(station.voters()),
                    &i64::from(station.blank),
                    &i64::from(station.invalid),
                ],
            )
            .await?;
        let candidacies: Vec<i64> = station
            .votes
            .iter()
            .map(|v| i64::from(v.candidacy))
            .collect();
        let acronyms: Vec<&str> = station.votes.iter().map(|v| v.acronym.as_str()).collect();
        let votes: Vec<i64> = station.votes.iter().map(|v| i64::from(v.votes)).collect();
        transaction
            .execute(
                UPSERT_VOTES,
                &[&station.province, &candidacies, &acronyms, &votes],
            )
            .await?;
        transaction.commit().await?;
        Ok(Recorded::New)
    }
}

/// La conexión que lee los snapshots para la API.
pub struct PgSnapshots {
    client: Client,
}

impl PgSnapshots {
    pub async fn connect(url: &str) -> Result<Self, PgError> {
        Ok(Self {
            client: connect(url).await?,
        })
    }
}

impl SnapshotSource for PgSnapshots {
    type Error = PgError;

    async fn snapshots(&self) -> Result<Vec<ProvinceSnapshot>, PgError> {
        self.client
            .query(SELECT_SNAPSHOTS, &[])
            .await?
            .iter()
            .map(|row| {
                Ok(SnapshotRow {
                    province: row.try_get(0)?,
                    stations: row.try_get(1)?,
                    census: row.try_get(2)?,
                    voters: row.try_get(3)?,
                    blank: row.try_get(4)?,
                    invalid: row.try_get(5)?,
                    candidacies: row.try_get(6)?,
                    acronyms: row.try_get(7)?,
                    votes: row.try_get(8)?,
                })
                .and_then(ProvinceSnapshot::try_from)
            })
            .collect()
    }
}

/// Una fila de `SELECT_SNAPSHOTS`, con los tipos de PostgreSQL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotRow {
    pub province: String,
    pub stations: i64,
    pub census: i64,
    pub voters: i64,
    pub blank: i64,
    pub invalid: i64,
    pub candidacies: Vec<i64>,
    pub acronyms: Vec<String>,
    pub votes: Vec<i64>,
}

impl TryFrom<SnapshotRow> for ProvinceSnapshot {
    type Error = PgError;

    fn try_from(row: SnapshotRow) -> Result<Self, PgError> {
        let out_of_range = || PgError::OutOfRange(row.province.clone());
        let unsigned = |n: i64| u64::try_from(n).map_err(|_| out_of_range());
        let votes = row
            .candidacies
            .iter()
            .zip(&row.acronyms)
            .zip(&row.votes)
            .map(|((&candidacy, acronym), &votes)| {
                Ok(CandidacyTotal {
                    candidacy: u32::try_from(candidacy).map_err(|_| out_of_range())?,
                    acronym: acronym.clone(),
                    votes: unsigned(votes)?,
                })
            })
            .collect::<Result<Vec<_>, PgError>>()?;
        Ok(ProvinceSnapshot {
            stations: u32::try_from(row.stations).map_err(|_| out_of_range())?,
            census: unsigned(row.census)?,
            voters: unsigned(row.voters)?,
            blank: unsigned(row.blank)?,
            invalid: unsigned(row.invalid)?,
            votes,
            province: row.province,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> SnapshotRow {
        SnapshotRow {
            province: "51".to_owned(),
            stations: 2,
            census: 200,
            voters: 106,
            blank: 2,
            invalid: 4,
            candidacies: vec![1, 2],
            acronyms: vec!["AAA".to_owned(), "BBB".to_owned()],
            votes: vec![60, 40],
        }
    }

    #[test]
    fn converts_a_row_into_a_snapshot() {
        let snapshot = ProvinceSnapshot::try_from(row());
        assert_eq!(
            snapshot.ok(),
            Some(ProvinceSnapshot {
                province: "51".to_owned(),
                stations: 2,
                census: 200,
                voters: 106,
                blank: 2,
                invalid: 4,
                votes: vec![
                    CandidacyTotal {
                        candidacy: 1,
                        acronym: "AAA".to_owned(),
                        votes: 60
                    },
                    CandidacyTotal {
                        candidacy: 2,
                        acronym: "BBB".to_owned(),
                        votes: 40
                    },
                ],
            })
        );
    }

    #[test]
    fn rejects_negative_or_too_large_numbers() {
        [
            SnapshotRow {
                stations: -1,
                ..row()
            },
            SnapshotRow {
                stations: i64::from(u32::MAX) + 1,
                ..row()
            },
            SnapshotRow {
                census: -1,
                ..row()
            },
            SnapshotRow {
                votes: vec![60, -1],
                ..row()
            },
            SnapshotRow {
                candidacies: vec![1, -2],
                ..row()
            },
        ]
        .into_iter()
        .for_each(|row| {
            let error = ProvinceSnapshot::try_from(row.clone()).map_err(|e| e.to_string());
            assert_eq!(
                error.err().as_deref(),
                Some("la fila de la provincia 51 tiene un número fuera de rango"),
                "{row:?}"
            );
        });
    }
}
