//! Los casos de uso (requisitos de escrutinio, «Servicio»): *registrar una mesa*, que también
//! anota las rechazadas, y *consultar los snapshots*, ya calculados. PostgreSQL y el dead letter de
//! Kafka son puertos: traits genéricos, sin `dyn`, con fakes en memoria en los tests.

use std::error::Error;
use std::future::Future;

use escrutinio_contrato::{DeadLetterMessage, Snapshots, StationMessage};
use thiserror::Error;

use crate::domain::snapshot::{ProvinceTotals, snapshots as build_snapshots};
use crate::domain::{Incoming, classify};

/// Si una mesa se ha contado ahora o ya estaba contada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recorded {
    New,
    Duplicate,
}

/// Donde se registran las mesas: una transacción por mesa, que la cuenta una sola vez.
pub trait StationStore {
    type Error: Error + Send + Sync + 'static;

    fn record(
        &mut self,
        station: &StationMessage,
    ) -> impl Future<Output = Result<Recorded, Self::Error>> + Send;

    /// Anota una mesa rechazada y su motivo, una sola vez: cuenta para saber cuántas mesas posibles
    /// le quedan a su provincia.
    fn reject(
        &mut self,
        station: &StationMessage,
        reason: &str,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

/// El dead letter.
pub trait DeadLetters {
    type Error: Error + Send + Sync + 'static;

    fn send(
        &mut self,
        key: Option<&str>,
        message: &DeadLetterMessage,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

/// De donde se leen los snapshots.
pub trait SnapshotSource {
    type Error: Error + Send + Sync + 'static;

    /// Lo acumulado de las provincias que tienen alguna mesa, válida o rechazada.
    fn totals(&self) -> impl Future<Output = Result<Vec<ProvinceTotals>, Self::Error>> + Send;
}

/// Consultar los snapshots: las 52 circunscripciones y el total, con todo calculado, para que quien
/// los pinte no sume ni compare nada.
pub async fn snapshots<S: SnapshotSource>(source: &S) -> Result<Snapshots, S::Error> {
    source.totals().await.map(build_snapshots)
}

/// Qué ha pasado con un mensaje.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Recorded,
    Duplicate,
    DeadLetter,
}

#[derive(Debug, Error)]
pub enum HandleError<S: Error + 'static, D: Error + 'static> {
    #[error("no se pudo registrar la mesa {0}")]
    Store(String, #[source] S),
    #[error("no se pudo enviar al dead letter")]
    DeadLetter(#[source] D),
}

/// Registra un mensaje de `escrutinio.mesas` o lo manda al dead letter.
pub async fn handle<S: StationStore, D: DeadLetters>(
    payload: &[u8],
    store: &mut S,
    dead_letters: &mut D,
) -> Result<Outcome, HandleError<S::Error, D::Error>> {
    match classify(payload) {
        Incoming::Station(station) => match store.record(&station).await {
            Ok(Recorded::New) => Ok(Outcome::Recorded),
            Ok(Recorded::Duplicate) => Ok(Outcome::Duplicate),
            Err(error) => Err(HandleError::Store(station.id(), error)),
        },
        Incoming::DeadLetter { key, message } => {
            if let Some(station) = &message.station {
                store
                    .reject(station, &message.reason)
                    .await
                    .map_err(|error| HandleError::Store(station.id(), error))?;
            }
            dead_letters
                .send(key.as_deref(), &message)
                .await
                .map(|()| Outcome::DeadLetter)
                .map_err(HandleError::DeadLetter)
        }
    }
}

/// Las cuentas de lo procesado, para el registro de la consola.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    pub recorded: usize,
    pub duplicates: usize,
    pub dead_letters: usize,
}

impl Tally {
    pub fn with(self, outcome: Outcome) -> Self {
        match outcome {
            Outcome::Recorded => Self {
                recorded: self.recorded + 1,
                ..self
            },
            Outcome::Duplicate => Self {
                duplicates: self.duplicates + 1,
                ..self
            },
            Outcome::DeadLetter => Self {
                dead_letters: self.dead_letters + 1,
                ..self
            },
        }
    }

    pub fn total(&self) -> usize {
        self.recorded + self.duplicates + self.dead_letters
    }
}

/// Algo que se puede vaciar para empezar otro escrutinio: devuelve cuántos elementos borró.
pub trait Wipe {
    type Error: Error + Send + Sync + 'static;

    fn wipe(&mut self) -> impl Future<Output = Result<u64, Self::Error>> + Send;
}

/// Lo borrado al vaciar el escrutinio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wiped {
    pub messages: u64,
    pub stations: u64,
}

#[derive(Debug, Error)]
pub enum WipeError<K: Error + 'static, P: Error + 'static> {
    #[error("no se pudieron borrar los mensajes de Kafka")]
    Kafka(#[source] K),
    #[error("no se pudieron borrar las mesas de PostgreSQL")]
    Postgres(#[source] P),
}

/// Borra el escrutinio: primero los mensajes de Kafka, para que no vuelvan a contarse, y después
/// las mesas y los snapshots de PostgreSQL.
pub async fn wipe<K: Wipe, P: Wipe>(
    kafka: &mut K,
    postgres: &mut P,
) -> Result<Wiped, WipeError<K::Error, P::Error>> {
    let messages = kafka.wipe().await.map_err(WipeError::Kafka)?;
    let stations = postgres.wipe().await.map_err(WipeError::Postgres)?;
    Ok(Wiped { messages, stations })
}

#[cfg(test)]
pub(crate) mod fakes {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fmt;

    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    pub struct Down;

    impl fmt::Display for Down {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.pad("caído")
        }
    }

    impl Error for Down {}

    /// Guarda las mesas registradas y las rechazadas con su motivo; con `down`, falla.
    #[derive(Default)]
    pub struct FakeStore {
        pub ids: BTreeSet<String>,
        pub rejected: BTreeMap<String, String>,
        pub down: bool,
    }

    impl StationStore for FakeStore {
        type Error = Down;

        async fn record(&mut self, station: &StationMessage) -> Result<Recorded, Down> {
            match (self.down, self.ids.insert(station.id())) {
                (true, _) => Err(Down),
                (false, true) => Ok(Recorded::New),
                (false, false) => Ok(Recorded::Duplicate),
            }
        }

        async fn reject(&mut self, station: &StationMessage, reason: &str) -> Result<(), Down> {
            if self.down {
                return Err(Down);
            }
            self.rejected.insert(station.id(), reason.to_owned());
            Ok(())
        }
    }

    #[derive(Default)]
    pub struct FakeDeadLetters {
        pub sent: Vec<(Option<String>, DeadLetterMessage)>,
        pub down: bool,
    }

    impl DeadLetters for FakeDeadLetters {
        type Error = Down;

        async fn send(
            &mut self,
            key: Option<&str>,
            message: &DeadLetterMessage,
        ) -> Result<(), Down> {
            if self.down {
                return Err(Down);
            }
            self.sent.push((key.map(str::to_owned), message.clone()));
            Ok(())
        }
    }

    /// Devuelve `count` al vaciarse, o falla; `wiped` dice si se llegó a vaciar.
    #[derive(Default)]
    pub struct FakeWipe {
        pub count: u64,
        pub down: bool,
        pub wiped: bool,
    }

    impl Wipe for FakeWipe {
        type Error = Down;

        async fn wipe(&mut self) -> Result<u64, Down> {
            if self.down {
                return Err(Down);
            }
            self.wiped = true;
            Ok(self.count)
        }
    }

    /// Devuelve siempre los mismos totales, o falla.
    pub struct FakeSnapshots(pub Result<Vec<ProvinceTotals>, ()>);

    impl SnapshotSource for FakeSnapshots {
        type Error = Down;

        async fn totals(&self) -> Result<Vec<ProvinceTotals>, Down> {
            self.0.clone().map_err(|()| Down)
        }
    }
}

#[cfg(test)]
mod tests {
    use escrutinio_contrato::StationMessage;

    use super::fakes::*;
    use super::*;
    use crate::domain::fixtures::{json, station};

    #[tokio::test]
    async fn records_a_station_once_and_ignores_it_after() {
        let (mut store, mut dead) = (FakeStore::default(), FakeDeadLetters::default());
        let payload = json(&station("A"));
        assert_eq!(
            handle(&payload, &mut store, &mut dead).await.ok(),
            Some(Outcome::Recorded)
        );
        assert_eq!(
            handle(&payload, &mut store, &mut dead).await.ok(),
            Some(Outcome::Duplicate)
        );
        assert_eq!(store.ids.len(), 1);
        assert!(dead.sent.is_empty());
    }

    #[tokio::test]
    async fn sends_what_cannot_be_counted_to_the_dead_letter() {
        let (mut store, mut dead) = (FakeStore::default(), FakeDeadLetters::default());
        let invalid = json(&StationMessage {
            census: 10,
            ..station("A")
        });
        for payload in [invalid.as_slice(), b"no es json"] {
            let outcome = handle(payload, &mut store, &mut dead).await;
            assert_eq!(outcome.ok(), Some(Outcome::DeadLetter));
        }
        assert!(store.ids.is_empty());
        assert_eq!(
            dead.sent
                .iter()
                .map(|(key, m)| (key.as_deref(), m.reason.as_str()))
                .collect::<Vec<_>>(),
            [(Some("51"), "voters_over_census"), (None, "malformed")]
        );
    }

    #[tokio::test]
    async fn notes_each_rejected_station_before_sending_it_to_the_dead_letter() {
        let (mut store, mut dead) = (FakeStore::default(), FakeDeadLetters::default());
        let invalid = json(&StationMessage {
            census: 10,
            ..station("A")
        });
        // Dos veces: la anota una vez y la manda las dos, para no perderla si falló el envío.
        for _ in 0..2 {
            handle(&invalid, &mut store, &mut dead).await.ok();
        }
        // Un mensaje ilegible no es de ninguna provincia: no se anota.
        handle(b"x", &mut store, &mut dead).await.ok();
        assert_eq!(
            store.rejected.into_iter().collect::<Vec<_>>(),
            [(
                "51-001-01-001-A".to_owned(),
                "voters_over_census".to_owned()
            )]
        );
        assert_eq!(dead.sent.len(), 3);
    }

    #[tokio::test]
    async fn serves_the_snapshots_calculated_and_fails_with_postgres() {
        use crate::domain::snapshot::fixtures::totals;
        let snapshots = snapshots(&FakeSnapshots(Ok(vec![totals("51", 96, 0)]))).await;
        let ceuta = snapshots
            .as_ref()
            .ok()
            .and_then(|s| s.provinces.iter().find(|p| p.province == "51"));
        assert_eq!(
            ceuta.map(|p| (p.stations, p.possible, p.complete)),
            Some((96, 96, true))
        );
        assert_eq!(snapshots.map(|s| s.provinces.len()).ok(), Some(52));
        assert_eq!(snapshots_error().await, Some(Down));
    }

    async fn snapshots_error() -> Option<Down> {
        snapshots(&FakeSnapshots(Err(()))).await.err()
    }

    #[tokio::test]
    async fn fails_when_postgres_or_the_dead_letter_fail() {
        let mut store = FakeStore {
            down: true,
            ..FakeStore::default()
        };
        let mut dead = FakeDeadLetters {
            down: true,
            ..FakeDeadLetters::default()
        };
        let stored = handle(&json(&station("A")), &mut store, &mut dead).await;
        assert!(matches!(&stored, Err(HandleError::Store(id, Down)) if id == "51-001-01-001-A"));
        assert_eq!(
            stored.map_err(|e| e.to_string()).err().as_deref(),
            Some("no se pudo registrar la mesa 51-001-01-001-A")
        );
        let dead_letter = handle(b"x", &mut store, &mut dead).await;
        assert!(matches!(dead_letter, Err(HandleError::DeadLetter(Down))));
    }

    #[tokio::test]
    async fn wipes_kafka_and_then_postgres() {
        let mut kafka = FakeWipe {
            count: 60_314,
            ..FakeWipe::default()
        };
        let mut postgres = FakeWipe {
            count: 60_310,
            ..FakeWipe::default()
        };
        assert_eq!(
            wipe(&mut kafka, &mut postgres).await.ok(),
            Some(Wiped {
                messages: 60_314,
                stations: 60_310
            })
        );
        assert!(kafka.wiped && postgres.wiped);
    }

    #[tokio::test]
    async fn does_not_touch_postgres_when_kafka_fails() {
        let mut kafka = FakeWipe {
            down: true,
            ..FakeWipe::default()
        };
        let mut postgres = FakeWipe::default();
        let result = wipe(&mut kafka, &mut postgres).await;
        assert!(matches!(result, Err(WipeError::Kafka(Down))), "{result:?}");
        assert!(!postgres.wiped);
        let result = wipe(
            &mut FakeWipe::default(),
            &mut FakeWipe {
                down: true,
                ..FakeWipe::default()
            },
        )
        .await;
        assert_eq!(
            result.map_err(|e| e.to_string()).err().as_deref(),
            Some("no se pudieron borrar las mesas de PostgreSQL")
        );
    }

    #[test]
    fn tallies_the_outcomes() {
        let tally = [
            Outcome::Recorded,
            Outcome::Recorded,
            Outcome::Duplicate,
            Outcome::DeadLetter,
        ]
        .into_iter()
        .fold(Tally::default(), Tally::with);
        assert_eq!(
            tally,
            Tally {
                recorded: 2,
                duplicates: 1,
                dead_letters: 1
            }
        );
        assert_eq!(tally.total(), 4);
    }
}
