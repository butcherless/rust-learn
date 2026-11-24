//! PostgreSQL (requisitos de escrutinio, «PostgreSQL»): las tablas, la transacción de cada mesa y
//! la lectura de los snapshots.
//!
//! `PgStore` y `PgSnapshots` usan conexiones distintas: la transacción necesita la conexión en
//! exclusiva y las lecturas de la API no deben esperarla. La lectura es una sola sentencia, así que
//! ve un estado coherente aunque el consumidor esté escribiendo. Sin tests contra una base de datos
//! real: la conversión de filas (`SnapshotRow`) es pura y sí se prueba.

use escrutinio_contrato::{CandidacyTotal, StationMessage};
use thiserror::Error;
use tokio_postgres::{Client, NoTls};

use crate::application::{Recorded, SnapshotSource, StationStore, Wipe};
use crate::domain::snapshot::ProvinceTotals;

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
CREATE TABLE IF NOT EXISTS mesa_rechazada (
    provincia text NOT NULL,
    municipio text NOT NULL,
    distrito  text NOT NULL,
    seccion   text NOT NULL,
    mesa      text NOT NULL,
    motivo    text NOT NULL,
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

const INSERT_REJECTED: &str = "
INSERT INTO mesa_rechazada (provincia, municipio, distrito, seccion, mesa, motivo)
VALUES ($1, $2, $3, $4, $5, $6)
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

/// Lo acumulado de cada provincia con alguna mesa, válida o rechazada, en una sola sentencia: así
/// las mesas, las rechazadas y los votos son de un mismo instante.
const SELECT_TOTALS: &str = "
WITH valid AS (
    SELECT p.provincia, p.mesas, p.censo, p.votantes, p.blancos, p.nulos,
           array_remove(array_agg(v.candidatura ORDER BY v.votos DESC, v.candidatura), NULL) AS candidaturas,
           array_remove(array_agg(v.siglas      ORDER BY v.votos DESC, v.candidatura), NULL) AS siglas,
           array_remove(array_agg(v.votos       ORDER BY v.votos DESC, v.candidatura), NULL) AS votos
    FROM provincia_snapshot p LEFT JOIN votos_provincia v USING (provincia)
    GROUP BY p.provincia, p.mesas, p.censo, p.votantes, p.blancos, p.nulos
), rejected AS (
    SELECT provincia, count(*) AS rechazadas FROM mesa_rechazada GROUP BY provincia
)
SELECT COALESCE(valid.provincia, rejected.provincia),
       COALESCE(valid.mesas, 0), COALESCE(rejected.rechazadas, 0),
       COALESCE(valid.censo, 0), COALESCE(valid.votantes, 0),
       COALESCE(valid.blancos, 0), COALESCE(valid.nulos, 0),
       COALESCE(valid.candidaturas, '{}'::bigint[]),
       COALESCE(valid.siglas, '{}'::text[]),
       COALESCE(valid.votos, '{}'::bigint[])
FROM valid FULL JOIN rejected ON rejected.provincia = valid.provincia
ORDER BY 1";

#[derive(Debug, Error)]
pub enum PgError {
    #[error("no se pudo conectar con PostgreSQL")]
    Connect(#[source] tokio_postgres::Error),
    #[error("falló una sentencia de PostgreSQL")]
    Query(#[from] tokio_postgres::Error),
    #[error("la fila de {0} tiene un número fuera de rango")]
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

    async fn reject(&mut self, station: &StationMessage, reason: &str) -> Result<(), PgError> {
        self.client
            .execute(
                INSERT_REJECTED,
                &[
                    &station.province,
                    &station.municipality,
                    &station.district,
                    &station.section,
                    &station.table,
                    &reason,
                ],
            )
            .await?;
        Ok(())
    }
}

/// Vacía las tres tablas en una transacción y devuelve cuántas mesas había.
impl Wipe for PgStore {
    type Error = PgError;

    async fn wipe(&mut self) -> Result<u64, PgError> {
        let transaction = self.client.transaction().await?;
        let stations: i64 = transaction
            .query_one(
                "SELECT (SELECT count(*) FROM mesa_escrutada) + (SELECT count(*) FROM mesa_rechazada)",
                &[],
            )
            .await?
            .try_get(0)?;
        transaction
            .batch_execute(
                "TRUNCATE mesa_escrutada, mesa_rechazada, provincia_snapshot, votos_provincia",
            )
            .await?;
        transaction.commit().await?;
        u64::try_from(stations).map_err(|_| PgError::OutOfRange("mesa_escrutada".to_owned()))
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

    async fn totals(&self) -> Result<Vec<ProvinceTotals>, PgError> {
        self.client
            .query(SELECT_TOTALS, &[])
            .await?
            .iter()
            .map(|row| {
                Ok(TotalsRow {
                    province: row.try_get(0)?,
                    stations: row.try_get(1)?,
                    rejected: row.try_get(2)?,
                    census: row.try_get(3)?,
                    voters: row.try_get(4)?,
                    blank: row.try_get(5)?,
                    invalid: row.try_get(6)?,
                    candidacies: row.try_get(7)?,
                    acronyms: row.try_get(8)?,
                    votes: row.try_get(9)?,
                })
                .and_then(ProvinceTotals::try_from)
            })
            .collect()
    }
}

/// Una fila de `SELECT_TOTALS`, con los tipos de PostgreSQL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TotalsRow {
    pub province: String,
    pub stations: i64,
    pub rejected: i64,
    pub census: i64,
    pub voters: i64,
    pub blank: i64,
    pub invalid: i64,
    pub candidacies: Vec<i64>,
    pub acronyms: Vec<String>,
    pub votes: Vec<i64>,
}

impl TryFrom<TotalsRow> for ProvinceTotals {
    type Error = PgError;

    fn try_from(row: TotalsRow) -> Result<Self, PgError> {
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
        Ok(ProvinceTotals {
            stations: u32::try_from(row.stations).map_err(|_| out_of_range())?,
            rejected: u32::try_from(row.rejected).map_err(|_| out_of_range())?,
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

    fn row() -> TotalsRow {
        TotalsRow {
            province: "51".to_owned(),
            stations: 2,
            rejected: 1,
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
    fn converts_a_row_into_totals() {
        let totals = ProvinceTotals::try_from(row());
        assert_eq!(
            totals.ok(),
            Some(ProvinceTotals {
                province: "51".to_owned(),
                stations: 2,
                rejected: 1,
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
            TotalsRow {
                stations: -1,
                ..row()
            },
            TotalsRow {
                stations: i64::from(u32::MAX) + 1,
                ..row()
            },
            TotalsRow {
                rejected: -1,
                ..row()
            },
            TotalsRow {
                census: -1,
                ..row()
            },
            TotalsRow {
                votes: vec![60, -1],
                ..row()
            },
            TotalsRow {
                candidacies: vec![1, -2],
                ..row()
            },
        ]
        .into_iter()
        .for_each(|row| {
            let error = ProvinceTotals::try_from(row.clone()).map_err(|e| e.to_string());
            assert_eq!(
                error.err().as_deref(),
                Some("la fila de 51 tiene un número fuera de rango"),
                "{row:?}"
            );
        });
    }
}
