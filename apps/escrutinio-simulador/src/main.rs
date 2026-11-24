use std::process::ExitCode;

use clap::Parser;
use escrutinio_simulador::adapters::cli::{self, Args};
use escrutinio_simulador::adapters::clock::SleepPacer;
use escrutinio_simulador::adapters::interior;
use escrutinio_simulador::adapters::kafka::KafkaPublisher;
use escrutinio_simulador::application::{self, progress};
use escrutinio_simulador::domain::schedule::schedule;
use escrutinio_simulador::domain::station::build;

/// Sale con 1 si fallan los ficheros o Kafka; clap sale con 2 si los argumentos no son válidos.
fn main() -> ExitCode {
    run(Args::parse()).map_or_else(
        |error| {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        },
        |()| ExitCode::SUCCESS,
    )
}

fn run(args: Args) -> anyhow::Result<()> {
    let files = interior::read(&args.datos)?;
    let dataset = build(
        files.stations,
        files.votes,
        files.candidacies,
        &files.expected,
    )?;
    println!("{}", cli::checked(&dataset.summary()));
    if args.solo_comprobar {
        return Ok(());
    }
    let plan = schedule(dataset.stations, args.duracion, args.semilla);
    let mut publisher = KafkaPublisher::connect(&args.kafka, &args.topic)?;
    println!(
        "Publicando en {} ({}) durante {}…",
        args.topic,
        args.kafka,
        cli::clock(args.duracion)
    );
    let mut pacer = SleepPacer::start();
    let total = plan.len();
    let published = application::run(
        &plan,
        &dataset.candidacies,
        &mut publisher,
        &mut pacer,
        |n| {
            if let Some(percent) = progress(n, total) {
                println!("{percent:>3} % · {} mesas", cli::thousands(n as u64));
            }
        },
    )?;
    println!("{}", cli::published(published, pacer.elapsed()));
    Ok(())
}
