//! Adaptador de entrada: los argumentos de la línea de comandos (§5).
//!
//! Se convierten en un [`Command`] de la aplicación.

use std::path::PathBuf;

use chrono::{Datelike, NaiveDate, Weekday};
use clap::builder::{PossibleValue, PossibleValuesParser, TypedValueParser};
use clap::{ArgAction, Parser, Subcommand};

use crate::adapters::render::{month_name, weekday_name};
use crate::application::Command;
use crate::domain::{DomainError, MarkId, MarkKind, Month, MonthExt, Shift, WEEK, parse_date};

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

/// El nombre del día en español, también sin tilde (`miercoles`, `sabado`).
fn weekday_arg() -> impl TypedValueParser<Value = Weekday> {
    choice(WEEK.map(|day| {
        let name = weekday_name(day);
        let plain: String = name
            .chars()
            .map(|c| match c {
                'á' => 'a',
                'é' => 'e',
                'í' => 'i',
                'ó' => 'o',
                'ú' => 'u',
                other => other,
            })
            .collect();
        let value = PossibleValue::new(name);
        (
            day,
            if plain == name {
                value
            } else {
                value.alias(plain)
            },
        )
    }))
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

/// Traduce los textos en inglés que clap añade a la ayuda y al uso (RNF-07).
fn spanish_clap(text: &str) -> String {
    [
        ("[possible values: ", "[valores posibles: "),
        ("[default: ", "[por defecto: "),
        ("[env: ", "[variable: "),
        ("[OPTIONS]", "[OPCIONES]"),
        ("Usage:", "Uso:"),
    ]
    .into_iter()
    .fold(text.to_owned(), |text, (english, spanish)| {
        text.replace(english, spanish)
    })
}

/// El texto en español de un error de argumentos, con el uso y la pista de `--help`, o de la
/// ayuda (RNF-07). `None` para la versión y los errores poco habituales, que muestra clap tal
/// cual. Va a stderr si `error.use_stderr()`, y si no a stdout.
pub fn error_text(error: &clap::Error) -> Option<String> {
    use clap::error::{ContextKind, ErrorKind};

    if matches!(
        error.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    ) {
        return Some(spanish_clap(&error.render().ansi().to_string()));
    }
    let context = |kind| error.get(kind).map(ToString::to_string);
    let arg = context(ContextKind::InvalidArg).unwrap_or_default();
    let value = context(ContextKind::InvalidValue).unwrap_or_default();
    let message = match error.kind() {
        ErrorKind::InvalidValue => Some(context(ContextKind::ValidValue).map_or_else(
            || format!("valor no válido «{value}» para {arg}"),
            |valid| format!("valor no válido «{value}» para {arg}; valores posibles: {valid}"),
        )),
        ErrorKind::ValueValidation => {
            let cause = std::error::Error::source(error).map(|source| {
                if source.is::<std::num::ParseIntError>() {
                    "no es un número entero".to_owned()
                } else {
                    source.to_string()
                }
            });
            Some(format!(
                "valor no válido «{value}» para {arg}: {}",
                cause.unwrap_or_default()
            ))
        }
        ErrorKind::UnknownArgument => Some(context(ContextKind::SuggestedArg).map_or_else(
            || format!("argumento inesperado «{arg}»"),
            |suggested| format!("argumento inesperado «{arg}»; ¿querías decir «{suggested}»?"),
        )),
        ErrorKind::InvalidSubcommand => {
            let name = context(ContextKind::InvalidSubcommand).unwrap_or_default();
            let suggested = error
                .get(ContextKind::SuggestedSubcommand)
                .map(|names| {
                    names
                        .to_string()
                        .split(", ")
                        .map(|name| format!("«{name}»"))
                        .collect::<Vec<_>>()
                        .join(" o ")
                })
                .filter(|names| !names.is_empty());
            Some(suggested.map_or_else(
                || format!("comando desconocido «{name}»"),
                |names| format!("comando desconocido «{name}»; ¿querías decir {names}?"),
            ))
        }
        ErrorKind::MissingRequiredArgument => {
            Some(format!("falta un argumento obligatorio: {arg}"))
        }
        ErrorKind::ArgumentConflict => Some(format!(
            "{arg} no se puede usar junto con {}",
            context(ContextKind::PriorArg).unwrap_or_default()
        )),
        _ => None,
    };
    message.map(|message| {
        let usage = context(ContextKind::Usage)
            .map(|usage| format!("\n\n{}", spanish_clap(&usage)))
            .unwrap_or_default();
        format!("error: {message}{usage}\n\nPara más información, prueba con «--help».\n")
    })
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

    fn spanish(args: &[&str]) -> Option<String> {
        Args::try_parse_from(std::iter::once("calendario").chain(args.iter().copied()))
            .err()
            .and_then(|error| error_text(&error))
    }

    #[test]
    fn rnf_07_argument_errors_are_in_spanish() {
        const MORE: &str = "\n\nPara más información, prueba con «--help».\n";
        [
            (
                &["mark", "nocturna", "2026-03-03"][..],
                "error: valor no válido «nocturna» para <TIPO>; valores posibles: laborable, reducido, festivo, vacaciones",
            ),
            (
                &["day", "mañana"][..],
                "error: valor no válido «mañana» para [FECHA]: fecha no válida: «mañana»; usa el formato AAAA-MM-DD con una fecha que exista",
            ),
            (
                &["add-year", "dos"][..],
                "error: valor no válido «dos» para <AÑO>: no es un número entero",
            ),
            (
                &["unmark", "1", "--foo"][..],
                "error: argumento inesperado «--foo»\n\nUso: calendario unmark <ID>...",
            ),
            (
                &["--fichero", "x", "list"][..],
                "error: argumento inesperado «--fichero»; ¿querías decir «--file»?\n\nUso: calendario --file <RUTA> <COMANDO>",
            ),
            (
                &["marca"][..],
                "error: comando desconocido «marca»; ¿querías decir «unmark» o «mark»?\n\nUso: calendario [OPCIONES] <COMANDO>",
            ),
            (
                &["unmark"][..],
                "error: falta un argumento obligatorio: <ID>...\n\nUso: calendario unmark <ID>...",
            ),
            (
                &["reduced-days", "viernes", "--ninguno"][..],
                "error: [DÍA]... no se puede usar junto con --ninguno\n\nUso: calendario reduced-days <DÍA>...",
            ),
        ]
        .into_iter()
        .for_each(|(args, expected)| {
            assert_eq!(
                spanish(args),
                Some(format!("{expected}{MORE}")),
                "{args:?}"
            );
        });
        assert_eq!(spanish(&["--version"]), None, "la versión la muestra clap");
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
