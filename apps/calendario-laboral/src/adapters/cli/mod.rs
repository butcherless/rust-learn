//! Adaptador de entrada: los argumentos de la línea de comandos (§5).
//!
//! Se convierten en un [`Command`] de la aplicación.

mod errors;
mod values;

use std::path::PathBuf;

use chrono::{Datelike, NaiveDate, Weekday};
use clap::{ArgAction, Parser, Subcommand};

use crate::adapters::viewer::Output;
use crate::application::Command;
use crate::domain::{MarkId, MarkKind, Month, MonthExt, Shift};

pub use errors::error_text;
use values::{date_arg, hours_arg, mark_kind_arg, month_arg, shift_arg, weekday_arg};

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

impl Args {
    /// Dónde pueden mostrarse los informes: la ventana solo si la salida es un `terminal` y no se
    /// ha pedido `--no-tui` (RN-22).
    pub const fn output(&self, terminal: bool) -> Output {
        if terminal && !self.no_tui {
            Output::Window
        } else {
            Output::Console
        }
    }
}

/// Los subcomandos.
/// Unos días de la semana, o ninguno con `--ninguno`, que hay que pedir explícitamente para que
/// un comando sin días no los borre por error (RF-16, RF-17).
#[derive(Debug, clap::Args)]
pub struct WeekdaysArg {
    /// Días de la semana, con o sin tilde.
    #[arg(
        value_name = "DÍA",
        value_parser = weekday_arg(),
        required_unless_present = "none",
        conflicts_with = "none"
    )]
    days: Vec<Weekday>,
    /// Ningún día.
    #[arg(long = "ninguno", help_heading = "Opciones")]
    none: bool,
}

