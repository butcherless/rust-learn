//! La próxima ausencia marcada de un año desde una fecha (RN-30).

use chrono::NaiveDate;

use crate::domain::{
    Calendar, Category, DateRange, DayKind, DomainError, Origin, ResolvedDay, resolve_year,
};

/// Una parte seguida de una ausencia con el mismo tipo; los fines de semana de en medio no la
/// cortan (RN-30).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbsencePart {
    /// El tipo: `Holiday` o `Vacation`.
    pub kind: DayKind,
    /// Del primer al último día de ese tipo.
    pub range: DateRange,
}

/// Un tramo de días seguidos que no se trabajan, que empieza y acaba en un festivo o un día de
/// vacaciones marcados, partido por tipos en orden (RN-30).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Absence {
    /// Las partes del tramo, en orden; nunca está vacío.
    pub parts: Vec<AbsencePart>,
}

/// La próxima ausencia marcada del año `number` desde `today`, incluido; en un año futuro, desde
/// su primer día. Las vacaciones por defecto (RN-14) no cuentan y cortan el tramo.
pub fn next_absence(
    calendar: &Calendar,
    number: i32,
    today: NaiveDate,
) -> Result<Option<Absence>, DomainError> {
    let days: Vec<ResolvedDay> = resolve_year(calendar, number)?
        .into_iter()
        .filter(|day| day.date >= today)
        .collect();
    Ok(absence_in(&days))
}

/// Un festivo o un día de vacaciones fijado por un `Mark`.
fn is_marked_absence(day: &ResolvedDay) -> bool {
    day.kind.category() == Category::Absence && matches!(day.origin, Origin::Mark(_))
}

/// Un día que puede quedar dentro de una ausencia: una ausencia marcada o un fin de semana.
fn is_free(day: &ResolvedDay) -> bool {
    is_marked_absence(day) || day.kind.category() == Category::NonWorking
}

/// La primera ausencia de `days`, que son días seguidos de un mismo año.
fn absence_in(days: &[ResolvedDay]) -> Option<Absence> {
    let start = days.iter().position(is_marked_absence)?;
    // Los días seguidos que no se trabajan. Solo se quedan los de ausencia, así que los fines de
    // semana del final no cuentan.
    let absences: Vec<&ResolvedDay> = days
        .get(start..)?
        .iter()
        .take_while(|day| is_free(day))
        .filter(|day| is_marked_absence(day))
        .collect();
    let parts = absences
        .chunk_by(|before, after| before.kind == after.kind)
        .map(|part| {
            let (from, to) = (part.first()?, part.last()?);
            Some(AbsencePart {
                kind: from.kind,
                range: DateRange::new(from.date, to.date).ok()?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Absence { parts })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::MarkKind;
    use crate::domain::fixtures::{calendar, date, marked, range};

    /// Lo esperado: cada parte con su tipo, desde y hasta.
    fn absence(parts: &[(DayKind, NaiveDate, NaiveDate)]) -> Option<Absence> {
        Some(Absence {
            parts: parts
                .iter()
                .map(|&(kind, from, to)| AbsencePart {
                    kind,
                    range: range(from, to),
                })
                .collect(),
        })
    }

    #[test]
    fn rn_30_the_next_absence_joins_the_free_days_and_splits_by_kind() {
        let december = marked(&[
            (MarkKind::Vacation, "2026-12-04", "2026-12-04"),
            (MarkKind::Holiday, "2026-12-07", "2026-12-07"),
            (MarkKind::Holiday, "2026-12-08", "2026-12-08"),
            (MarkKind::Vacation, "2026-12-14", "2026-12-18"),
            (MarkKind::Vacation, "2026-12-21", "2026-12-22"),
        ]);
        let (vacation, holiday) = (DayKind::Vacation, DayKind::Holiday);
        [
            (
                "un puente: viernes de vacaciones, fin de semana y dos festivos",
                date(2026, 11, 14),
                absence(&[
                    (vacation, date(2026, 12, 4), date(2026, 12, 4)),
                    (holiday, date(2026, 12, 7), date(2026, 12, 8)),
                ]),
            ),
            (
                "hoy es festivo: empieza hoy",
                date(2026, 12, 8),
                absence(&[(holiday, date(2026, 12, 8), date(2026, 12, 8))]),
            ),
            (
                "un fin de semana entre vacaciones no corta la parte, y el del final no cuenta",
                date(2026, 12, 9),
                absence(&[(vacation, date(2026, 12, 14), date(2026, 12, 22))]),
            ),
            ("no quedan ausencias", date(2026, 12, 23), None),
            ("un año pasado", date(2027, 1, 4), None),
        ]
        .into_iter()
        .for_each(|(case, today, expected)| {
            assert_eq!(next_absence(&december, 2026, today), Ok(expected), "{case}");
        });
    }

    #[test]
    fn rn_30_only_marked_absences_of_the_shown_year_count() {
        assert_eq!(
            next_absence(&calendar(), 2026, date(2026, 6, 1)),
            Ok(None),
            "las vacaciones por defecto (RN-14) no cuentan"
        );
        let summer = marked(&[(MarkKind::Holiday, "2026-07-03", "2026-07-03")]);
        assert_eq!(
            next_absence(&summer, 2026, date(2026, 6, 1)),
            Ok(absence(&[(
                DayKind::Holiday,
                date(2026, 7, 3),
                date(2026, 7, 3)
            )])),
            "un festivo entre vacaciones por defecto, que no salen en el tramo"
        );
        let holiday = |calendar: Calendar, day| {
            calendar
                .with_mark(MarkKind::Holiday, DateRange::day(day), None)
                .expect("mark")
                .0
        };
        let two_years = holiday(
            holiday(
                calendar().with_year(2027).expect("year"),
                date(2026, 12, 31),
            ),
            date(2027, 1, 1),
        );
        [
            (
                "el tramo no pasa al año siguiente",
                2026,
                absence(&[(DayKind::Holiday, date(2026, 12, 31), date(2026, 12, 31))]),
            ),
            (
                "un año futuro, desde su primer día",
                2027,
                absence(&[(DayKind::Holiday, date(2027, 1, 1), date(2027, 1, 1))]),
            ),
        ]
        .into_iter()
        .for_each(|(case, year, expected)| {
            assert_eq!(
                next_absence(&two_years, year, date(2026, 12, 30)),
                Ok(expected),
                "{case}"
            );
        });
    }
}
