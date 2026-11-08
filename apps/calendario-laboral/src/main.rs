//! El binario `calendario`: la raíz de composición y el único sitio con efectos secundarios.
//!
//! Lee el reloj y el fichero, ejecuta el caso de uso, guarda el fichero si ha cambiado y
//! muestra el resultado.

use std::error::Error;
use std::path::Path;
use std::process::ExitCode;

use clap::Parser;

use calendario_laboral::adapters::cli::Args;
use calendario_laboral::adapters::{json, render};
use calendario_laboral::application::{Command, Event, Outcome, execute};

/// Termina con 1 si la operación falla y con 2 si los argumentos no son válidos (lo hace clap).
fn main() -> ExitCode {
    let args = Args::parse();
    let command = args.command.into_command(chrono::Local::now().date_naive());
    run(&args.file, command).map_or_else(
        |error| {
            anstream::eprintln!("error: {error}");
            ExitCode::FAILURE
        },
        |()| ExitCode::SUCCESS,
    )
}

/// Solo guarda si el caso de uso ha ido bien, así una operación inválida deja el fichero
/// como estaba (RN-13).
fn run(path: &Path, command: Command) -> Result<(), Box<dyn Error>> {
    match execute(json::load(path)?, command)? {
        Outcome::Changed { calendar, event } => {
            json::save(path, &calendar)?;
            show(&event);
        }
        Outcome::Unchanged(event) => show(&event),
        Outcome::Report(report) => anstream::print!("{}", render::report(&report)),
    }
    Ok(())
}

fn show(event: &Event) {
    let message = render::event(event);
    anstream::println!("{}", message.text);
    if let Some(warning) = message.warning {
        anstream::eprintln!("{warning}");
    }
}
