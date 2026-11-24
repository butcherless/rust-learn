//! El adaptador de PostgreSQL contra el PostgreSQL del compose (ver `common`).

mod common;

use escrutinio_contrato::{CandidacyTotal, ProvinceSnapshot, StationMessage, VoteMessage};
use escrutinio_servicio::adapters::postgres::{PgSnapshots, PgStore};
use escrutinio_servicio::application::{Recorded, SnapshotSource, StationStore, Wipe};

#[tokio::test]
async fn records_each_station_once_and_adds_it_to_its_province() {
    if !common::enabled("records_each_station_once_and_adds_it_to_its_province") {
        return;
    }
    let url = common::schema("store").await;
    let mut store = PgStore::connect(&url).await.expect("store");
    let snapshots = PgSnapshots::connect(&url).await.expect("snapshots");
    assert_eq!(snapshots.snapshots().await.expect("empty"), vec![]);

    let b = StationMessage {
        votes: vec![
            VoteMessage {
                candidacy: 2,
                acronym: "BBB".to_owned(),
                votes: 40,
            },
            VoteMessage {
                candidacy: 1,
                acronym: "AAA".to_owned(),
                votes: 10,
            },
        ],
        ..common::station("51", "B")
    };
    let outcomes = [
        store.record(&common::station("51", "A")).await,
        store.record(&common::station("51", "A")).await,
        store.record(&b).await,
        store.record(&common::station("28", "U")).await,
    ]
    .map(|result| result.expect("recorded"));
    assert_eq!(
        outcomes,
        [
            Recorded::New,
            Recorded::Duplicate,
            Recorded::New,
            Recorded::New
        ]
    );

    let total = |candidacy, acronym: &str, votes| CandidacyTotal {
        candidacy,
        acronym: acronym.to_owned(),
        votes,
    };
    assert_eq!(
        snapshots.snapshots().await.expect("snapshots"),
        vec![
            ProvinceSnapshot {
                province: "28".to_owned(),
                stations: 1,
                census: 100,
                voters: 53,
                blank: 1,
                invalid: 2,
                votes: vec![total(1, "AAA", 30), total(2, "BBB", 20)],
            },
            ProvinceSnapshot {
                province: "51".to_owned(),
                stations: 2,
                census: 200,
                voters: 106,
                blank: 2,
                invalid: 4,
                votes: vec![total(2, "BBB", 60), total(1, "AAA", 40)],
            },
        ]
    );
    common::drop_schema(&url).await;
}

#[tokio::test]
async fn fails_to_connect_to_a_closed_port() {
    let error = PgStore::connect("postgres://nadie@127.0.0.1:1/nada")
        .await
        .err()
        .map(|e| e.to_string());
    assert_eq!(error.as_deref(), Some("no se pudo conectar con PostgreSQL"));
}

#[tokio::test]
async fn wipes_the_stations_and_the_snapshots() {
    if !common::enabled("wipes_the_stations_and_the_snapshots") {
        return;
    }
    let url = common::schema("wipe").await;
    let mut store = PgStore::connect(&url).await.expect("store");
    for table in ["A", "B"] {
        store
            .record(&common::station("51", table))
            .await
            .expect("recorded");
    }
    assert_eq!(store.wipe().await.expect("wiped"), 2);
    let snapshots = PgSnapshots::connect(&url).await.expect("snapshots");
    assert_eq!(snapshots.snapshots().await.expect("read"), vec![]);
    assert_eq!(
        store
            .record(&common::station("51", "A"))
            .await
            .expect("again"),
        Recorded::New
    );
    common::drop_schema(&url).await;
}
