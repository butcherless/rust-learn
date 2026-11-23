//! El caso de uso: validar y publicar el plan, cada mesa en su momento. Las válidas van al topic de
//! mesas y las demás al dead letter, con su motivo. Kafka y el reloj son puertos (traits genéricos),
//! así que se prueba con fakes en memoria.

use std::collections::BTreeMap;
use std::error::Error;
use std::time::Duration;

use thiserror::Error;

use escrutinio_contrato::{DeadLetterMessage, validate};

use crate::domain::message::station_message;
use crate::domain::schedule::Scheduled;
use crate::domain::station::{Candidacy, StationKey};

/// A qué topic va un mensaje.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topic {
    Stations,
    DeadLetter,
}

/// Donde se publican las mesas.
pub trait Publisher {
    type Error: Error + 'static;

    fn publish(&mut self, topic: Topic, key: &str, payload: &str) -> Result<(), Self::Error>;

    /// Espera a que se confirme todo lo publicado.
    fn finish(&mut self) -> Result<(), Self::Error>;
}

/// Espera hasta un momento contado desde el inicio de la publicación.
pub trait Pacer {
    fn wait_until(&mut self, at: Duration);
}

/// Cuántas mesas han ido a cada topic.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Published {
    pub valid: usize,
    pub rejected: usize,
}

impl Published {
    pub fn total(&self) -> usize {
        self.valid + self.rejected
    }

    fn add(self, topic: Topic) -> Self {
        match topic {
            Topic::Stations => Self {
                valid: self.valid + 1,
                ..self
            },
            Topic::DeadLetter => Self {
                rejected: self.rejected + 1,
                ..self
            },
        }
    }
}

