//! Adaptador de entrada: los argumentos de la línea de comandos (§5).
//!
//! Se convierten en un [`Command`] de la aplicación.

use std::path::PathBuf;

use chrono::{Datelike, NaiveDate};
use clap::{ArgAction, Parser, Subcommand, ValueEnum};

use crate::application::Command;
use crate::domain::{DomainError, MarkId, MarkKind, Month, Shift, parse_date};

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
        #[arg(value_name = "TIPO", display_order = 1)]
        kind: MarkKindArg,
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
        #[arg(value_name = "MES", display_order = 2, hide_possible_values = true)]
        month: Option<MonthArg>,
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
    /// Configura las horas de una jornada.
    #[command(help_template = HELP, next_help_heading = "Argumentos")]
    Hours {
        /// Jornada.
        #[arg(value_name = "JORNADA", display_order = 1)]
        shift: ShiftArg,
        /// Horas, con coma o punto decimal: 8,5 u 8.5.
        #[arg(value_name = "HORAS", value_parser = hours_arg, display_order = 2)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MarkKindArg {
    /// Jornada reducida.
    #[value(name = "reducido")]
    Reduced,
    /// Festivo.
    #[value(name = "festivo")]
    Holiday,
    /// Vacaciones; consumen cupo.
    #[value(name = "vacaciones")]
    Vacation,
}

impl From<MarkKindArg> for MarkKind {
    fn from(arg: MarkKindArg) -> Self {
        match arg {
            MarkKindArg::Reduced => Self::Reduced,
            MarkKindArg::Holiday => Self::Holiday,
            MarkKindArg::Vacation => Self::Vacation,
        }
    }
}

/// [`Month`] como valor de la línea de comandos: el nombre en español o el número, con o sin
/// cero delante.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MonthArg {
    /// Enero.
    #[value(name = "enero", alias = "1", alias = "01")]
    January,
    /// Febrero.
    #[value(name = "febrero", alias = "2", alias = "02")]
    February,
    /// Marzo.
    #[value(name = "marzo", alias = "3", alias = "03")]
    March,
    /// Abril.
    #[value(name = "abril", alias = "4", alias = "04")]
    April,
    /// Mayo.
    #[value(name = "mayo", alias = "5", alias = "05")]
    May,
    /// Junio.
    #[value(name = "junio", alias = "6", alias = "06")]
    June,
    /// Julio.
    #[value(name = "julio", alias = "7", alias = "07")]
    July,
    /// Agosto.
    #[value(name = "agosto", alias = "8", alias = "08")]
    August,
    /// Septiembre.
    #[value(name = "septiembre", alias = "9", alias = "09", alias = "setiembre")]
    September,
    /// Octubre.
    #[value(name = "octubre", alias = "10")]
    October,
    /// Noviembre.
    #[value(name = "noviembre", alias = "11")]
    November,
    /// Diciembre.
    #[value(name = "diciembre", alias = "12")]
    December,
}

impl From<MonthArg> for Month {
    fn from(arg: MonthArg) -> Self {
        match arg {
            MonthArg::January => Self::January,
            MonthArg::February => Self::February,
            MonthArg::March => Self::March,
            MonthArg::April => Self::April,
            MonthArg::May => Self::May,
            MonthArg::June => Self::June,
            MonthArg::July => Self::July,
            MonthArg::August => Self::August,
            MonthArg::September => Self::September,
            MonthArg::October => Self::October,
            MonthArg::November => Self::November,
            MonthArg::December => Self::December,
        }
    }
}

/// [`Shift`] como valor de la línea de comandos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ShiftArg {
    /// Laborable fuera del verano.
    #[value(name = "laborable")]
    Working,
    /// Reducido fuera del verano.
    #[value(name = "reducido")]
    Reduced,
    /// Laborable del 1-jul al 31-ago.
    #[value(name = "laborable-verano")]
    SummerWorking,
    /// Reducido del 1-jul al 31-ago.
    #[value(name = "reducido-verano")]
    SummerReduced,
}

impl From<ShiftArg> for Shift {
    fn from(arg: ShiftArg) -> Self {
        match arg {
            ShiftArg::Working => Self::Working,
            ShiftArg::Reduced => Self::Reduced,
            ShiftArg::SummerWorking => Self::SummerWorking,
            ShiftArg::SummerReduced => Self::SummerReduced,
        }
    }
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
                kind: kind.into(),
                from,
                to,
                description,
            },
            Self::Unmark { id } => Command::Unmark(MarkId(id)),
            Self::List { year } => Command::List(year),
            Self::Day { date } => Command::Day(date),
            Self::Month { year, month } => Command::Month {
                year: year.unwrap_or(today.year()),
                month: month.map_or_else(|| Month::of(today), Month::from),
            },
            Self::Year { year } => Command::Year(year.unwrap_or(today.year())),
            Self::Summary { year } => Command::Summary(year.unwrap_or(today.year())),
            Self::Hours { shift, hours } => Command::Hours(shift.into(), hours),
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
                &["hours", "laborable-verano", "8.5"][..],
                Command::Hours(Shift::SummerWorking, 8.5),
            ),
            (
                &["hours", "laborable", "7,25"][..],
                Command::Hours(Shift::Working, 7.25),
            ),
            (
                &["hours", "reducido", "6"][..],
                Command::Hours(Shift::Reduced, 6.0),
            ),
            (
                &["hours", "reducido-verano", "5"][..],
                Command::Hours(Shift::SummerReduced, 5.0),
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
                &["hours", "laborable", "ocho"][..],
                ErrorKind::ValueValidation,
            ),
            (&["hours", "nocturna", "8"][..], ErrorKind::InvalidValue),
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
