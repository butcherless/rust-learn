//! El bucle de Kafka contra el Kafka y el PostgreSQL del compose (ver `common`): registra las mesas,
//! descarta las repetidas y manda al dead letter lo que no es una mesa.

mod common;

use std::sync::Arc;
use std::time::Duration;

use escrutinio_contrato::DeadLetterMessage;
use escrutinio_servicio::adapters::kafka::{self, KafkaDeadLetters};
use escrutinio_servicio::adapters::postgres::{PgSnapshots, PgStore};
use escrutinio_servicio::application::{SnapshotSource, Tally};
use rdkafka::consumer::StreamConsumer;
use rdkafka::message::Message;
use tokio::sync::Notify;

#[tokio::test]
async fn consumes_stations_duplicates_and_garbage() {
    if !common::enabled("consumes_stations_duplicates_and_garbage") {
        return;
    }
    let (topic, dlq, group) = (
        common::unique("mesas"),
        common::unique("dlq"),
        common::unique("grupo"),
    );
    common::topics(&[&topic, &dlq]).await;
    let url = common::schema("consume").await;
    let mut store = PgStore::connect(&url).await.expect("store");
    let mut dead_letters = KafkaDeadLetters::new(common::KAFKA, &dlq).expect("dead letters");
    let consumer = kafka::consumer(common::KAFKA, &group, &topic).expect("consumer");

    let a = common::json(&common::station("51", "A"));
    for (key, payload) in [
        ("51", a.as_slice()),
        ("51", a.as_slice()),
        ("51", b"basura".as_slice()),
    ] {
        common::produce(&topic, key, payload).await;
    }

    let done = Arc::new(Notify::new());
    let mut last = Tally::default();
    let consuming = kafka::consume(&consumer, &mut store, &mut dead_letters, |tally| {
        last = tally;
        if tally.total() == 3 {
            done.notify_one();
        }
    });
    tokio::time::timeout(Duration::from_secs(60), async {
        tokio::select! {
            result = consuming => panic!("consume ended: {result:?}"),
            () = done.notified() => {}
        }
    })
    .await
    .expect("three messages in time");
    assert_eq!(
        last,
        Tally {
            recorded: 1,
            duplicates: 1,
            dead_letters: 1
        }
    );

    let snapshots = PgSnapshots::connect(&url).await.expect("snapshots");
    let stations: u32 = snapshots
        .snapshots()
        .await
        .expect("read")
        .iter()
        .map(|s| s.stations)
        .sum();
    assert_eq!(stations, 1);

    let reader: StreamConsumer =
        kafka::consumer(common::KAFKA, &common::unique("lector"), &dlq).expect("reader");
    let message = tokio::time::timeout(Duration::from_secs(60), reader.recv())
        .await
        .expect("dead letter in time")
        .expect("dead letter");
    let dead: DeadLetterMessage =
        serde_json::from_slice(message.payload().unwrap_or_default()).expect("json");
    assert_eq!(
        (dead.reason.as_str(), dead.raw.as_deref()),
        ("malformed", Some("basura"))
    );
    assert_eq!(message.key(), None);
}
