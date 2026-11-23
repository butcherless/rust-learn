use std::error::Error;
use std::iter;
use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use escrutinio::census::{Election, ordered};
use escrutinio::cli::Args;
use escrutinio::feed::{self, REFRESH};
use escrutinio::mock::Scrutiny;
use escrutinio::tui::{
    self,
    app::{App, Message},
};

/// Sale con 1 si falla la terminal o el cliente HTTP; clap sale con 2 si los argumentos no son
/// válidos. Que el servicio no responda no es un error: la ventana lo muestra y reintenta.
fn main() -> ExitCode {
    run(Args::parse()).map_or_else(
        |error| {
            eprintln!("error: {}", chain(error.as_ref()));
            ExitCode::FAILURE
        },
        |()| ExitCode::SUCCESS,
    )
}

fn run(args: Args) -> Result<(), Box<dyn Error>> {
    match args.servicio {
        None => {
            let districts = ordered(Election::Generales2026);
            let scrutiny = Scrutiny::start(&districts, args.participacion, args.semilla);
            let interval = Duration::from_millis(args.intervalo);
            tui::run(App::new(scrutiny), interval, || Message::Tick)?;
        }
        Some(url) => {
            let client = feed::client()?;
            let fetch =
                || Message::Snapshots(feed::fetch(&client, &url).map_err(|error| chain(&error)));
            let districts = ordered(Election::Generales2023);
            let scrutiny = Scrutiny::start(&districts, args.participacion, args.semilla);
            let (app, _) = App::following(scrutiny, &url).update(fetch());
            tui::run(app, REFRESH, fetch)?;
        }
    }
    Ok(())
}

/// El error y sus causas: `a: b: c`.
fn chain(error: &(dyn Error + 'static)) -> String {
    iter::successors(Some(error), |&e| e.source())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ")
}
