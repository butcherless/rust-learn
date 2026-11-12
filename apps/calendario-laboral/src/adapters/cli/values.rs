//! Los valores de los argumentos: listas cerradas construidas a partir del dominio, fechas y horas.

use chrono::{NaiveDate, Weekday};
use clap::builder::{PossibleValue, PossibleValuesParser, TypedValueParser};

use crate::adapters::render::{month_name, weekday_name};
use crate::domain::{DomainError, MarkKind, Month, MonthExt, Shift, WEEK, parse_date};

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
pub(super) fn mark_kind_arg() -> impl TypedValueParser<Value = MarkKind> {
    choice(MarkKind::ALL.map(|kind| (kind, PossibleValue::new(kind.name()))))
}

/// `laborable`, `reducido`, `laborable-verano` o `reducido-verano`, los nombres de `Shift`.
pub(super) fn shift_arg() -> impl TypedValueParser<Value = Shift> {
    choice(Shift::ALL.map(|shift| (shift, PossibleValue::new(shift.name()))))
}

/// El nombre del día en español, también sin tilde (`miercoles`, `sabado`).
pub(super) fn weekday_arg() -> impl TypedValueParser<Value = Weekday> {
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
pub(super) fn month_arg() -> impl TypedValueParser<Value = Month> {
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

pub(super) fn date_arg(text: &str) -> Result<NaiveDate, DomainError> {
    parse_date(text)
}

/// Lee horas escritas con coma o punto decimal (RN-15).
pub(super) fn hours_arg(text: &str) -> Result<f64, String> {
    text.replace(',', ".")
        .parse::<f64>()
        .map_err(|_| format!("«{text}» no es un número de horas"))
}
