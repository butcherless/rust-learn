//! Kafka (requisitos de escrutinio, «Kafka»): el consumidor de `escrutinio.mesas` y el productor
//! del dead letter, asíncronos, de rdkafka.
//!
//! El consumidor solo confirma lo procesado: `consume` guarda el offset de cada mensaje después de
//! su `COMMIT` en PostgreSQL (`enable.auto.offset.store=false`), y Kafka confirma lo guardado cada
//! 5 s. Si el servicio cae antes, vuelve a recibir esos mensajes y PostgreSQL los descarta. Una
//! confirmación por mensaje (`commit_message`) dejaba el consumidor parado tras unas 50.000. Sin
//! tests: necesita un broker, así que no tiene lógica propia más allá de ese bucle.

use std::time::Duration;

use escrutinio_contrato::DeadLetterMessage;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::error::KafkaError;
use rdkafka::message::Message;
use rdkafka::producer::{FutureProducer, FutureRecord};
use thiserror::Error;

use crate::application::{DeadLetters, HandleError, StationStore, Tally, handle};

/// Cuánto se espera a que Kafka acepte un mensaje del dead letter.
const SEND_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Error)]
pub enum KafkaServiceError {
    #[error("no se pudo conectar con Kafka en {0}")]
    Connect(String, #[source] KafkaError),
    #[error("no se pudo leer de Kafka")]
    Receive(#[source] KafkaError),
    #[error("no se pudo guardar el offset en Kafka")]
    StoreOffset(#[source] KafkaError),
    #[error("no se pudo publicar en el dead letter")]
    Send(#[source] KafkaError),
    #[error("no se pudo codificar el mensaje del dead letter")]
    Encode(#[source] serde_json::Error),
}

/// Un consumidor del grupo `group` suscrito a `topic`, desde el principio si el grupo es nuevo.
pub fn consumer(
    brokers: &str,
    group: &str,
    topic: &str,
) -> Result<StreamConsumer, KafkaServiceError> {
    let connect = |error| KafkaServiceError::Connect(brokers.to_owned(), error);
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("group.id", group)
        // Confirma cada 5 s solo los offsets que `consume` guarda tras procesar el mensaje.
        .set("enable.auto.commit", "true")
        .set("enable.auto.offset.store", "false")
        .set("auto.offset.reset", "earliest")
        .set("allow.auto.create.topics", "false")
        .create()
        .map_err(connect)?;
    consumer.subscribe(&[topic]).map_err(connect)?;
    Ok(consumer)
}

/// El productor del dead letter.
pub struct KafkaDeadLetters {
    producer: FutureProducer,
    topic: String,
}

impl KafkaDeadLetters {
    pub fn new(brokers: &str, topic: &str) -> Result<Self, KafkaServiceError> {
        let producer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("acks", "all")
            .set("enable.idempotence", "true")
            .set("allow.auto.create.topics", "false")
            .create()
            .map_err(|error| KafkaServiceError::Connect(brokers.to_owned(), error))?;
        Ok(Self {
            producer,
            topic: topic.to_owned(),
        })
    }
}

impl DeadLetters for KafkaDeadLetters {
    type Error = KafkaServiceError;

    async fn send(
        &mut self,
        key: Option<&str>,
        message: &DeadLetterMessage,
    ) -> Result<(), KafkaServiceError> {
        let payload = serde_json::to_string(message).map_err(KafkaServiceError::Encode)?;
        let record = FutureRecord::to(&self.topic).payload(&payload);
        let record = match key {
            Some(key) => record.key(key),
            None => record,
        };
        self.producer
            .send(record, SEND_TIMEOUT)
            .await
            .map(|_| ())
            .map_err(|(error, _)| KafkaServiceError::Send(error))
    }
}

#[derive(Debug, Error)]
pub enum ConsumeError<S: std::error::Error + 'static> {
    #[error(transparent)]
    Kafka(#[from] KafkaServiceError),
    #[error(transparent)]
    Handle(#[from] HandleError<S, KafkaServiceError>),
}

/// Procesa los mensajes uno a uno, sin fin: registra cada mesa, confirma su offset y avisa con
/// `on_tally`. Solo termina con un error.
pub async fn consume<S: StationStore>(
    consumer: &StreamConsumer,
    store: &mut S,
    dead_letters: &mut KafkaDeadLetters,
    mut on_tally: impl FnMut(Tally),
) -> Result<(), ConsumeError<S::Error>> {
    let mut tally = Tally::default();
    loop {
        let message = consumer.recv().await.map_err(KafkaServiceError::Receive)?;
        let outcome = handle(message.payload().unwrap_or_default(), store, dead_letters).await?;
        consumer
            .store_offset_from_message(&message)
            .map_err(KafkaServiceError::StoreOffset)?;
        tally = tally.with(outcome);
        on_tally(tally);
    }
}
