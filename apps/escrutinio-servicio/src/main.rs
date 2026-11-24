use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use escrutinio_servicio::adapters::cli::{self, Args, Command};
use escrutinio_servicio::adapters::http;
use escrutinio_servicio::adapters::kafka::{self, KafkaDeadLetters, KafkaTopics};
use escrutinio_servicio::adapters::postgres::{PgSnapshots, PgStore};
use escrutinio_servicio::application::wipe;

/// Sale con 1 si falla PostgreSQL, Kafka o la API, y con 0 al pulsar Ctrl+C; clap sale con 2 si los
/// argumentos no son válidos.
#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    let result = match args.command {
        Some(Command::Borrar) => delete(args).await,
        None => run(args).await,
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run(args: Args) -> anyhow::Result<()> {
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
        result = consuming => Ok(result?),
        result = serving => Ok(result?),
        result = tokio::signal::ctrl_c() => Ok(result?),
    }
}

/// Borra los mensajes de los dos topics y las mesas de PostgreSQL.
async fn delete(args: Args) -> anyhow::Result<()> {
    let mut topics = KafkaTopics::new(&args.kafka, &[&args.topic, &args.topic_dlq])?;
    let mut store = PgStore::connect(&args.postgres).await?;
    let wiped = wipe(&mut topics, &mut store).await?;
    println!("{}", cli::wiped(&wiped));
    Ok(())
}
