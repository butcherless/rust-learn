//! La próxima ausencia desde una fecha (RN-30).

use std::iter;

use chrono::{Datelike, NaiveDate};

use crate::domain::{
    Calendar, Category, DateRange, DayKind, DomainError, ResolvedDay, Year, resolve_year,
};

/// Un tramo de días seguidos que no se trabajan, que empieza y acaba en un festivo o un día de
/// vacaciones (RN-30).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Absence {
    /// Del primer al último día de ausencia del tramo.
    pub range: DateRange,
    /// Los tipos de ausencia del tramo, en el orden en que aparecen y sin repetir.
    pub kinds: Vec<DayKind>,
    /// Los días de ausencia del tramo, sin contar los fines de semana.
    pub days: usize,
    /// Los días que faltan desde `today` hasta el primero: 0 si es hoy.
    pub days_until: i64,
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
    Ok(absence_in(&days, today))
}

fn is_absence(day: &ResolvedDay) -> bool {
    day.kind.category() == Category::Absence
}

/// La primera ausencia de `days`, que empiezan en `today` y van seguidos salvo donde falta un año.
fn absence_in(days: &[ResolvedDay], today: NaiveDate) -> Option<Absence> {
    let start = days.iter().position(is_absence)?;
    let rest = days.get(start..)?;
    let first = rest.first()?.date;
    // Los días seguidos (sin saltar un año que no esté en el calendario) que no se trabajan.
    let free: Vec<&ResolvedDay> = rest
        .iter()
        .zip(first.iter_days())
        .take_while(|(day, expected)| {
            day.date == *expected && day.kind.category() != Category::Working
        })
        .map(|(day, _)| day)
        .collect();
    // El tramo acaba en su último día de ausencia: los fines de semana del final no cuentan.
    let last = free.iter().rposition(|day| is_absence(day))?;
    let absences: Vec<&ResolvedDay> = free
        .get(..=last)?
        .iter()
        .copied()
        .filter(|day| is_absence(day))
        .collect();
    let kinds = absences
        .iter()
        .fold(Vec::new(), |kinds: Vec<DayKind>, day| {
            if kinds.contains(&day.kind) {
                kinds
            } else {
                kinds.into_iter().chain(iter::once(day.kind)).collect()
            }
        });
    Some(Absence {
        range: DateRange::new(first, free.get(last)?.date).ok()?,
        kinds,
        days: absences.len(),
        days_until: (first - today).num_days(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::MarkKind;
    use crate::domain::fixtures::{calendar, date, marked, range};

    /// Lo esperado: desde, hasta, tipos, días de ausencia y días hasta el primero.
    fn absence(
        from: NaiveDate,
        to: NaiveDate,
        kinds: &[DayKind],
        days: usize,
        days_until: i64,
    ) -> Option<Absence> {
        Some(Absence {
            range: range(from, to),
            kinds: kinds.to_vec(),
            days,
            days_until,
        })
    }

    #[test]
    fn rn_30_the_next_absence_joins_the_free_days_between_absences() {
        let december = marked(&[
            (MarkKind::Vacation, "2026-12-04", "2026-12-04"),
            (MarkKind::Holiday, "2026-12-07", "2026-12-07"),
            (MarkKind::Holiday, "2026-12-08", "2026-12-08"),
            (MarkKind::Vacation, "2026-12-14", "2026-12-18"),
        ]);
        let (vacation, holiday) = (DayKind::Vacation, DayKind::Holiday);
        [
            (
                "un puente: viernes de vacaciones, fin de semana y dos festivos",
                date(2026, 11, 14),
                absence(
                    date(2026, 12, 4),
                    date(2026, 12, 8),
                    &[vacation, holiday],
                    3,
                    20,
                ),
            ),
            (
                "hoy es festivo: empieza hoy",
                date(2026, 12, 8),
                absence(date(2026, 12, 8), date(2026, 12, 8), &[holiday], 1, 0),
            ),
            (
                "el fin de semana del final no cuenta",
                date(2026, 12, 9),
                absence(date(2026, 12, 14), date(2026, 12, 18), &[vacation], 5, 5),
            ),
            ("no quedan ausencias", date(2026, 12, 19), None),
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
            Ok(absence(
                date(2026, 7, 1),
                date(2026, 8, 4),
                &[DayKind::Vacation],
                25,
                30
            )),
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
            Ok(absence(
                date(2026, 12, 31),
                date(2027, 1, 1),
                &[DayKind::Holiday],
                2,
                1
            )),
            "fin de año"
        );
    }
}
