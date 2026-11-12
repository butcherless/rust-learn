//! Adaptador de entrada: los argumentos de la línea de comandos (§5).
//!
//! Se convierten en un [`Command`] de la aplicación.

use std::path::PathBuf;

use chrono::{Datelike, NaiveDate};
use clap::builder::{PossibleValue, PossibleValuesParser, TypedValueParser};
use clap::{ArgAction, Parser, Subcommand};

use crate::adapters::render::month_name;
use crate::application::Command;
use crate::domain::{DomainError, MarkId, MarkKind, Month, MonthExt, Shift, parse_date};

/// La plantilla de la ayuda, con encabezados en español en lugar de los de clap.
const HELP: &str = "{about-with-newline}\nUso: {usage}\n\n{all-args}{after-help}";

/// Gestiona el calendario laboral de uno o varios años.
#[derive(Debug, Parser)]
#[command(
    name = "calendario",
    version,
    help_template = HELP,
    next_help_heading = "Opciones",
    disable_help_flag = true,
    disable_version_flag = true,
    disable_help_subcommand = true,
    subcommand_help_heading = "Comandos",
    subcommand_value_name = "COMANDO"
)]
pub struct Args {
    /// Fichero JSON del calendario.
    #[arg(
        long,
        global = true,
        env = "CALENDARIO_FILE",
        value_name = "RUTA",
        default_value = "calendario.json"
    )]
    pub file: PathBuf,

    /// Imprime `month`, `year` y `summary` en la consola en lugar de abrir la ventana (RN-22).
    #[arg(long, global = true)]
    pub no_tui: bool,

    /// Muestra la ayuda.
    #[arg(short, long, global = true, action = ArgAction::Help)]
    help: Option<bool>,

    /// Muestra la versión.
    #[arg(short = 'V', long, action = ArgAction::Version)]
    version: Option<bool>,

    /// La operación.
    #[command(subcommand)]
    pub command: CommandArg,
}

/// Los subcomandos.
#[derive(Debug, Subcommand)]
pub enum CommandArg {
    /// Añade un año al calendario.
    #[command(help_template = HELP, next_help_heading = "Argumentos")]
    AddYear {
        /// Año, por ejemplo 2027.
        #[arg(value_name = "AÑO")]
        year: i32,
    },
    /// Marca un día o un rango de días.
    #[command(help_template = HELP, next_help_heading = "Argumentos")]
    Mark {
        /// Tipo de marca.
        #[arg(value_name = "TIPO", value_parser = mark_kind_arg(), display_order = 1)]
        kind: MarkKind,
        /// Día, o primer día del rango (AAAA-MM-DD).
        #[arg(value_name = "FECHA", value_parser = date_arg, display_order = 2)]
        from: NaiveDate,
        /// Último día del rango (AAAA-MM-DD).
        #[arg(value_name = "HASTA", value_parser = date_arg, display_order = 3)]
        to: Option<NaiveDate>,
        /// Descripción de la marca.
        #[arg(long = "desc", value_name = "TEXTO", help_heading = "Opciones")]
        description: Option<String>,
    },
    /// Elimina una marca.
    #[command(help_template = HELP, next_help_heading = "Argumentos")]
    Unmark {
        /// Id de la marca, como lo muestra «list».
        #[arg(value_name = "ID")]
        id: u32,
    },
    /// Lista las marcas de todos los años o de uno.
    #[command(help_template = HELP, next_help_heading = "Argumentos")]
    List {
        /// Solo las marcas de este año.
        #[arg(long, value_name = "AÑO", help_heading = "Opciones")]
        year: Option<i32>,
    },
    /// Muestra el tipo y las horas de un día.
    #[command(help_template = HELP, next_help_heading = "Argumentos")]
    Day {
        /// Fecha (AAAA-MM-DD).
        #[arg(value_name = "FECHA", value_parser = date_arg)]
        date: NaiveDate,
    },
    /// Muestra un mes [por defecto: el actual].
    #[command(help_template = HELP, next_help_heading = "Argumentos")]
    Month {
        /// Año.
        #[arg(value_name = "AÑO", requires = "month", display_order = 1)]
        year: Option<i32>,
        /// Mes, por nombre o número: junio o 6.
        #[arg(
            value_name = "MES",
            value_parser = month_arg(),
            display_order = 2,
            hide_possible_values = true
        )]
        month: Option<Month>,
    },
    /// Muestra un año completo [por defecto: el actual].
    #[command(help_template = HELP, next_help_heading = "Argumentos")]
    Year {
        /// Año.
        #[arg(value_name = "AÑO")]
        year: Option<i32>,
    },
    /// Resume un año [por defecto: el actual].
    #[command(help_template = HELP, next_help_heading = "Argumentos")]
    Summary {
        /// Año.
        #[arg(value_name = "AÑO")]
        year: Option<i32>,
    },
    /// Configura las horas de una jornada en un año.
    #[command(help_template = HELP, next_help_heading = "Argumentos")]
    Hours {
        /// Año, con sus propias horas (RN-25).
        #[arg(value_name = "AÑO", display_order = 1)]
        year: i32,
        /// Jornada.
        #[arg(value_name = "JORNADA", value_parser = shift_arg(), display_order = 2)]
        shift: Shift,
        /// Horas, con coma o punto decimal: 8,5 u 8.5.
        #[arg(value_name = "HORAS", value_parser = hours_arg, display_order = 3)]
        hours: f64,
    },
    /// Configura el cupo de vacaciones de un año.
    #[command(help_template = HELP, next_help_heading = "Argumentos")]
    Quota {
        /// Año.
        #[arg(value_name = "AÑO", display_order = 1)]
        year: i32,
        /// Días de vacaciones.
        #[arg(value_name = "DÍAS", display_order = 2)]
        days: u32,
    },
}

