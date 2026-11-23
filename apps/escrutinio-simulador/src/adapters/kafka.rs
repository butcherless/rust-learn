//! El productor de Kafka (requisitos de escrutinio, «Kafka»): `BaseProducer` síncrono de
//! rdkafka, con `acks=all` y la idempotencia activada.
//!
//! No tiene tests: necesita un broker. Por eso solo traduce el puerto `Publisher` a rdkafka, sin
//! lógica propia. Al conectar pide los metadatos del topic, así que un Kafka caído o un topic que no
//! existe fallan al principio y no tras horas de publicar.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use rdkafka::ClientContext;
use rdkafka::config::ClientConfig;
use rdkafka::error::KafkaError;
use rdkafka::producer::{BaseProducer, BaseRecord, DeliveryResult, Producer, ProducerContext};
use thiserror::Error;

use crate::application::{Publisher, Topic};

/// Cuánto se espera a Kafka al conectar y al confirmar lo publicado.
const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Error)]
pub enum KafkaPublishError {
    #[error("no se pudo conectar con Kafka en {0}")]
    Connect(String, #[source] KafkaError),
    #[error("el topic {0} no existe en Kafka")]
    NoTopic(String),
    #[error("Kafka no aceptó la mesa")]
    Send(#[source] KafkaError),
    #[error("Kafka no confirmó las mesas a tiempo")]
    Flush(#[source] KafkaError),
    #[error("Kafka rechazó {0} mesas")]
    Undelivered(usize),
}

/// Cuenta los mensajes que Kafka no pudo entregar.
#[derive(Default)]
struct Deliveries {
    failed: AtomicUsize,
}

impl ClientContext for Deliveries {}

impl ProducerContext for Deliveries {
    type DeliveryOpaque = ();

    fn delivery(&self, result: &DeliveryResult<'_>, _: ()) {
        if result.is_err() {
            self.failed.fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub struct KafkaPublisher {
    producer: BaseProducer<Deliveries>,
    stations: String,
    dead_letter: String,
}

impl KafkaPublisher {
    /// Conecta con `brokers` y comprueba que existen el topic de mesas y el del dead letter.
    pub fn connect(
        brokers: &str,
        stations: &str,
        dead_letter: &str,
    ) -> Result<Self, KafkaPublishError> {
        let connect = |error| KafkaPublishError::Connect(brokers.to_owned(), error);
        let producer: BaseProducer<Deliveries> = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("acks", "all")
            .set("enable.idempotence", "true")
            // Si el topic no existe, error: no se crea con la configuración por defecto.
            .set("allow.auto.create.topics", "false")
            .create_with_context(Deliveries::default())
            .map_err(connect)?;
        [stations, dead_letter].into_iter().try_for_each(|topic| {
            let metadata = producer
                .client()
                .fetch_metadata(Some(topic), TIMEOUT)
                .map_err(connect)?;
            metadata
                .topics()
                .iter()
                .find(|t| t.name() == topic && t.error().is_none() && !t.partitions().is_empty())
                .map(|_| ())
                .ok_or_else(|| KafkaPublishError::NoTopic(topic.to_owned()))
        })?;
        Ok(Self {
            producer,
            stations: stations.to_owned(),
            dead_letter: dead_letter.to_owned(),
        })
    }
}

impl Publisher for KafkaPublisher {
    type Error = KafkaPublishError;

    fn publish(&mut self, topic: Topic, key: &str, payload: &str) -> Result<(), KafkaPublishError> {
        let topic = match topic {
            Topic::Stations => &self.stations,
            Topic::DeadLetter => &self.dead_letter,
        };
        self.producer
            .send(BaseRecord::to(topic).key(key).payload(payload))
            .map_err(|(error, _)| KafkaPublishError::Send(error))?;
        // Atiende las confirmaciones pendientes sin esperar.
        self.producer.poll(Duration::ZERO);
        Ok(())
    }

    fn finish(&mut self) -> Result<(), KafkaPublishError> {
        self.producer
            .flush(TIMEOUT)
            .map_err(KafkaPublishError::Flush)?;
        match self.producer.context().failed.load(Ordering::Relaxed) {
            0 => Ok(()),
            failed => Err(KafkaPublishError::Undelivered(failed)),
        }
    }
}
