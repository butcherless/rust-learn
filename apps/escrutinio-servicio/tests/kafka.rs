//! El bucle de Kafka contra el Kafka y el PostgreSQL del compose (ver `common`): registra las mesas,
//! descarta las repetidas, anota las rechazadas y manda al dead letter lo que no se puede contar.

mod common;

use std::sync::Arc;
use std::time::Duration;

use escrutinio_contrato::DeadLetterMessage;
use escrutinio_servicio::adapters::kafka::{self, KafkaDeadLetters, KafkaTopics};
use escrutinio_servicio::adapters::postgres::{PgSnapshots, PgStore};
use escrutinio_servicio::application::{SnapshotSource, Tally, Wipe};
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
    // Una mesa con más votantes que censo: el servicio la rechaza.
    let invalid = common::json(&escrutinio_contrato::StationMessage {
        census: 10,
        ..common::station("51", "B")
    });
    for (key, payload) in [
        ("51", a.as_slice()),
        ("51", a.as_slice()),
        ("51", invalid.as_slice()),
        ("51", b"basura".as_slice()),
    ] {
        common::produce(&topic, key, payload).await;
    }

    let done = Arc::new(Notify::new());
    let mut last = Tally::default();
    let consuming = kafka::consume(&consumer, &mut store, &mut dead_letters, |tally| {
        last = tally;
        if tally.total() == 4 {
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
    .expect("four messages in time");
    assert_eq!(
        last,
        Tally {
            recorded: 1,
            duplicates: 1,
            dead_letters: 2
        }
    );

    let snapshots = PgSnapshots::connect(&url).await.expect("snapshots");
    let totals = snapshots.totals().await.expect("read");
    assert_eq!(
        totals
            .iter()
            .map(|t| (t.stations, t.rejected))
            .collect::<Vec<_>>(),
        [(1, 1)]
    );

    let reader: StreamConsumer =
        kafka::consumer(common::KAFKA, &common::unique("lector"), &dlq).expect("reader");
    let mut dead = vec![];
    for _ in 0..2 {
        let message = tokio::time::timeout(Duration::from_secs(60), reader.recv())
            .await
            .expect("dead letter in time")
            .expect("dead letter");
        let parsed: DeadLetterMessage =
            serde_json::from_slice(message.payload().unwrap_or_default()).expect("json");
        dead.push((message.key().map(<[u8]>::to_vec), parsed));
    }
    assert_eq!(
        dead.iter()
            .map(|(key, m)| (key.as_deref(), m.reason.as_str(), m.raw.as_deref()))
            .collect::<Vec<_>>(),
        [
            (Some(b"51".as_slice()), "voters_over_census", None),
            (None, "malformed", Some("basura")),
        ]
    );
    common::drop_topics(&[&topic, &dlq]).await;
    common::drop_schema(&url).await;
}

#[tokio::test]
async fn wipes_every_message_of_the_topics() {
    if !common::enabled("wipes_every_message_of_the_topics") {
        return;
    }
    let (topic, dlq) = (common::unique("mesas"), common::unique("dlq"));
    common::topics(&[&topic, &dlq]).await;
    for (target, key) in [(&topic, "51"), (&topic, "52"), (&dlq, "24")] {
        common::produce(target, key, b"{}").await;
    }
    let mut topics = KafkaTopics::new(common::KAFKA, &[&topic, &dlq]).expect("topics");
    assert_eq!(topics.wipe().await.expect("wiped"), 3);
    assert_eq!(topics.wipe().await.expect("wiped again"), 0);

    let missing = KafkaTopics::new(common::KAFKA, &["no-existe"])
        .expect("topics")
        .wipe()
        .await;
    assert_eq!(
        missing.map_err(|e| e.to_string()).err().as_deref(),
        Some("el topic no-existe no existe en Kafka")
    );
    common::drop_topics(&[&topic, &dlq]).await;
}
