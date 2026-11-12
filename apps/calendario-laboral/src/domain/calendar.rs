//! El contenido del fichero: configuración, `Year` y `Mark` (§2.3).
//!
//! Cada cambio recibe el `Calendar` por valor y devuelve uno nuevo.

use std::fmt;
use std::iter;

use chrono::{Datelike, NaiveDate, Weekday};

use crate::domain::{DateRange, DomainError, MarkKind, Shift, check_year};

/// Días de vacaciones por año salvo que se configure otra cosa (RN-10).
pub const DEFAULT_VACATION_QUOTA: u32 = 25;

/// Los días de la semana, desde el lunes (RN-12).
pub const WEEK: [Weekday; 7] = [
    Weekday::Mon,
    Weekday::Tue,
    Weekday::Wed,
    Weekday::Thu,
    Weekday::Fri,
    Weekday::Sat,
    Weekday::Sun,
];

/// Las horas de los cuatro `Shift` laborables (RF-09).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hours {
    working: f64,
    reduced: f64,
    summer_working: f64,
    summer_reduced: f64,
}

impl Default for Hours {
    fn default() -> Self {
        Self {
            working: 8.5,
            reduced: 6.0,
            summer_working: 8.5,
            summer_reduced: 6.0,
        }
    }
}

impl Hours {
    /// Las horas de `shift`.
    pub const fn of(self, shift: Shift) -> f64 {
        match shift {
            Shift::Working => self.working,
            Shift::Reduced => self.reduced,
            Shift::SummerWorking => self.summer_working,
            Shift::SummerReduced => self.summer_reduced,
        }
    }

    /// Las mismas horas con `hours` para `shift`, que deben estar entre 0 y 24 (RN-17).
    pub fn with(self, shift: Shift, hours: f64) -> Result<Self, DomainError> {
        (0.0..=24.0)
            .contains(&hours)
            .then_some(match shift {
                Shift::Working => Self {
                    working: hours,
                    ..self
                },
                Shift::Reduced => Self {
                    reduced: hours,
                    ..self
                },
                Shift::SummerWorking => Self {
                    summer_working: hours,
                    ..self
                },
                Shift::SummerReduced => Self {
                    summer_reduced: hours,
                    ..self
                },
            })
            .ok_or(DomainError::InvalidHours(hours))
    }
}

/// El id de un `Mark`, único en el fichero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MarkId(pub u32);

impl fmt::Display for MarkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(&self.0.to_string())
    }
}

/// Un tipo fijado sobre un día o un rango de días.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mark {
    /// Id único en el fichero.
    pub id: MarkId,
    /// Lo que fija la marca.
    pub kind: MarkKind,
    /// Los días que cubre.
    pub range: DateRange,
    /// Texto opcional que se muestra con los días.
    pub description: Option<String>,
}

/// Un año del calendario con su cupo de vacaciones, sus `Hours` y sus `Mark`.
#[derive(Debug, Clone, PartialEq)]
pub struct Year {
    number: i32,
    vacation_quota: u32,
    hours: Hours,
    marks: Vec<Mark>,
}

impl Year {
    const fn new(number: i32, hours: Hours) -> Self {
        Self {
            number,
            vacation_quota: DEFAULT_VACATION_QUOTA,
            hours,
            marks: Vec::new(),
        }
    }

    /// Las horas de cada `Shift` en este año (RN-25).
    pub const fn hours(&self) -> Hours {
        self.hours
    }

    /// El número del año.
    pub const fn number(&self) -> i32 {
        self.number
    }

    /// Días de vacaciones disponibles en el año.
    pub const fn vacation_quota(&self) -> u32 {
        self.vacation_quota
    }

    /// Los `Mark`, en el orden en que se añadieron.
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    /// Si algún `Mark` es de vacaciones, lo que desactiva las vacaciones por defecto (RN-14).
    pub fn has_vacation_marks(&self) -> bool {
        self.marks
            .iter()
            .any(|mark| mark.kind == MarkKind::Vacation)
    }

    fn with_mark(self, mark: Mark) -> Self {
        Self {
            marks: self.marks.into_iter().chain(iter::once(mark)).collect(),
            ..self
        }
    }

    fn without_mark(self, id: MarkId) -> Self {
        Self {
            marks: self
                .marks
                .into_iter()
                .filter(|mark| mark.id != id)
                .collect(),
            ..self
        }
    }
}

