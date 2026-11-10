//! El binario `calendario`: la raíz de composición y el único sitio con efectos secundarios.
//!
//! Lee el reloj y el fichero, ejecuta el caso de uso, guarda el fichero si ha cambiado y
//! muestra el resultado: en la consola o, para `month`, `year` y `summary` en un terminal, en
//! una ventana a pantalla completa (RN-22).

use std::error::Error;
use std::io::IsTerminal;
use std::path::Path;
use std::process::ExitCode;

use clap::Parser;

use calendario_laboral::adapters::cli::Args;
use calendario_laboral::adapters::render::Marker;
use calendario_laboral::adapters::{json, render, viewer};
use calendario_laboral::application::{Command, Event, Outcome, Report, execute};

/// Termina con 1 si la operación falla y con 2 si los argumentos no son válidos (lo hace clap).
fn main() -> ExitCode {
    let args = Args::parse();
    let command = args.command.into_command(chrono::Local::now().date_naive());
    run(&args.file, command, args.no_tui).map_or_else(
        |error| {
            anstream::eprintln!("error: {error}");
            ExitCode::FAILURE
        },
        |()| ExitCode::SUCCESS,
    )
}

/// Solo guarda si el caso de uso ha ido bien, así una operación inválida deja el fichero
/// como estaba (RN-13).
fn run(path: &Path, command: Command, no_tui: bool) -> Result<(), Box<dyn Error>> {
    match execute(json::load(path)?, command)? {
        Outcome::Changed { calendar, event } => {
            json::save(path, &calendar)?;
            show(&event);
        }
        Outcome::Unchanged(event) => show(&event),
        Outcome::Report(report) => present(&report, no_tui)?,
    }
    Ok(())
}

/// La ventana si [`viewer::opens`] lo decide, y la consola si no.
fn present(report: &Report, no_tui: bool) -> std::io::Result<()> {
    let color = color();
    let text = render::report(report, Marker::for_color(color));
    if viewer::opens(report, no_tui, std::io::stdout().is_terminal()) {
        // Con `NO_COLOR`, sin los códigos de color, como haría anstream en la consola.
        viewer::run(&if color {
            text
        } else {
            anstream::adapter::strip_str(&text).to_string()
        })
    } else {
        anstream::print!("{text}");
        Ok(())
    }
}

/// Si stdout tendrá color: no al redirigir ni con `NO_COLOR`. Sin color, cada día lleva su letra
/// en lugar de la barra (RN-20).
fn color() -> bool {
    match anstream::AutoStream::choice(&std::io::stdout()) {
        anstream::ColorChoice::Never => false,
        anstream::ColorChoice::Auto
        | anstream::ColorChoice::AlwaysAnsi
        | anstream::ColorChoice::Always => true,
    }
}

fn show(event: &Event) {
    let message = render::event(event);
    anstream::println!("{}", message.text);
    if let Some(warning) = message.warning {
        anstream::eprintln!("{warning}");
    }
}