/// [`MarkKind`] como valor de la línea de comandos, para que el dominio no dependa de clap.
/// Un valor de una lista cerrada: clap comprueba el texto (`possible_values`) y lo convierte en su
/// valor del dominio. Así los enums del dominio no dependen de clap y sus nombres no se repiten.
fn choice<T>(
    choices: impl IntoIterator<Item = (T, PossibleValue)>,
) -> impl TypedValueParser<Value = T>
where
    T: Copy + Send + Sync + 'static,
{
    let choices: Vec<(T, PossibleValue)> = choices.into_iter().collect();
    PossibleValuesParser::new(choices.iter().map(|(_, value)| value.clone())).try_map(move |text| {
        choices
            .iter()
            .find(|(_, value)| value.matches(&text, false))
            .map(|(choice, _)| *choice)
            .ok_or_else(|| format!("«{text}» no es un valor válido"))
    })
}

/// `reducido`, `festivo` o `vacaciones`, los nombres de `MarkKind`.
fn mark_kind_arg() -> impl TypedValueParser<Value = MarkKind> {
    choice(MarkKind::ALL.map(|kind| (kind, PossibleValue::new(kind.name()))))
}

/// `laborable`, `reducido`, `laborable-verano` o `reducido-verano`, los nombres de `Shift`.
fn shift_arg() -> impl TypedValueParser<Value = Shift> {
    choice(Shift::ALL.map(|shift| (shift, PossibleValue::new(shift.name()))))
}

/// El nombre del mes en minúsculas, o su número con o sin cero (`6`, `06`); también `setiembre`.
fn month_arg() -> impl TypedValueParser<Value = Month> {
    choice(Month::ALL.map(|month| {
        let number = month.number_from_month();
        let value = PossibleValue::new(month_name(month).to_lowercase())
            .aliases([number.to_string(), format!("{number:02}")]);
        let value = if month == Month::September {
            value.alias("setiembre")
        } else {
            value
        };
        (month, value)
    }))
}

fn date_arg(text: &str) -> Result<NaiveDate, DomainError> {
    parse_date(text)
}

/// Lee horas escritas con coma o punto decimal (RN-15).
fn hours_arg(text: &str) -> Result<f64, String> {
    text.replace(',', ".")
        .parse::<f64>()
        .map_err(|_| format!("«{text}» no es un número de horas"))
}

