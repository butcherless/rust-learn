//! Lo compartido por los tests de integración. Usan el Kafka y el PostgreSQL de
//! `compose/escrutinio.yaml` y solo se ejecutan con `ESCRUTINIO_INTEGRACION=1`; sin ella se saltan.
//! Cada test crea sus propios topics, grupo y esquema, así que pueden ir en paralelo y repetirse.

#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use escrutinio_contrato::{StationMessage, VoteMessage};
use rdkafka::ClientConfig;
use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::client::DefaultClientContext;
use rdkafka::producer::{FutureProducer, FutureRecord};

pub const KAFKA: &str = "localhost:9092";
pub const POSTGRES: &str = "postgres://escrutinio:escrutinio@localhost:5432/escrutinio";

/// `true` si hay que ejecutar los tests de integración. Si no, avisa por stderr.
pub fn enabled(test: &str) -> bool {
    let on = std::env::var("ESCRUTINIO_INTEGRACION").is_ok_and(|value| value == "1");
    if !on {
        eprintln!("{test}: omitido, necesita ESCRUTINIO_INTEGRACION=1 y compose/escrutinio.yaml");
    }
    on
}

/// Un nombre que no se repite ni entre ejecuciones ni entre tests en paralelo:
/// `it_<nombre>_<nanosegundos>_<contador>`.
pub fn unique(name: &str) -> String {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("it_{name}_{nanos}_{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

/// Crea un esquema nuevo y devuelve la URL de PostgreSQL que lo usa.
pub async fn schema(name: &str) -> String {
    let schema = unique(name);
    let (client, connection) = tokio_postgres::connect(POSTGRES, tokio_postgres::NoTls)
        .await
        .expect("PostgreSQL del compose");
    tokio::spawn(connection);
    client
        .batch_execute(&format!("CREATE SCHEMA {schema}"))
        .await
        .expect("create schema");
    format!("{POSTGRES}?options=-c%20search_path%3D{schema}")
}

/// Crea topics nuevos de una partición.
pub async fn topics(names: &[&str]) {
    let admin: AdminClient<DefaultClientContext> = ClientConfig::new()
        .set("bootstrap.servers", KAFKA)
        .create()
        .expect("admin client");
    let topics: Vec<NewTopic> = names
        .iter()
        .map(|name| NewTopic::new(name, 1, TopicReplication::Fixed(1)))
        .collect();
    admin
        .create_topics(&topics, &AdminOptions::new())
        .await
        .expect("create topics")
        .into_iter()
        .for_each(|result| {
            result.expect("topic created");
        });
}

/// Publica un mensaje y espera a que Kafka lo confirme.
pub async fn produce(topic: &str, key: &str, payload: &[u8]) {
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", KAFKA)
        .create()
        .expect("producer");
    producer
        .send(
            FutureRecord::to(topic).key(key).payload(payload),
            Duration::from_secs(10),
        )
        .await
        .expect("produced");
}

/// Una mesa con 100 de censo, 1 blanco, 2 nulos y 30 + 20 votos a las candidaturas 1 y 2.
pub fn station(province: &str, table: &str) -> StationMessage {
    StationMessage {
        province: province.to_owned(),
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
    serde_json::to_vec(station).expect("json")
}