#[derive(Debug, Error)]
pub enum RunError<E: Error + 'static> {
    #[error("no se pudo codificar la mesa {0}")]
    Encode(Box<StationKey>, #[source] serde_json::Error),
    #[error("no se pudo publicar la mesa {0}")]
    Publish(Box<StationKey>, #[source] E),
    #[error("no se confirmaron todas las mesas publicadas")]
    Finish(#[source] E),
}

/// Publica el plan en orden. `on_published` recibe cuántas mesas van, después de cada una.
pub fn run<P: Publisher, W: Pacer>(
    plan: &[Scheduled],
    candidacies: &BTreeMap<u32, Candidacy>,
    publisher: &mut P,
    pacer: &mut W,
    mut on_published: impl FnMut(usize),
) -> Result<Published, RunError<P::Error>> {
    let published = plan
        .iter()
        .try_fold(Published::default(), |published, scheduled| {
            let station = &scheduled.station;
            let message = station_message(station, candidacies);
            let key = message.key().to_owned();
            let (topic, payload) = match validate(&message) {
                Ok(()) => (Topic::Stations, serde_json::to_string(&message)),
                Err(rejection) => (
                    Topic::DeadLetter,
                    serde_json::to_string(&DeadLetterMessage::rejected(message, &rejection)),
                ),
            };
            let payload =
                payload.map_err(|error| RunError::Encode(Box::new(station.key.clone()), error))?;
            pacer.wait_until(scheduled.at);
            publisher
                .publish(topic, &key, &payload)
                .map_err(|error| RunError::Publish(Box::new(station.key.clone()), error))?;
            let published = published.add(topic);
            on_published(published.total());
            Ok(published)
        })?;
    publisher.finish().map_err(RunError::Finish)?;
    Ok(published)
}

/// Los avances que se muestran: cada 10 % de `total`. Devuelve el porcentaje cuando `count` cruza
/// uno.
pub fn progress(count: usize, total: usize) -> Option<usize> {
    let tenths = |n: usize| n * 10 / total.max(1);
    (count > 0 && tenths(count) > tenths(count - 1)).then(|| tenths(count) * 10)
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use super::*;
    use crate::domain::schedule::schedule;
    use crate::domain::station::Station;
    use crate::domain::station::fixtures::dataset;

    #[derive(Debug, PartialEq, Eq)]
    struct Refused;

    impl fmt::Display for Refused {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.pad("rechazado")
        }
    }

    impl Error for Refused {}

    /// Guarda lo publicado; puede rechazar la mesa número `fail_at` o el cierre.
    #[derive(Default)]
    struct FakePublisher {
        sent: Vec<(Topic, String, String)>,
        finished: bool,
        fail_at: Option<usize>,
        fail_finish: bool,
    }

    impl Publisher for FakePublisher {
        type Error = Refused;

        fn publish(&mut self, topic: Topic, key: &str, payload: &str) -> Result<(), Refused> {
            match self.fail_at {
                Some(at) if at == self.sent.len() => Err(Refused),
                _ => {
                    self.sent.push((topic, key.to_owned(), payload.to_owned()));
                    Ok(())
                }
            }
        }

        fn finish(&mut self) -> Result<(), Refused> {
            self.finished = true;
            if self.fail_finish {
                Err(Refused)
            } else {
                Ok(())
            }
        }
    }

    #[derive(Default)]
    struct FakePacer(Vec<Duration>);

    impl Pacer for FakePacer {
        fn wait_until(&mut self, at: Duration) {
            self.0.push(at);
        }
    }

    /// El plan de las tres mesas de la fixture; con `invalid`, la de la provincia 52 tiene más
    /// votantes que censo.
    fn plan(invalid: bool) -> (Vec<Scheduled>, BTreeMap<u32, Candidacy>) {
        let dataset = dataset();
        let stations = dataset
            .stations
            .into_iter()
            .map(|s| match (invalid, s.key.province.as_str()) {
                (true, "52") => Station { census: 10, ..s },
                _ => s,
            })
            .collect();
        (
            schedule(stations, Duration::from_secs(60), 1),
            dataset.candidacies,
        )
    }

    #[test]
    fn publishes_every_station_in_order_after_waiting_for_its_moment() {
        let (plan, candidacies) = plan(false);
        let (mut publisher, mut pacer) = (FakePublisher::default(), FakePacer::default());
        let mut counts = vec![];
        let published = run(&plan, &candidacies, &mut publisher, &mut pacer, |n| {
            counts.push(n)
        });
        assert_eq!(
            published.ok(),
            Some(Published {
                valid: 3,
                rejected: 0
            })
        );
        assert_eq!(counts, [1, 2, 3]);
        assert_eq!(pacer.0, plan.iter().map(|s| s.at).collect::<Vec<_>>());
        assert_eq!(
            publisher
                .sent
                .iter()
                .map(|(topic, key, _)| (*topic, key.as_str()))
                .collect::<Vec<_>>(),
            plan.iter()
                .map(|s| (Topic::Stations, s.station.key.province.as_str()))
                .collect::<Vec<_>>()
        );
        assert!(publisher.sent[0].2.starts_with(r#"{"province":"#));
        assert!(publisher.finished);
    }

    #[test]
    fn sends_invalid_stations_to_the_dead_letter_and_goes_on() {
        let (plan, candidacies) = plan(true);
        let mut publisher = FakePublisher::default();
        let published = run(
            &plan,
            &candidacies,
            &mut publisher,
            &mut FakePacer::default(),
            |_| {},
        );
        assert_eq!(
            published.ok(),
            Some(Published {
                valid: 2,
                rejected: 1
            })
        );
        let dead: Vec<_> = publisher
            .sent
            .iter()
            .filter(|(topic, ..)| *topic == Topic::DeadLetter)
            .collect();
        assert_eq!(dead.len(), 1);
        assert_eq!(dead[0].1, "52");
        assert!(
            dead[0].2.starts_with(r#"{"reason":"voters_over_census","#),
            "{}",
            dead[0].2
        );
    }

    #[test]
    fn stops_at_the_first_station_kafka_refuses() {
        let (plan, candidacies) = plan(false);
        let mut publisher = FakePublisher {
            fail_at: Some(1),
            ..FakePublisher::default()
        };
        let result = run(
            &plan,
            &candidacies,
            &mut publisher,
            &mut FakePacer::default(),
            |_| {},
        );
        assert!(
            matches!(&result, Err(RunError::Publish(key, Refused)) if **key == plan[1].station.key),
            "{result:?}"
        );
        assert_eq!(publisher.sent.len(), 1);
        assert!(!publisher.finished);
    }

    #[test]
    fn fails_when_kafka_does_not_confirm() {
        let (plan, candidacies) = plan(false);
        let mut publisher = FakePublisher {
            fail_finish: true,
            ..FakePublisher::default()
        };
        let result = run(
            &plan,
            &candidacies,
            &mut publisher,
            &mut FakePacer::default(),
            |_| {},
        );
        assert!(
            matches!(result, Err(RunError::Finish(Refused))),
            "{result:?}"
        );
        assert_eq!(
            RunError::Finish(Refused).to_string(),
            "no se confirmaron todas las mesas publicadas"
        );
    }

    #[test]
    fn reports_progress_every_ten_percent() {
        assert_eq!(
            (1..=20).filter_map(|n| progress(n, 20)).collect::<Vec<_>>(),
            [10, 20, 30, 40, 50, 60, 70, 80, 90, 100]
        );
        assert_eq!(progress(0, 20), None);
        assert_eq!(progress(1, 3), Some(30));
    }
}