#[derive(Debug, Subcommand)]
pub enum CommandArg {
    /// Añade un año al calendario.
    #[command(help_template = HELP, next_help_heading = "Argumentos")]
    AddYear {
        /// Año, por ejemplo 2027.
        #[arg(value_name = "AÑO")]
        year: i32,
        /// Copia los festivos de este año, con la misma fecha.
        #[arg(long = "festivos-de", value_name = "AÑO", help_heading = "Opciones")]
        holidays_from: Option<i32>,
    },
    /// Elimina un año con sus marcas; si tiene marcas, hay que confirmarlo.
    #[command(help_template = HELP, next_help_heading = "Argumentos")]
    RemoveYear {
        /// Año.
        #[arg(value_name = "AÑO")]
        year: i32,
        /// Confirma que se eliminan también sus marcas.
        #[arg(long = "confirmar", help_heading = "Opciones")]
        confirmed: bool,
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
    /// Modifica una marca y conserva su id; lo que no se indica se mantiene.
    #[command(
        help_template = HELP,
        next_help_heading = "Argumentos",
        group(
            clap::ArgGroup::new("changes")
                .required(true)
                .multiple(true)
                .args(["kind", "from", "to", "description"])
        )
    )]
    Edit {
        /// Id de la marca, como lo muestra «list».
        #[arg(value_name = "ID")]
        id: u32,
        /// Tipo de marca nuevo.
        #[arg(long = "tipo", value_name = "TIPO", value_parser = mark_kind_arg(), help_heading = "Opciones")]
        kind: Option<MarkKind>,
        /// Primer día nuevo (AAAA-MM-DD).
        #[arg(long = "desde", value_name = "FECHA", value_parser = date_arg, help_heading = "Opciones")]
        from: Option<NaiveDate>,
        /// Último día nuevo (AAAA-MM-DD).
        #[arg(long = "hasta", value_name = "FECHA", value_parser = date_arg, help_heading = "Opciones")]
        to: Option<NaiveDate>,
        /// Descripción nueva; "" la quita.
        #[arg(long = "desc", value_name = "TEXTO", help_heading = "Opciones")]
        description: Option<String>,
    },
    /// Elimina una o varias marcas; si una no existe, no elimina ninguna.
    #[command(help_template = HELP, next_help_heading = "Argumentos")]
    Unmark {
        /// Ids de las marcas, como los muestra «list».
        #[arg(value_name = "ID", required = true)]
        ids: Vec<u32>,
    },
    /// Lista las marcas de todos los años o de uno.
    #[command(help_template = HELP, next_help_heading = "Argumentos")]
    List {
        /// Solo las marcas de este año.
        #[arg(value_name = "AÑO")]
        year: Option<i32>,
    },
    /// Muestra el tipo y las horas de un día.
    #[command(help_template = HELP, next_help_heading = "Argumentos")]
    Day {
        /// Fecha (AAAA-MM-DD) [por defecto: hoy].
        #[arg(value_name = "FECHA", value_parser = date_arg)]
        date: Option<NaiveDate>,
    },
    /// Muestra un mes [por defecto: el actual; con solo el mes, del año actual].
    #[command(
        help_template = HELP,
        next_help_heading = "Argumentos",
        allow_missing_positional = true
    )]
    Month {
        /// Año. Con un solo argumento, ese argumento es el mes.
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
    /// Muestra la configuración: fin de semana, jornada reducida, y cupo y horas de cada año.
    #[command(help_template = HELP)]
    Config,
    /// Configura los días del fin de semana de todos los años [por defecto: sábado y domingo].
    #[command(help_template = HELP, next_help_heading = "Argumentos")]
    Weekend {
        #[command(flatten)]
        days: WeekdaysArg,
    },
    /// Configura los días de jornada reducida de todos los años [por defecto: viernes].
    #[command(help_template = HELP, next_help_heading = "Argumentos")]
    ReducedDays {
        #[command(flatten)]
        days: WeekdaysArg,
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

impl CommandArg {
    /// El [`Command`] de la aplicación; `today` da el año y el mes que falten.
    pub fn into_command(self, today: NaiveDate) -> Command {
        match self {
            Self::AddYear {
                year,
                holidays_from,
            } => Command::AddYear {
                year,
                holidays_from,
            },
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
            Self::Edit {
                id,
                kind,
                from,
                to,
                description,
            } => Command::Edit {
                id: MarkId(id),
                kind,
                from,
                to,
                description,
            },
            Self::RemoveYear { year, confirmed } => Command::RemoveYear { year, confirmed },
            Self::Unmark { ids } => Command::Unmark(ids.into_iter().map(MarkId).collect()),
            Self::List { year } => Command::List(year),
            Self::Day { date } => Command::Day(date.unwrap_or(today)),
            Self::Month { year, month } => Command::Month {
                year: year.unwrap_or(today.year()),
                month: month.unwrap_or_else(|| Month::of(today)),
            },
            Self::Year { year } => Command::Year(year.unwrap_or(today.year())),
            Self::Summary { year } => Command::Summary(year.unwrap_or(today.year())),
            Self::Config => Command::Config,
            Self::Weekend { days } => Command::Weekend(days.days),
            Self::ReducedDays { days } => Command::ReducedDays(days.days),
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
    fn rn_22_the_window_needs_a_terminal_and_no_no_tui() {
        [
            ("terminal", &[][..], true, Output::Window),
            ("redirected", &[][..], false, Output::Console),
            ("--no-tui", &["--no-tui"][..], true, Output::Console),
            (
                "redirected with --no-tui",
                &["--no-tui"][..],
                false,
                Output::Console,
            ),
        ]
        .into_iter()
        .for_each(|(case, flags, terminal, expected)| {
            let args = Args::try_parse_from(
                ["calendario"]
                    .into_iter()
                    .chain(flags.iter().copied())
                    .chain(["year"]),
            )
            .map(|args| args.output(terminal));
            assert_eq!(args.ok(), Some(expected), "{case}");
        });
    }

    #[test]
    fn rf_01_to_rf_10_parse_the_subcommands_of_the_table() {
        [
            (
                &["add-year", "2027"][..],
                Command::AddYear {
                    year: 2027,
                    holidays_from: None,
                },
            ),
            (
                &["add-year", "2027", "--festivos-de", "2026"][..],
                Command::AddYear {
                    year: 2027,
                    holidays_from: Some(2026),
                },
            ),
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
            (&["unmark", "4"][..], Command::Unmark(vec![MarkId(4)])),
            (
                &["remove-year", "2025"][..],
                Command::RemoveYear {
                    year: 2025,
                    confirmed: false,
                },
            ),
            (
                &["remove-year", "2025", "--confirmar"][..],
                Command::RemoveYear {
                    year: 2025,
                    confirmed: true,
                },
            ),
            (
                &["mark", "laborable", "2026-03-06"][..],
                Command::Mark {
                    kind: MarkKind::Working,
                    from: date(2026, 3, 6),
                    to: None,
                    description: None,
                },
            ),
            (
                &[
                    "edit",
                    "4",
                    "--tipo",
                    "festivo",
                    "--desde",
                    "2026-03-02",
                    "--hasta",
                    "2026-03-03",
                    "--desc",
                    "Puente",
                ][..],
                Command::Edit {
                    id: MarkId(4),
                    kind: Some(MarkKind::Holiday),
                    from: Some(date(2026, 3, 2)),
                    to: Some(date(2026, 3, 3)),
                    description: Some("Puente".to_owned()),
                },
            ),
            (
                &["edit", "4", "--hasta", "2026-03-03"][..],
                Command::Edit {
                    id: MarkId(4),
                    kind: None,
                    from: None,
                    to: Some(date(2026, 3, 3)),
                    description: None,
                },
            ),
            (
                &["unmark", "4", "7"][..],
                Command::Unmark(vec![MarkId(4), MarkId(7)]),
            ),
            (&["list"][..], Command::List(None)),
            (&["config"][..], Command::Config),
            (
                &["reduced-days", "viernes", "lunes"][..],
                Command::ReducedDays(vec![Weekday::Fri, Weekday::Mon]),
            ),
            (
                &["reduced-days", "miercoles", "miércoles", "sabado"][..],
                Command::ReducedDays(vec![Weekday::Wed, Weekday::Wed, Weekday::Sat]),
            ),
            (
                &["reduced-days", "--ninguno"][..],
                Command::ReducedDays(Vec::new()),
            ),
            (
                &["weekend", "viernes", "sábado"][..],
                Command::Weekend(vec![Weekday::Fri, Weekday::Sat]),
            ),
            (&["weekend", "--ninguno"][..], Command::Weekend(Vec::new())),
            (&["list", "2026"][..], Command::List(Some(2026))),
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
    fn rf_05_rf_06_views_default_to_today() {
        [
            (
                &["month"][..],
                Command::Month {
                    year: 2026,
                    month: Month::November,
                },
            ),
            (
                &["month", "junio"][..],
                Command::Month {
                    year: 2026,
                    month: Month::June,
                },
            ),
            (
                &["month", "12"][..],
                Command::Month {
                    year: 2026,
                    month: Month::December,
                },
            ),
            (&["year"][..], Command::Year(2026)),
            (&["summary"][..], Command::Summary(2026)),
            (&["day"][..], Command::Day(date(2026, 11, 8))),
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
                &["mark", "nocturna", "2026-03-03"][..],
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
            (&["month", "2026"][..], ErrorKind::InvalidValue),
            (&["month", "2026", "13"][..], ErrorKind::InvalidValue),
            (&["reduced-days"][..], ErrorKind::MissingRequiredArgument),
            (&["unmark"][..], ErrorKind::MissingRequiredArgument),
            (&["edit", "4"][..], ErrorKind::MissingRequiredArgument),
            (
                &["edit", "4", "--desde", "2026-3-2"][..],
                ErrorKind::ValueValidation,
            ),
            (&["list", "--year", "2026"][..], ErrorKind::UnknownArgument),
            (&["weekend"][..], ErrorKind::MissingRequiredArgument),
            (&["reduced-days", "friday"][..], ErrorKind::InvalidValue),
            (
                &["reduced-days", "viernes", "--ninguno"][..],
                ErrorKind::ArgumentConflict,
            ),
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
