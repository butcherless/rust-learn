//! Casos de uso de los `Year`: añadir, con los festivos de otro, y eliminar (RF-01, RF-19, RF-20).

use super::*;

/// Añade el año y, con `holidays_from`, copia sus festivos con la misma fecha (RF-19). Los que no
/// existen en el año nuevo, como un 29 de febrero, se saltan.
pub(super) fn add_year(
    calendar: Calendar,
    year: i32,
    holidays_from: Option<i32>,
) -> Result<Outcome, DomainError> {
    let holidays: Vec<Mark> = holidays_from
        .map(|from| calendar.year(from).map(|from| from.marks().to_vec()))
        .transpose()?
        .unwrap_or_default()
        .into_iter()
        .filter(|mark| mark.kind == MarkKind::Holiday)
        .collect();
    let calendar = calendar.with_year(year)?;
    let (calendar, copied) = holidays
        .into_iter()
        .filter_map(|mark| {
            let from = mark.range.from().with_year(year)?;
            let to = mark.range.to().with_year(year)?;
            DateRange::new(from, to)
                .ok()
                .map(|range| (range, mark.description))
        })
        .try_fold((calendar, 0), |(calendar, copied), (range, description)| {
            calendar
                .with_mark(MarkKind::Holiday, range, description)
                .map(|(calendar, _)| (calendar, copied + 1))
        })?;
    Ok(Outcome::Changed {
        calendar,
        event: Event::YearAdded {
            year,
            quota: DEFAULT_VACATION_QUOTA,
            holidays: holidays_from.map(|from| (from, copied)),
        },
    })
}

/// Elimina un año; si tiene `Mark`, solo con `confirmed` (RF-20).
pub(super) fn remove_year(
    calendar: Calendar,
    year: i32,
    confirmed: bool,
) -> Result<Outcome, DomainError> {
    let marks = calendar.year(year)?.marks().len();
    if marks > 0 && !confirmed {
        return Err(DomainError::YearHasMarks { year, marks });
    }
    let (calendar, _) = calendar.without_year(year)?;
    Ok(Outcome::Changed {
        calendar,
        event: Event::YearRemoved { year, marks },
    })
}
