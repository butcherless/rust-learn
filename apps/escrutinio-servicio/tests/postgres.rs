//! El adaptador de PostgreSQL contra el PostgreSQL del compose (ver `common`).

mod common;

use escrutinio_contrato::{CandidacyTotal, StationMessage, VoteMessage};
use escrutinio_servicio::adapters::postgres::{PgSnapshots, PgStore};
use escrutinio_servicio::application::{Recorded, SnapshotSource, StationStore, Wipe, snapshots};
use escrutinio_servicio::domain::snapshot::ProvinceTotals;

#[tokio::test]
async fn records_each_station_once_and_adds_it_to_its_province() {
    if !common::enabled("records_each_station_once_and_adds_it_to_its_province") {
        return;
    }
    let url = common::schema("store").await;
    let mut store = PgStore::connect(&url).await.expect("store");
    let snapshots = PgSnapshots::connect(&url).await.expect("snapshots");
    assert_eq!(snapshots.totals().await.expect("empty"), vec![]);

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
        snapshots.totals().await.expect("totals"),
        vec![
            ProvinceTotals {
                province: "28".to_owned(),
                stations: 1,
                rejected: 0,
                census: 100,
                voters: 53,
                blank: 1,
                invalid: 2,
                votes: vec![total(1, "AAA", 30), total(2, "BBB", 20)],
            },
            ProvinceTotals {
                province: "51".to_owned(),
                stations: 2,
                rejected: 0,
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
async fn notes_rejected_stations_once_and_calculates_the_snapshot() {
    if !common::enabled("notes_rejected_stations_once_and_calculates_the_snapshot") {
        return;
    }
    let url = common::schema("rejected").await;
    let mut store = PgStore::connect(&url).await.expect("store");
    let source = PgSnapshots::connect(&url).await.expect("snapshots");
    // Ceuta tiene 96 mesas: una válida, y de las otras 95 rechazamos 2 (una repetida).
    store
        .record(&common::station("51", "A"))
        .await
        .expect("recorded");
    for table in ["B", "C", "C"] {
        store
            .reject(&common::station("51", table), "voters_over_census")
            .await
            .expect("rejected");
    }
    // Una provincia con solo mesas rechazadas también cuenta.
    store
        .reject(&common::station("52", "A"), "voters_over_census")
        .await
        .expect("rejected");

    let totals = source.totals().await.expect("totals");
    assert_eq!(
        totals
            .iter()
            .map(|t| (t.province.as_str(), t.stations, t.rejected))
            .collect::<Vec<_>>(),
        [("51", 1, 2), ("52", 0, 1)]
    );

    let snapshots = snapshots(&source).await.expect("snapshots");
    let province = |code| snapshots.provinces.iter().find(|p| p.province == code);
    assert_eq!(snapshots.provinces.len(), 52);
    assert_eq!(
        province("51").map(|p| (p.stations, p.possible, p.rejected, p.complete)),
        Some((1, 94, 2, false))
    );
    assert_eq!(
        province("52").map(|p| (p.stations, p.possible, p.rejected)),
        Some((0, 86, 1))
    );
    assert_eq!(province("01").map(|p| p.possible), Some(427));
    assert_eq!((snapshots.total.stations, snapshots.total.rejected), (1, 3));
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
    store
        .reject(&common::station("51", "R"), "voters_over_census")
        .await
        .expect("rejected");
    // Cuenta las mesas y las rechazadas.
    assert_eq!(store.wipe().await.expect("wiped"), 3);
    let snapshots = PgSnapshots::connect(&url).await.expect("snapshots");
    assert_eq!(snapshots.totals().await.expect("read"), vec![]);
    assert_eq!(
        store
            .record(&common::station("51", "A"))
            .await
            .expect("again"),
        Recorded::New
    );
    common::drop_schema(&url).await;
}
