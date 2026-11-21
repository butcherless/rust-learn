//! El binario `calendario`: la raíz de composición y el único sitio con efectos secundarios.
//!
//! Lee el reloj y el fichero, ejecuta el caso de uso, guarda el fichero si ha cambiado y
//! muestra el resultado: en la consola o, para `month`, `year` y `summary` en un terminal, en
//! una ventana a pantalla completa (RN-22), que puede volver a leer el fichero (RN-28). En la
//! ventana, `month` y `year` piden además los embalses de Madrid (RN-31) y los mercados (RN-32).
//!
//! - `present` elige la ventana si `viewer::opens` lo decide, y si no imprime con anstream. Con
//!   `NO_COLOR`, quita los códigos de color antes de pasar el texto a la ventana (`color` mira
//!   `anstream::AutoStream::choice`). Para la recarga le pasa `query`, que vuelve a leer el fichero
//!   y ejecuta otra vez el mismo `Command`.
//! - Si `viewer::shows_panel`, crea el `ApiClient` y el `ExpansionClient` y lanza las peticiones
//!   con `panel::spawn`; si alguno no se puede crear, la ventana va sin embalses ni mercados. La
//!   consola nunca usa la red, así que `tests/cli.rs` tampoco.
//! - Códigos de salida: 1 si la operación falla y 2 si los argumentos no son válidos
//!   (`argument_error`, con `Args::try_parse`). Los avisos van a stderr.

use std::error::Error;
use std::io::IsTerminal;
use std::path::Path;
use std::process::ExitCode;

use clap::Parser;

use calendario_laboral::adapters::cli::{Args, error_text};
use calendario_laboral::adapters::markets::{ExpansionClient, MARKETS_URL};
use calendario_laboral::adapters::panel::{self, Panel};
use calendario_laboral::adapters::render::Marker;
use calendario_laboral::adapters::reservoirs::{API_URL, ApiClient};
use calendario_laboral::adapters::viewer::Output;
use calendario_laboral::adapters::{json, render, viewer};
use calendario_laboral::application::{Command, Event, Outcome, Report, execute};
use calendario_laboral::domain::MADRID_RESERVOIRS;

/// Termina con 1 si la operación falla y con 2 si los argumentos no son válidos.
fn main() -> ExitCode {
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error) => return argument_error(&error),
    };
    let output = args.output(std::io::stdout().is_terminal());
    let command = args.command.into_command(jiff::ZonedDateTime::now().date());
    run(&args.file, command, output).map_or_else(
        |error| {
            anstream::eprintln!("error: {error}");
            ExitCode::FAILURE
        },
        |()| ExitCode::SUCCESS,
    )
}

/// Muestra el error de argumentos o la ayuda en español, o la versión como la da clap, y
/// devuelve su código de salida: 2 para los errores y 0 para la ayuda (RNF-07).
fn argument_error(error: &clap::Error) -> ExitCode {
    match error_text(error) {
        Some(text) if error.use_stderr() => anstream::eprint!("{text}"),
        Some(text) => anstream::print!("{text}"),
        None => error.print().unwrap_or(()),
    }
    ExitCode::from(u8::try_from(error.exit_code()).unwrap_or(2))
}

/// Solo guarda si el caso de uso ha ido bien, así una operación inválida deja el fichero
/// como estaba (RN-13).
fn run(path: &Path, command: Command, output: Output) -> Result<(), Box<dyn Error>> {
    match execute(json::load(path)?, command.clone())? {
        Outcome::Changed { calendar, event } => {
            json::save(path, &calendar)?;
            show(&event);
        }
        Outcome::Unchanged(event) => show(&event),
        Outcome::Report(report) => present(&report, output, || query(path, command.clone()))?,
    }
    Ok(())
}

/// Vuelve a leer el fichero y a lanzar la consulta, para la tecla `r` de la ventana (RN-28).
fn query(path: &Path, command: Command) -> Result<Report, Box<dyn Error>> {
    match execute(json::load(path)?, command)? {
        Outcome::Report(report) => Ok(report),
        Outcome::Changed { .. } | Outcome::Unchanged(_) => {
            Err("la consulta ha cambiado el calendario".into())
        }
    }
}

/// La ventana si [`viewer::opens`] lo decide, y la consola si no. En la ventana, `reload` da el
/// informe con el fichero tal como esté al pulsar `r` (RN-28), y las vistas de mes y de año piden
/// los embalses y los mercados en otro hilo (RN-31, RN-32); si algún cliente no se puede crear,
/// van sin ellos.
fn present(
    report: &Report,
    output: Output,
    reload: impl Fn() -> Result<Report, Box<dyn Error>>,
) -> std::io::Result<()> {
    let color = color();
    let marker = Marker::for_color(color);
    if viewer::opens(report, output) {
        // Con `NO_COLOR`, sin los códigos de color, como haría anstream en la consola.
        let window_text = |report: &Report, panel: Option<&Panel>| {
            let text = render::report_with(report, marker, panel);
            if color {
                text
            } else {
                anstream::adapter::strip_str(&text).to_string()
            }
        };
        let fetched = viewer::shows_panel(report)
            .then(|| {
                Some((
                    ApiClient::new(API_URL).ok()?,
                    ExpansionClient::new(MARKETS_URL).ok()?,
                ))
            })
            .flatten()
            .map(|(reservoirs, markets)| panel::spawn(reservoirs, markets, &MADRID_RESERVOIRS));
        viewer::run(
            report.clone(),
            window_text,
            || reload().map_err(|error| error.to_string()),
            fetched,
        )
    } else {
        anstream::print!("{}", render::report(report, marker));
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