impl CommandArg {
    /// El [`Command`] de la aplicación; `today` da el año y el mes que falten.
    pub fn into_command(self, today: NaiveDate) -> Command {
        match self {
            Self::AddYear { year } => Command::AddYear(year),
            Self::Mark {
                kind,
                from,
                to,
                description,
            } => Command::Mark {
                kind,
                from,
                to,
                description,
            },
            Self::Unmark { id } => Command::Unmark(MarkId(id)),
            Self::List { year } => Command::List(year),
            Self::Day { date } => Command::Day(date),
            Self::Month { year, month } => Command::Month {
                year: year.unwrap_or(today.year()),
                month: month.unwrap_or_else(|| Month::of(today)),
            },
            Self::Year { year } => Command::Year(year.unwrap_or(today.year())),
            Self::Summary { year } => Command::Summary(year.unwrap_or(today.year())),
            Self::Hours { year, shift, hours } => Command::Hours { year, shift, hours },
            Self::Quota { year, days } => Command::Quota { year, days },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::date;
    use clap::error::ErrorKind;

    fn parse(args: &[&str]) -> Result<(PathBuf, Command), ErrorKind> {
        Args::try_parse_from(std::iter::once("calendario").chain(args.iter().copied()))
            .map(|args| (args.file, args.command.into_command(date(2026, 11, 8))))
            .map_err(|error| error.kind())
    }

    fn command(args: &[&str]) -> Result<Command, ErrorKind> {
        parse(args).map(|(_, command)| command)
    }

    #[test]
    fn rf_01_to_rf_10_parse_the_subcommands_of_the_table() {
        [
            (&["add-year", "2027"][..], Command::AddYear(2027)),
            (
                &["mark", "vacaciones", "2026-06-05", "2026-06-12"][..],
                Command::Mark {
                    kind: MarkKind::Vacation,
                    from: date(2026, 6, 5),
                    to: Some(date(2026, 6, 12)),
                    description: None,
                },
            ),
            (
                &["mark", "festivo", "2026-03-03", "--desc", "Fiesta local"][..],
                Command::Mark {
                    kind: MarkKind::Holiday,
                    from: date(2026, 3, 3),
                    to: None,
                    description: Some("Fiesta local".to_owned()),
                },
            ),
            (
                &["mark", "reducido", "2026-12-24"][..],
                Command::Mark {
                    kind: MarkKind::Reduced,
                    from: date(2026, 12, 24),
                    to: None,
                    description: None,
                },
            ),
            (&["unmark", "4"][..], Command::Unmark(MarkId(4))),
            (&["list"][..], Command::List(None)),
            (&["list", "--year", "2026"][..], Command::List(Some(2026))),
            (&["day", "2026-06-08"][..], Command::Day(date(2026, 6, 8))),
            (
                &["month", "2026", "6"][..],
                Command::Month {
                    year: 2026,
                    month: Month::June,
                },
            ),
            (&["year", "2027"][..], Command::Year(2027)),
            (&["summary", "2025"][..], Command::Summary(2025)),
            (
                &["hours", "2026", "laborable-verano", "8.5"][..],
                Command::Hours {
                    year: 2026,
                    shift: Shift::SummerWorking,
                    hours: 8.5,
                },
            ),
            (
                &["hours", "2026", "laborable", "7,25"][..],
                Command::Hours {
                    year: 2026,
                    shift: Shift::Working,
                    hours: 7.25,
                },
            ),
            (
                &["hours", "2026", "reducido", "6"][..],
                Command::Hours {
                    year: 2026,
                    shift: Shift::Reduced,
                    hours: 6.0,
                },
            ),
            (
                &["hours", "2027", "reducido-verano", "5"][..],
                Command::Hours {
                    year: 2027,
                    shift: Shift::SummerReduced,
                    hours: 5.0,
                },
            ),
            (
                &["quota", "2026", "25"][..],
                Command::Quota {
                    year: 2026,
                    days: 25,
                },
            ),
        ]
        .into_iter()
        .for_each(|(args, expected)| assert_eq!(command(args), Ok(expected), "{args:?}"));
    }

    #[test]
    fn rf_06_months_are_given_by_spanish_name_or_number() {
        [
            ("enero", Month::January),
            ("1", Month::January),
            ("01", Month::January),
            ("junio", Month::June),
            ("09", Month::September),
            ("setiembre", Month::September),
            ("12", Month::December),
            ("diciembre", Month::December),
        ]
        .into_iter()
        .for_each(|(text, month)| {
            assert_eq!(
                command(&["month", "2026", text]),
                Ok(Command::Month { year: 2026, month }),
                "{text}"
            );
        });
    }

    #[test]
    fn views_default_to_the_current_month_and_year() {
        [
            (
                &["month"][..],
                Command::Month {
                    year: 2026,
                    month: Month::November,
                },
            ),
            (&["year"][..], Command::Year(2026)),
            (&["summary"][..], Command::Summary(2026)),
        ]
        .into_iter()
        .for_each(|(args, expected)| assert_eq!(command(args), Ok(expected), "{args:?}"));
    }

    #[test]
    fn the_file_comes_from_the_flag_or_defaults() {
        assert_eq!(
            parse(&["list", "--file", "/tmp/x.json"]).map(|(file, _)| file),
            Ok(PathBuf::from("/tmp/x.json"))
        );
        assert_eq!(
            parse(&["--file", "otro.json", "list"]).map(|(file, _)| file),
            Ok(PathBuf::from("otro.json"))
        );
    }

    #[test]
    fn rn_01_invalid_arguments_are_rejected() {
        [
            (
                &["mark", "laborable", "2026-03-03"][..],
                ErrorKind::InvalidValue,
            ),
            (
                &["mark", "festivo", "2026-02-30"][..],
                ErrorKind::ValueValidation,
            ),
            (
                &["mark", "festivo", "2026-3-3"][..],
                ErrorKind::ValueValidation,
            ),
            (&["day", "mañana"][..], ErrorKind::ValueValidation),
            (
                &["hours", "2026", "laborable", "ocho"][..],
                ErrorKind::ValueValidation,
            ),
            (
                &["hours", "2026", "nocturna", "8"][..],
                ErrorKind::InvalidValue,
            ),
            (&["hours", "laborable", "8"][..], ErrorKind::ValueValidation),
            (&["month", "2026"][..], ErrorKind::MissingRequiredArgument),
            (&["month", "2026", "13"][..], ErrorKind::InvalidValue),
            (&["month", "2026", "june"][..], ErrorKind::InvalidValue),
            (&["unmark", "-1"][..], ErrorKind::UnknownArgument),
            (&["quota", "2026"][..], ErrorKind::MissingRequiredArgument),
            (&[][..], ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand),
            (&["--help"][..], ErrorKind::DisplayHelp),
            (&["-V"][..], ErrorKind::DisplayVersion),
        ]
        .into_iter()
        .for_each(|(args, kind)| assert_eq!(command(args), Err(kind), "{args:?}"));
    }

    #[test]
    fn the_command_definition_is_consistent() {
        <Args as clap::CommandFactory>::command().debug_assert();
    }
}
