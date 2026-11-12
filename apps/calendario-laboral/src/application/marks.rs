//! Casos de uso de los `Mark`: marcar, modificar, eliminar y listar (RF-02, RF-03, RF-04, RF-18).

use super::*;

/// Marca un día o un rango; un `Mark` idéntico no se añade (RF-02, RN-06, RN-11).
pub(super) fn mark(
    calendar: Calendar,
    kind: MarkKind,
    range: DateRange,
    description: Option<String>,
) -> Result<Outcome, DomainError> {
    match calendar.with_mark(kind, range, trimmed(description))? {
        (_, Marking::Existing(id)) => Ok(Outcome::Unchanged(Event::AlreadyMarked(id))),
        (calendar, Marking::Added(mark)) => {
            let excess = vacation_excess(&calendar, &mark)?;
            Ok(Outcome::Changed {
                calendar,
                event: Event::Marked { mark, excess },
            })
        }
    }
}

/// Cambia lo indicado de un `Mark` y mantiene el resto, también su id (RF-18). Con
/// `description`, una en blanco quita la que tuviera.
pub(super) fn edit(
    calendar: Calendar,
    id: MarkId,
    kind: Option<MarkKind>,
    (from, to): (Option<NaiveDate>, Option<NaiveDate>),
    description: Option<String>,
) -> Result<Outcome, DomainError> {
    let old = calendar.mark(id)?.clone();
    let mark = Mark {
        id,
        kind: kind.unwrap_or(old.kind),
        range: DateRange::new(
            from.unwrap_or(old.range.from()),
            to.unwrap_or(old.range.to()),
        )?,
        description: description.map_or(old.description, |text| trimmed(Some(text))),
    };
    let (calendar, _) = calendar.with_replaced_mark(mark.clone())?;
    let excess = vacation_excess(&calendar, &mark)?;
    Ok(Outcome::Changed {
        calendar,
        event: Event::Edited { mark, excess },
    })
}

/// Elimina los `Mark` de `ids` uno tras otro; el primero que no existe hace fallar todo y el
/// fichero no cambia (RN-13). Un id repetido se elimina una vez.
pub(super) fn unmark(calendar: Calendar, ids: &[MarkId]) -> Result<Outcome, DomainError> {
    let unique = ids.iter().fold(Vec::new(), |unique: Vec<MarkId>, id| {
        if unique.contains(id) {
            unique
        } else {
            unique.into_iter().chain([*id]).collect()
        }
    });
    let (calendar, removed) = unique.into_iter().try_fold(
        (calendar, Vec::new()),
        |(calendar, removed): (Calendar, Vec<Mark>), id| {
            calendar
                .without_mark(id)
                .map(|(calendar, mark)| (calendar, removed.into_iter().chain([mark]).collect()))
        },
    )?;
    Ok(Outcome::Changed {
        calendar,
        event: Event::Unmarked(removed),
    })
}

pub(super) fn list(calendar: &Calendar, year: Option<i32>) -> Result<Vec<Mark>, DomainError> {
    match year {
        Some(year) => calendar
            .year(year)
            .map(|year| sorted(year.marks().iter().cloned())),
        None => Ok(sorted(calendar.marks().cloned())),
    }
}

/// La descripción sin espacios alrededor; en blanco, ninguna.
fn trimmed(description: Option<String>) -> Option<String> {
    description
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

/// Los días de vacaciones por encima del cupo tras guardar `mark`, para el aviso de RN-11.
fn vacation_excess(calendar: &Calendar, mark: &Mark) -> Result<usize, DomainError> {
    match mark.kind {
        MarkKind::Vacation => Ok(summarize(calendar, mark.range.from().year())?.vacation_excess()),
        MarkKind::Working | MarkKind::Reduced | MarkKind::Holiday => Ok(0),
    }
}
