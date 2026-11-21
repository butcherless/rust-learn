use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use escrutinio::census::ordered;
use escrutinio::cli::Args;
use escrutinio::mock::Scrutiny;
use escrutinio::tui::{self, app::App};

/// Sale con 1 si falla la terminal; clap sale con 2 si los argumentos no son válidos.
fn main() -> ExitCode {
    let args = Args::parse();
    let scrutiny = Scrutiny::start(&ordered(), args.participacion, args.semilla);
    tui::run(App::new(scrutiny), Duration::from_millis(args.intervalo)).map_or_else(
        |error| {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        },
        |()| ExitCode::SUCCESS,
    )
}