/// Los días de `days` en el orden de la semana, sin repetidos.
fn from_monday(days: &[Weekday]) -> Vec<Weekday> {
    WEEK.into_iter().filter(|day| days.contains(day)).collect()
}

/// El resultado de añadir un `Mark`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Marking {
    /// Se ha añadido el `Mark`.
    Added(Mark),
    /// Ya existía un `Mark` idéntico, así que no se ha añadido nada (RN-06).
    Existing(MarkId),
}

/// La configuración común y los años (§2.3).
#[derive(Debug, Clone, PartialEq)]
pub struct Calendar {
    weekend: Vec<Weekday>,
    reduced_days: Vec<Weekday>,
    years: Vec<Year>,
}

impl Default for Calendar {
    /// Sábado y domingo como fin de semana, el viernes reducido, las horas por defecto y ningún
    /// año.
    fn default() -> Self {
        Self {
            weekend: vec![Weekday::Sat, Weekday::Sun],
            reduced_days: vec![Weekday::Fri],
            years: Vec::new(),
        }
    }
}

impl Calendar {
    /// Los días de fin de semana, desde el lunes.
    pub fn weekend(&self) -> &[Weekday] {
        &self.weekend
    }

    /// Los días de la regla `ReducedWeekday`, desde el lunes (RN-18).
    pub fn reduced_days(&self) -> &[Weekday] {
        &self.reduced_days
    }

    /// Los años, en orden ascendente.
    pub fn years(&self) -> &[Year] {
        &self.years
    }

    /// Todos los `Mark` de todos los años.
    pub fn marks(&self) -> impl Iterator<Item = &Mark> {
        self.years.iter().flat_map(Year::marks)
    }

    /// El año `number`, que debe estar en el calendario (RN-02).
    pub fn year(&self, number: i32) -> Result<&Year, DomainError> {
        self.years
            .iter()
            .find(|year| year.number == number)
            .ok_or(DomainError::UnknownYear(number))
    }

    /// Si `date` cae en fin de semana (RN-04).
    pub fn is_weekend(&self, date: NaiveDate) -> bool {
        self.weekend.contains(&date.weekday())
    }

    /// Si `date` es un día de la regla `ReducedWeekday` (RN-18).
    pub fn is_reduced_day(&self, date: NaiveDate) -> bool {
        self.reduced_days.contains(&date.weekday())
    }

    /// El mismo calendario con `days` como fin de semana, ordenados desde el lunes y sin repetidos.
    #[must_use]
    pub fn with_weekend(self, days: &[Weekday]) -> Self {
        Self {
            weekend: from_monday(days),
            ..self
        }
    }

    /// El mismo calendario con `days` como días reducidos por regla, ordenados desde el lunes y
    /// sin repetidos (RN-18).
    #[must_use]
    pub fn with_reduced_days(self, days: &[Weekday]) -> Self {
        Self {
            reduced_days: from_monday(days),
            ..self
        }
    }

    /// El mismo calendario sin el año `number`, y ese año con sus `Mark` (RF-20).
    pub fn without_year(self, number: i32) -> Result<(Self, Year), DomainError> {
        let year = self.year(number)?.clone();
        Ok((
            Self {
                years: self
                    .years
                    .into_iter()
                    .filter(|year| year.number != number)
                    .collect(),
                ..self
            },
            year,
        ))
    }

    /// El mismo calendario con `hours` para todos los `Shift` del año `number`, sin tocar las
    /// de los demás años (RN-25).
    pub fn with_year_hours(self, number: i32, hours: Hours) -> Result<Self, DomainError> {
        self.update_year(number, |year| Year { hours, ..year })
    }

    /// El mismo calendario con `hours` para `shift` en el año `number` (RF-09, RN-25).
    pub fn with_shift_hours(
        self,
        number: i32,
        shift: Shift,
        hours: f64,
    ) -> Result<Self, DomainError> {
        let hours = self.year(number)?.hours.with(shift, hours)?;
        self.with_year_hours(number, hours)
    }

    /// Las `Hours` que hereda un `Year` nuevo: las del anterior más cercano; si no hay, las del
    /// siguiente; y sin años, las de por defecto (RN-25).
    fn inherited_hours(&self, number: i32) -> Hours {
        let previous = self.years.iter().rev().find(|year| year.number < number);
        let next = self.years.iter().find(|year| year.number > number);
        previous.or(next).map_or_else(Hours::default, Year::hours)
    }

