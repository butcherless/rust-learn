//! La próxima ausencia desde una fecha (RN-30).

use chrono::{Datelike, NaiveDate};

use crate::domain::{
    Calendar, Category, DateRange, DayKind, DomainError, ResolvedDay, Year, resolve_year,
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
/// vacaciones, partido por tipos en orden (RN-30).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Absence {
    /// Las partes del tramo, en orden; nunca está vacío.
    pub parts: Vec<AbsencePart>,
}

/// La próxima ausencia desde `today`, incluido, en los años del calendario desde el de `today`.
/// Un tramo puede seguir en el año siguiente si también está en el calendario.
pub fn next_absence(calendar: &Calendar, today: NaiveDate) -> Result<Option<Absence>, DomainError> {
    let days: Vec<ResolvedDay> = calendar
        .years()
        .iter()
        .map(Year::number)
        .filter(|&number| number >= today.year())
        .map(|number| resolve_year(calendar, number))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .filter(|day| day.date >= today)
        .collect();
    Ok(absence_in(&days))
}

fn is_absence(day: &ResolvedDay) -> bool {
    day.kind.category() == Category::Absence
}

/// La primera ausencia de `days`, que van seguidos salvo donde falta un año.
fn absence_in(days: &[ResolvedDay]) -> Option<Absence> {
    let start = days.iter().position(is_absence)?;
    let rest = days.get(start..)?;
    let first = rest.first()?.date;
    // Los días seguidos (sin saltar un año que no esté en el calendario) que no se trabajan. Solo
    // se quedan los de ausencia, así que los fines de semana del final no cuentan.
    let absences: Vec<&ResolvedDay> = rest
        .iter()
        .zip(first.iter_days())
        .take_while(|(day, expected)| {
            day.date == *expected && day.kind.category() != Category::Working
        })
        .map(|(day, _)| day)
        .filter(|day| is_absence(day))
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
            (
                "un año que no está en el calendario",
                date(2027, 1, 4),
                None,
            ),
        ]
        .into_iter()
        .for_each(|(case, today, expected)| {
            assert_eq!(next_absence(&december, today), Ok(expected), "{case}");
        });
    }

    #[test]
    fn rn_30_default_vacation_counts_and_an_absence_can_cross_into_the_next_year() {
        assert_eq!(
            next_absence(&calendar(), date(2026, 6, 1)),
            Ok(absence(&[(
                DayKind::Vacation,
                date(2026, 7, 1),
                date(2026, 8, 4)
            )])),
            "vacaciones por defecto (RN-14)"
        );
        let holiday = |calendar: Calendar, day| {
            calendar
                .with_mark(MarkKind::Holiday, DateRange::day(day), None)
                .expect("mark")
                .0
        };
        let year_end = holiday(
            holiday(
                calendar().with_year(2027).expect("year"),
                date(2026, 12, 31),
            ),
            date(2027, 1, 1),
        );
        assert_eq!(
            next_absence(&year_end, date(2026, 12, 30)),
            Ok(absence(&[(
                DayKind::Holiday,
                date(2026, 12, 31),
                date(2027, 1, 1)
            )])),
            "fin de año"
        );
    }
}
