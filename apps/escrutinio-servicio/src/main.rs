use std::error::Error;
use std::iter;
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use escrutinio_servicio::adapters::cli::{self, Args};
use escrutinio_servicio::adapters::http;
use escrutinio_servicio::adapters::kafka::{self, KafkaDeadLetters};
use escrutinio_servicio::adapters::postgres::{PgSnapshots, PgStore};

/// Sale con 1 si falla PostgreSQL, Kafka o la API, y con 0 al pulsar Ctrl+C; clap sale con 2 si los
/// argumentos no son válidos.
#[tokio::main]
async fn main() -> ExitCode {
    match run(Args::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {}", chain(error.as_ref()));
            ExitCode::FAILURE
        }
    }
}

async fn run(args: Args) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut store = PgStore::connect(&args.postgres).await?;
    let snapshots = Arc::new(PgSnapshots::connect(&args.postgres).await?);
    let consumer = kafka::consumer(&args.kafka, &args.grupo, &args.topic)?;
    let mut dead_letters = KafkaDeadLetters::new(&args.kafka, &args.topic_dlq)?;
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", args.puerto)).await?;
    println!(
        "Consumiendo {} de {} y sirviendo http://localhost:{}/snapshots",
        args.topic, args.kafka, args.puerto
    );
    let consuming = kafka::consume(&consumer, &mut store, &mut dead_letters, |tally| {
        if let Some(line) = cli::report(&tally) {
            println!("{line}");
        }
    });
    let serving = axum::serve(listener, http::router(snapshots)).into_future();
    tokio::select! {
        result = consuming => result.map_err(|error| error.into()),
        result = serving => result.map_err(|error| error.into()),
        result = tokio::signal::ctrl_c() => result.map_err(|error| error.into()),
    }
}

/// El error y sus causas: `a: b: c`.
fn chain(error: &(dyn Error + 'static)) -> String {
    iter::successors(Some(error), |&e| e.source())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ")
}