    /// El mismo calendario con el año `number`, el cupo por defecto y sin `Mark` (RF-01).
    pub fn with_year(self, number: i32) -> Result<Self, DomainError> {
        let number = check_year(number)?;
        if self.year(number).is_ok() {
            return Err(DomainError::DuplicateYear(number));
        }
        let year = Year::new(number, self.inherited_hours(number));
        let (before, after): (Vec<Year>, Vec<Year>) = self
            .years
            .into_iter()
            .partition(|year| year.number < number);
        Ok(Self {
            years: before
                .into_iter()
                .chain(iter::once(year))
                .chain(after)
                .collect(),
            ..self
        })
    }

    /// El mismo calendario con `days` como cupo de vacaciones del año `number` (RF-10).
    pub fn with_quota(self, number: i32, days: u32) -> Result<Self, DomainError> {
        self.update_year(number, |year| Year {
            vacation_quota: days,
            ..year
        })
    }

    /// Añade un `Mark` con el siguiente id, salvo que exista uno idéntico (RF-02, RN-02, RN-06).
    ///
    /// Dos `Mark` son idénticos si tienen el mismo tipo y rango; la descripción no cuenta.
    pub fn with_mark(
        self,
        kind: MarkKind,
        range: DateRange,
        description: Option<String>,
    ) -> Result<(Self, Marking), DomainError> {
        let number = range.year()?;
        let existing = self
            .year(number)?
            .marks
            .iter()
            .find(|mark| mark.kind == kind && mark.range == range)
            .map(|mark| mark.id);
        match existing {
            Some(id) => Ok((self, Marking::Existing(id))),
            None => {
                let mark = Mark {
                    id: self.next_id(),
                    kind,
                    range,
                    description,
                };
                let calendar = self.update_year(number, |year| year.with_mark(mark.clone()))?;
                Ok((calendar, Marking::Added(mark)))
            }
        }
    }

    /// El mismo calendario con `mark` en lugar del `Mark` de su id, y el `Mark` anterior (RF-18).
    /// Falla si el id no existe, si el año del rango no existe o si queda idéntico a otro `Mark`.
    pub fn with_replaced_mark(self, mark: Mark) -> Result<(Self, Mark), DomainError> {
        let (calendar, old) = self.without_mark(mark.id)?;
        let identical = calendar
            .year(mark.range.year()?)?
            .marks
            .iter()
            .find(|other| other.kind == mark.kind && other.range == mark.range)
            .map(|other| other.id);
        match identical {
            Some(id) => Err(DomainError::DuplicateMark(id)),
            None => calendar.restore_mark(mark).map(|calendar| (calendar, old)),
        }
    }

    /// Añade un `Mark` que conserva su id, tal como se lee del fichero. El id no puede estar en uso.
    pub fn restore_mark(self, mark: Mark) -> Result<Self, DomainError> {
        if self.marks().any(|existing| existing.id == mark.id) {
            return Err(DomainError::DuplicateMarkId(mark.id));
        }
        let number = mark.range.year()?;
        self.update_year(number, |year| year.with_mark(mark.clone()))
    }

    /// Elimina el `Mark` `id` y lo devuelve (RF-03).
    pub fn without_mark(self, id: MarkId) -> Result<(Self, Mark), DomainError> {
        let mark = self
            .marks()
            .find(|mark| mark.id == id)
            .cloned()
            .ok_or(DomainError::UnknownMark(id))?;
        let years = self
            .years
            .into_iter()
            .map(|year| year.without_mark(id))
            .collect();
        Ok((Self { years, ..self }, mark))
    }

    /// El mayor id en uso más uno, o 1 si no hay `Mark` (RN-16).
    fn next_id(&self) -> MarkId {
        MarkId(
            self.marks()
                .map(|mark| mark.id.0)
                .max()
                .map_or(1, |max| max.saturating_add(1)),
        )
    }

