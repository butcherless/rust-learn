//! Los errores de argumentos y la ayuda de clap, en español (RNF-07).

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

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::adapters::cli::Args;

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
}