    fn update_year(self, number: i32, update: impl Fn(Year) -> Year) -> Result<Self, DomainError> {
        self.year(number)?;
        Ok(Self {
            years: self
                .years
                .into_iter()
                .map(|year| {
                    if year.number == number {
                        update(year)
                    } else {
                        year
                    }
                })
                .collect(),
            ..self
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::{calendar, date, marked, range};

    fn add(
        calendar: Calendar,
        kind: MarkKind,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<(Calendar, Marking), DomainError> {
        DateRange::new(from, to).and_then(|range| calendar.with_mark(kind, range, None))
    }

    #[test]
    fn rf_01_adds_years_in_order_with_the_default_quota() {
        let years = Calendar::default()
            .with_year(2027)
            .and_then(|calendar| calendar.with_year(2025))
            .and_then(|calendar| calendar.with_year(2026))
            .map(|calendar| {
                calendar
                    .years()
                    .iter()
                    .map(|year| (year.number(), year.vacation_quota(), year.marks().len()))
                    .collect::<Vec<_>>()
            });
        assert_eq!(years, Ok(vec![(2025, 25, 0), (2026, 25, 0), (2027, 25, 0)]));
    }

    #[test]
    fn rf_01_rejects_repeated_and_invalid_years() {
        [
            (2026, DomainError::DuplicateYear(2026)),
            (0, DomainError::InvalidYear(0)),
            (10_000, DomainError::InvalidYear(10_000)),
        ]
        .into_iter()
        .for_each(|(year, error)| {
            assert_eq!(calendar().with_year(year), Err(error), "{year}");
        });
    }

    #[test]
    fn rf_02_adds_marks_with_increasing_ids() {
        let result = add(
            calendar(),
            MarkKind::Holiday,
            date(2026, 3, 3),
            date(2026, 3, 3),
        )
        .and_then(|(calendar, _)| {
            DateRange::new(date(2026, 6, 5), date(2026, 6, 12)).and_then(|range| {
                calendar.with_mark(MarkKind::Vacation, range, Some("Viaje".to_owned()))
            })
        });
        let expected = Mark {
            id: MarkId(2),
            kind: MarkKind::Vacation,
            range: range(date(2026, 6, 5), date(2026, 6, 12)),
            description: Some("Viaje".to_owned()),
        };
        assert_eq!(
            result.as_ref().map(|(_, marking)| marking),
            Ok(&Marking::Added(expected.clone()))
        );
        assert_eq!(
            result.map(|(calendar, _)| calendar.marks().cloned().collect::<Vec<_>>().len()),
            Ok(2)
        );
    }

    #[test]
    fn rn_02_marks_need_an_existing_year_and_a_single_year() {
        [
            (
                date(2025, 3, 3),
                date(2025, 3, 3),
                DomainError::UnknownYear(2025),
            ),
            (
                date(2026, 12, 28),
                date(2027, 1, 3),
                DomainError::CrossesYear {
                    from: date(2026, 12, 28),
                    to: date(2027, 1, 3),
                },
            ),
        ]
        .into_iter()
        .for_each(|(from, to, error)| {
            assert_eq!(
                add(calendar(), MarkKind::Holiday, from, to),
                Err(error),
                "{from} {to}"
            );
        });
    }

    #[test]
    fn rn_05_overlapping_marks_are_all_kept() {
        let marks = add(
            calendar(),
            MarkKind::Vacation,
            date(2026, 6, 1),
            date(2026, 6, 30),
        )
        .and_then(|(calendar, _)| {
            add(
                calendar,
                MarkKind::Holiday,
                date(2026, 6, 24),
                date(2026, 6, 24),
            )
        })
        .map(|(calendar, _)| calendar.marks().count());
        assert_eq!(marks, Ok(2));
    }

    #[test]
    fn rn_06_identical_marks_are_not_duplicated() {
        let first = add(
            calendar(),
            MarkKind::Holiday,
            date(2026, 3, 3),
            date(2026, 3, 3),
        );
        let second = first.clone().and_then(|(calendar, _)| {
            calendar.with_mark(
                MarkKind::Holiday,
                DateRange::day(date(2026, 3, 3)),
                Some("Otra descripción".to_owned()),
            )
        });
        assert_eq!(
            second.as_ref().map(|(_, marking)| marking),
            Ok(&Marking::Existing(MarkId(1)))
        );
        assert_eq!(
            second.map(|(calendar, _)| calendar),
            first.map(|(calendar, _)| calendar)
        );
        let other_kind = add(
            calendar(),
            MarkKind::Holiday,
            date(2026, 3, 3),
            date(2026, 3, 3),
        )
        .and_then(|(calendar, _)| {
            add(
                calendar,
                MarkKind::Vacation,
                date(2026, 3, 3),
                date(2026, 3, 3),
            )
        })
        .map(|(_, marking)| marking);
        assert!(
            matches!(other_kind, Ok(Marking::Added(_))),
            "{other_kind:?}"
        );
    }

    #[test]
    fn rf_18_replaces_a_mark_keeping_its_id() {
        let calendar = marked(&[
            (MarkKind::Holiday, "2026-03-03", "2026-03-03"),
            (MarkKind::Vacation, "2026-06-01", "2026-06-05"),
        ]);
        let edited = Mark {
            id: MarkId(2),
            kind: MarkKind::Vacation,
            range: range(date(2026, 6, 1), date(2026, 6, 12)),
            description: Some("Viaje".to_owned()),
        };
        let replaced = calendar.clone().with_replaced_mark(edited.clone());
        assert_eq!(
            replaced
                .as_ref()
                .map(|(calendar, old)| (calendar.marks().cloned().collect::<Vec<_>>(), old.id)),
            Ok((
                vec![
                    calendar.marks().next().cloned().expect("holiday"),
                    edited.clone()
                ],
                MarkId(2)
            ))
        );
        [
            (
                Mark {
                    kind: MarkKind::Holiday,
                    range: range(date(2026, 3, 3), date(2026, 3, 3)),
                    ..edited.clone()
                },
                DomainError::DuplicateMark(MarkId(1)),
            ),
            (
                Mark {
                    id: MarkId(9),
                    ..edited.clone()
                },
                DomainError::UnknownMark(MarkId(9)),
            ),
            (
                Mark {
                    range: range(date(2027, 1, 4), date(2027, 1, 8)),
                    ..edited
                },
                DomainError::UnknownYear(2027),
            ),
        ]
        .into_iter()
        .for_each(|(mark, error)| {
            assert_eq!(
                calendar.clone().with_replaced_mark(mark.clone()),
                Err(error),
                "{mark:?}"
            );
        });
    }

    #[test]
    fn rf_20_removes_a_year_with_its_marks() {
        let calendar = marked(&[(MarkKind::Holiday, "2026-03-03", "2026-03-03")])
            .with_year(2027)
            .expect("2027");
        let removed = calendar.clone().without_year(2026);
        assert_eq!(
            removed.as_ref().map(|(calendar, year)| (
                calendar
                    .years()
                    .iter()
                    .map(Year::number)
                    .collect::<Vec<_>>(),
                year.number(),
                year.marks().len()
            )),
            Ok((vec![2027], 2026, 1))
        );
        assert_eq!(
            calendar.without_year(2030),
            Err(DomainError::UnknownYear(2030))
        );
    }

    #[test]
    fn rf_03_removes_marks() {
        let removed = add(
            calendar(),
            MarkKind::Holiday,
            date(2026, 3, 3),
            date(2026, 3, 3),
        )
        .and_then(|(calendar, _)| calendar.without_mark(MarkId(1)))
        .map(|(calendar, mark)| (calendar.marks().count(), mark.id));
        assert_eq!(removed, Ok((0, MarkId(1))));
        assert_eq!(
            calendar().without_mark(MarkId(7)),
            Err(DomainError::UnknownMark(MarkId(7)))
        );
    }

    #[test]
    fn rn_16_the_next_id_is_one_more_than_the_largest() {
        let ids = add(
            calendar(),
            MarkKind::Holiday,
            date(2026, 3, 3),
            date(2026, 3, 3),
        )
        .and_then(|(calendar, _)| {
            add(
                calendar,
                MarkKind::Holiday,
                date(2026, 3, 4),
                date(2026, 3, 4),
            )
        })
        .and_then(|(calendar, _)| calendar.without_mark(MarkId(1)))
        .and_then(|(calendar, _)| {
            add(
                calendar,
                MarkKind::Holiday,
                date(2026, 3, 5),
                date(2026, 3, 5),
            )
        })
        .map(|(calendar, _)| calendar.marks().map(|mark| mark.id.0).collect::<Vec<_>>());
        assert_eq!(ids, Ok(vec![2, 3]));
    }

    #[test]
    fn restored_marks_keep_their_id_which_must_be_unique() {
        let mark = Mark {
            id: MarkId(9),
            kind: MarkKind::Holiday,
            range: DateRange::day(date(2026, 3, 3)),
            description: None,
        };
        let restored = calendar().restore_mark(mark.clone());
        assert_eq!(
            restored
                .clone()
                .map(|calendar| calendar.marks().cloned().collect::<Vec<_>>()),
            Ok(vec![mark.clone()])
        );
        assert_eq!(
            restored.and_then(|calendar| calendar.restore_mark(mark)),
            Err(DomainError::DuplicateMarkId(MarkId(9)))
        );
    }

    #[test]
    fn rf_09_sets_the_hours_of_each_shift() {
        Shift::ALL.into_iter().for_each(|shift| {
            let hours = calendar()
                .with_shift_hours(2026, shift, 7.25)
                .and_then(|calendar| calendar.year(2026).map(Year::hours));
            assert_eq!(
                hours.clone().map(|hours| hours.of(shift)),
                Ok(7.25),
                "{shift}"
            );
            let others = hours.map(|hours| {
                Shift::ALL
                    .into_iter()
                    .filter(|other| *other != shift)
                    .map(|other| hours.of(other))
                    .collect::<Vec<_>>()
            });
            assert_eq!(
                others.map(|others| others.iter().all(|other| *other != 7.25)),
                Ok(true),
                "{shift}"
            );
        });
    }

    #[test]
    fn rn_25_each_year_has_its_hours_and_a_new_year_inherits_them() {
        let hours = |calendar: &Calendar, number| {
            calendar
                .year(number)
                .map(|year| year.hours().of(Shift::Working))
        };
        let calendar = calendar()
            .with_shift_hours(2026, Shift::Working, 8.0)
            .and_then(|calendar| calendar.with_year(2028))
            .and_then(|calendar| calendar.with_year(2025))
            .and_then(|calendar| calendar.with_shift_hours(2028, Shift::Working, 7.0))
            .and_then(|calendar| calendar.with_year(2027))
            .expect("calendar");
        [
            (2025, 8.0, "sin año anterior, hereda del siguiente"),
            (2026, 8.0, "las suyas"),
            (2027, 8.0, "hereda del anterior más cercano, 2026"),
            (2028, 7.0, "las suyas, sin cambiar las de 2026"),
        ]
        .into_iter()
        .for_each(|(number, expected, case)| {
            assert_eq!(hours(&calendar, number), Ok(expected), "{number}: {case}");
        });
        assert_eq!(
            Calendar::default()
                .with_year(2030)
                .and_then(|calendar| calendar.year(2030).map(Year::hours)),
            Ok(Hours::default()),
            "sin años, las de por defecto"
        );
        assert_eq!(
            calendar.with_shift_hours(2031, Shift::Working, 8.0),
            Err(DomainError::UnknownYear(2031))
        );
    }

    #[test]
    fn rn_17_hours_are_between_0_and_24() {
        [
            (0.0, true),
            (24.0, true),
            (-0.5, false),
            (24.5, false),
            (f64::NAN, false),
        ]
        .into_iter()
        .for_each(|(hours, valid)| {
            assert_eq!(
                Hours::default().with(Shift::Working, hours).is_ok(),
                valid,
                "{hours}"
            );
        });
    }

    #[test]
    fn rf_10_sets_the_quota_of_a_year() {
        let quota = calendar()
            .with_quota(2026, 30)
            .and_then(|calendar| calendar.year(2026).map(Year::vacation_quota));
        assert_eq!(quota, Ok(30));
        assert_eq!(
            calendar().with_quota(2030, 30),
            Err(DomainError::UnknownYear(2030))
        );
    }

    #[test]
    fn rn_04_the_weekend_is_computed_from_the_weekday() {
        let calendar = Calendar::default();
        assert_eq!(calendar.weekend(), [Weekday::Sat, Weekday::Sun]);
        [
            (date(2026, 6, 5), false),
            (date(2026, 6, 6), true),
            (date(2026, 6, 7), true),
        ]
        .into_iter()
        .for_each(|(day, weekend)| assert_eq!(calendar.is_weekend(day), weekend, "{day}"));
        let friday = calendar.with_weekend(&[Weekday::Sun, Weekday::Fri, Weekday::Sun]);
        assert_eq!(friday.weekend(), [Weekday::Fri, Weekday::Sun]);
    }

    #[test]
    fn rn_18_fridays_are_the_default_reduced_days() {
        let calendar = Calendar::default();
        assert_eq!(calendar.reduced_days(), [Weekday::Fri]);
        [
            (date(2026, 6, 4), false),
            (date(2026, 6, 5), true),
            (date(2026, 6, 6), false),
        ]
        .into_iter()
        .for_each(|(day, reduced)| assert_eq!(calendar.is_reduced_day(day), reduced, "{day}"));
        let custom = calendar.with_reduced_days(&[Weekday::Fri, Weekday::Mon, Weekday::Fri]);
        assert_eq!(custom.reduced_days(), [Weekday::Mon, Weekday::Fri]);
        assert!(custom.with_reduced_days(&[]).reduced_days().is_empty());
    }

    #[test]
    fn mark_ids_display_as_numbers() {
        assert_eq!(MarkId(4).to_string(), "4");
    }
}
