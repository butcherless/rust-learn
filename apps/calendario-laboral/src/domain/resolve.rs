//! Resolución del `DayKind` efectivo de cada día (§2.2) y resumen anual (RF-08).

use std::collections::HashSet;

use chrono::{Datelike, NaiveDate};

use crate::domain::{
    Calendar, Category, DayKind, DomainError, MarkId, MarkKind, Month, MonthExt, Year, days_of_year,
};

/// De dónde sale el `DayKind` de un día.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Lo ha fijado un `Mark`.
    Mark(MarkId),
    /// Lo ha fijado la regla de fin de semana.
    Weekend,
    /// Lo ha fijado la regla `ReducedWeekday`, los viernes por defecto (RN-18).
    ReducedWeekday,
    /// No aplica nada, así que es un día laborable.
    Default,
    /// Vacaciones por defecto desde el 1 de julio en un año sin marcas de vacaciones (RN-14).
    DefaultVacation,
}

/// Una fecha con su `DayKind` efectivo, horas, `Origin` y descripción.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedDay {
    /// La fecha.
    pub date: NaiveDate,
    /// El `DayKind` efectivo.
    pub kind: DayKind,
    /// Las horas de su `Shift`, o 0 si no es laborable.
    pub hours: f64,
    /// Lo que ha fijado el tipo.
    pub origin: Origin,
    /// La descripción del `Mark` que ha fijado el tipo.
    pub description: Option<String>,
}

/// Lo que puede decidir el tipo de un día, de menor a mayor prioridad (§2.2, RN-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Rule {
    Reduced,
    /// Un `Mark` `Working`: gana a `Reduced` y pierde con las ausencias (RN-26).
    Working,
    Vacation,
    Weekend,
    Holiday,
}

impl From<MarkKind> for Rule {
    fn from(kind: MarkKind) -> Self {
        match kind {
            MarkKind::Working => Self::Working,
            MarkKind::Reduced => Self::Reduced,
            MarkKind::Holiday => Self::Holiday,
            MarkKind::Vacation => Self::Vacation,
        }
    }
}

/// El `DayKind` que da una regla, con las variantes de verano en julio y agosto (RN-08).
const fn day_kind(rule: Option<Rule>, summer: bool) -> DayKind {
    match (rule, summer) {
        (None | Some(Rule::Working), false) => DayKind::Working,
        (None | Some(Rule::Working), true) => DayKind::SummerWorking,
        (Some(Rule::Reduced), false) => DayKind::Reduced,
        (Some(Rule::Reduced), true) => DayKind::SummerReduced,
        (Some(Rule::Vacation), _) => DayKind::Vacation,
        (Some(Rule::Weekend), _) => DayKind::Weekend,
        (Some(Rule::Holiday), _) => DayKind::Holiday,
    }
}

/// Todos los días del año `number`, resueltos (RN-05, RN-07, RN-08, RN-14).
pub fn resolve_year(calendar: &Calendar, number: i32) -> Result<Vec<ResolvedDay>, DomainError> {
    let year = calendar.year(number)?;
    let days = days_of_year(number)
        .map(|date| resolve_marks(calendar, year, date))
        .collect();
    Ok(if year.has_vacation_marks() {
        days
    } else {
        with_default_vacation(days, year.vacation_quota())
    })
}

/// El día `date`, resuelto. Su año debe estar en el calendario (RF-05).
pub fn resolve_day(calendar: &Calendar, date: NaiveDate) -> Result<ResolvedDay, DomainError> {
    resolve_year(calendar, date.year())?
        .into_iter()
        .find(|day| day.date == date)
        .ok_or_else(|| DomainError::InvalidDate(date.to_string()))
}

/// El tipo que dan los `Mark` y las reglas `Weekend` y `ReducedWeekday`: gana el de mayor
/// prioridad y, con la misma, el primero. Los `Mark` van antes que las reglas, y entre ellos el
/// más antiguo, así un `Mark` `Reduced` en viernes conserva su `Origin` y su descripción.
fn resolve_marks(calendar: &Calendar, year: &Year, date: NaiveDate) -> ResolvedDay {
    let weekend = calendar
        .is_weekend(date)
        .then_some((Rule::Weekend, Origin::Weekend, None));
    let reduced_day =
        calendar
            .is_reduced_day(date)
            .then_some((Rule::Reduced, Origin::ReducedWeekday, None));
    let winner = year
        .marks()
        .iter()
        .filter(|mark| mark.range.contains(date))
        .map(|mark| {
            (
                Rule::from(mark.kind),
                Origin::Mark(mark.id),
                mark.description.as_deref(),
            )
        })
        .chain(weekend)
        .chain(reduced_day)
        .fold(None, |best, candidate| match best {
            Some(best @ (rule, _, _)) if rule >= candidate.0 => Some(best),
            Some(_) | None => Some(candidate),
        });
    let kind = day_kind(winner.map(|(rule, _, _)| rule), Month::of(date).is_summer());
    ResolvedDay {
        date,
        kind,
        hours: kind.shift().map_or(0.0, |shift| year.hours().of(shift)),
        origin: winner.map_or(Origin::Default, |(_, origin, _)| origin),
        description: winner.and_then(|(_, _, description)| description.map(str::to_owned)),
    }
}

/// Mes desde el que se aplican las vacaciones por defecto, el 1 de julio (RN-14).
const DEFAULT_VACATION_FROM: Month = Month::July;

/// Convierte en vacaciones los primeros `quota` días laborables desde el 1 de julio (RN-14).
fn with_default_vacation(days: Vec<ResolvedDay>, quota: u32) -> Vec<ResolvedDay> {
    let chosen: HashSet<NaiveDate> = days
        .iter()
        .filter(|day| {
            Month::of(day.date) >= DEFAULT_VACATION_FROM && day.kind.category() == Category::Working
        })
        .take(usize::try_from(quota).unwrap_or(usize::MAX))
        .map(|day| day.date)
        .collect();
    days.into_iter()
        .map(|day| {
            if chosen.contains(&day.date) {
                ResolvedDay {
                    kind: DayKind::Vacation,
                    hours: 0.0,
                    origin: Origin::DefaultVacation,
                    description: None,
                    ..day
                }
            } else {
                day
            }
        })
        .collect()
}

/// Los días y horas laborables de un mes y el acumulado del año hasta él (RF-14, RN-19).
/// La jornada de referencia de RN-24, fija: no son las horas configuradas de ningún `Shift`.
pub const REFERENCE_DAY_HOURS: f64 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonthTotal {
    /// El mes.
    pub month: Month,
    /// Días laborables del mes (RN-09).
    pub working_days: usize,
    /// Suma de las horas de los días laborables del mes (RN-09).
    pub working_hours: f64,
    /// Horas laborables desde enero hasta este mes, incluido.
    pub cumulative_hours: f64,
}

/// Los totales del año (RF-08).
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    /// El año.
    pub year: i32,
    /// Días de cada tipo, en el orden de [`DayKind::ALL`].
    pub counts: [(DayKind, usize); 7],
    /// Días de los cuatro tipos laborables (RN-09).
    pub working_days: usize,
    /// Suma de las horas de los días laborables (RN-09).
    pub working_hours: f64,
    /// Días cuyo tipo efectivo es `Vacation` (RN-10).
    pub vacation_used: usize,
    /// El cupo de vacaciones del año.
    pub vacation_quota: u32,
    /// Los totales de cada mes, de enero a diciembre (RF-14).
    pub months: Vec<MonthTotal>,
}

impl Summary {
    /// Días de vacaciones restantes, nunca por debajo de cero.
    pub fn vacation_remaining(&self) -> usize {
        self.quota().saturating_sub(self.vacation_used)
    }

    /// Días de vacaciones por encima del cupo (RN-11).
    pub fn vacation_excess(&self) -> usize {
        self.vacation_used.saturating_sub(self.quota())
    }

    /// Las horas laborables del año repartidas entre los 12 meses (RN-24).
    pub fn monthly_average_hours(&self) -> f64 {
        self.working_hours / Month::ALL.len() as f64
    }

    /// La media mensual en días de [`REFERENCE_DAY_HOURS`] (RN-24).
    pub fn monthly_average_days(&self) -> f64 {
        self.monthly_average_hours() / REFERENCE_DAY_HOURS
    }

    pub fn month(&self, month: Month) -> Option<&MonthTotal> {
        self.months.iter().find(|total| total.month == month)
    }

    fn quota(&self) -> usize {
        usize::try_from(self.vacation_quota).unwrap_or(usize::MAX)
    }
}

/// El resumen del año `number` (RF-08).
pub fn summarize(calendar: &Calendar, number: i32) -> Result<Summary, DomainError> {
    Ok(summary_of(
        calendar.year(number)?,
        &resolve_year(calendar, number)?,
    ))
}

/// El [`Summary`] de un `Year` a partir de sus días ya resueltos con [`resolve_year`], para no
/// resolverlo otra vez cuando ya se tienen.
pub fn summary_of(year: &Year, days: &[ResolvedDay]) -> Summary {
    let count = |kind: DayKind| days.iter().filter(|day| day.kind == kind).count();
    let working = || {
        days.iter()
            .filter(|day| day.kind.category() == Category::Working)
    };
    Summary {
        year: year.number(),
        counts: DayKind::ALL.map(|kind| (kind, count(kind))),
        working_days: working().count(),
        working_hours: hours(working()),
        vacation_used: count(DayKind::Vacation),
        vacation_quota: year.vacation_quota(),
        months: month_totals(days),
    }
}

/// La suma de las horas de `days`. Empieza en 0.0 porque `Sum` de `f64` empieza en -0.0, que se
/// mostraría como «-0,0» en un mes sin días laborables.
fn hours<'a>(days: impl Iterator<Item = &'a ResolvedDay>) -> f64 {
    days.fold(0.0, |total, day| total + day.hours)
}

/// Los 12 `MonthTotal` del año, con el acumulado (RF-14, RN-19).
fn month_totals(days: &[ResolvedDay]) -> Vec<MonthTotal> {
    Month::ALL
        .into_iter()
        .map(|month| {
            let working = || {
                days.iter().filter(move |day| {
                    Month::of(day.date) == month && day.kind.category() == Category::Working
                })
            };
            (month, working().count(), hours(working()))
        })
        .scan(0.0, |cumulative, (month, working_days, working_hours)| {
            *cumulative += working_hours;
            Some(MonthTotal {
                month,
                working_days,
                working_hours,
                cumulative_hours: *cumulative,
            })
        })
        .collect()
}

/// Las vacaciones de un año en una fecha (RN-27): las disfrutadas hasta ese día, incluido, y el
/// cupo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VacationProgress {
    /// Días de `DayKind` `Vacation` hasta la fecha, incluidos los de `DefaultVacation`.
    pub taken: usize,
    /// El cupo de vacaciones del año.
    pub quota: u32,
}

impl VacationProgress {
    /// El progreso de `year` en `today`, a partir de sus días ya resueltos con [`resolve_year`].
    pub fn at(year: &Year, days: &[ResolvedDay], today: NaiveDate) -> Self {
        Self {
            taken: days
                .iter()
                .filter(|day| day.kind == DayKind::Vacation && day.date <= today)
                .count(),
            quota: year.vacation_quota(),
        }
    }

    /// Días del cupo aún sin disfrutar, nunca por debajo de cero.
    pub fn pending(&self) -> usize {
        usize::try_from(self.quota)
            .unwrap_or(usize::MAX)
            .saturating_sub(self.taken)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::{calendar, date, marked};
    use crate::domain::{DateRange, Shift};

    fn kind_of(calendar: &Calendar, day: NaiveDate) -> Result<(DayKind, Origin), DomainError> {
        resolve_day(calendar, day).map(|day| (day.kind, day.origin))
    }

    #[test]
    fn rn_05_the_kind_of_highest_priority_wins() {
        // Holiday > Weekend > Vacation > Reduced > Working.
        let calendar = marked(&[
            (MarkKind::Reduced, "2026-06-01", "2026-06-30"),
            (MarkKind::Vacation, "2026-06-08", "2026-06-14"),
            (MarkKind::Holiday, "2026-06-10", "2026-06-10"),
            (MarkKind::Holiday, "2026-06-13", "2026-06-13"),
            (MarkKind::Holiday, "2026-06-10", "2026-06-11"),
        ]);
        [
            (
                date(2026, 6, 2),
                Ok((DayKind::Reduced, Origin::Mark(MarkId(1)))),
            ),
            (
                date(2026, 6, 8),
                Ok((DayKind::Vacation, Origin::Mark(MarkId(2)))),
            ),
            (
                date(2026, 6, 10),
                Ok((DayKind::Holiday, Origin::Mark(MarkId(3)))),
            ),
            (
                date(2026, 6, 11),
                Ok((DayKind::Holiday, Origin::Mark(MarkId(5)))),
            ),
            (
                date(2026, 6, 12),
                Ok((DayKind::Vacation, Origin::Mark(MarkId(2)))),
            ),
            (
                date(2026, 6, 13),
                Ok((DayKind::Holiday, Origin::Mark(MarkId(4)))),
            ),
            (date(2026, 6, 14), Ok((DayKind::Weekend, Origin::Weekend))),
            (date(2026, 5, 28), Ok((DayKind::Working, Origin::Default))),
        ]
        .into_iter()
        .for_each(|(day, expected)| assert_eq!(kind_of(&calendar, day), expected, "{day}"));
    }

    #[test]
    fn rn_04_weekends_are_not_marked_but_computed() {
        [
            (date(2026, 6, 6), DayKind::Weekend),
            (date(2026, 6, 7), DayKind::Weekend),
            (date(2026, 6, 8), DayKind::Working),
        ]
        .into_iter()
        .for_each(|(day, kind)| {
            assert_eq!(
                kind_of(&calendar(), day).map(|(kind, _)| kind),
                Ok(kind),
                "{day}"
            );
        });
    }

    #[test]
    fn rn_08_working_and_reduced_days_change_in_summer() {
        let calendar = marked(&[
            (MarkKind::Reduced, "2026-06-30", "2026-07-01"),
            (MarkKind::Reduced, "2026-08-31", "2026-09-01"),
            (MarkKind::Vacation, "2026-01-02", "2026-01-02"),
        ]);
        [
            (date(2026, 6, 29), DayKind::Working),
            (date(2026, 6, 30), DayKind::Reduced),
            (date(2026, 7, 1), DayKind::SummerReduced),
            (date(2026, 7, 2), DayKind::SummerWorking),
            (date(2026, 8, 31), DayKind::SummerReduced),
            (date(2026, 9, 1), DayKind::Reduced),
            (date(2026, 9, 2), DayKind::Working),
        ]
        .into_iter()
        .for_each(|(day, kind)| {
            assert_eq!(
                kind_of(&calendar, day).map(|(kind, _)| kind),
                Ok(kind),
                "{day}"
            );
        });
    }

    #[test]
    fn rf_05_resolves_a_date_with_its_hours_and_description() {
        let described = calendar()
            .with_mark(
                MarkKind::Holiday,
                DateRange::day(date(2026, 3, 3)),
                Some("Fiesta local".to_owned()),
            )
            .map(|(calendar, _)| calendar);
        assert_eq!(
            described.and_then(|calendar| resolve_day(&calendar, date(2026, 3, 3))),
            Ok(ResolvedDay {
                date: date(2026, 3, 3),
                kind: DayKind::Holiday,
                hours: 0.0,
                origin: Origin::Mark(MarkId(1)),
                description: Some("Fiesta local".to_owned()),
            })
        );
        assert_eq!(
            resolve_day(&calendar(), date(2026, 3, 4)).map(|day| day.hours),
            Ok(8.5)
        );
    }

    #[test]
    fn rn_02_resolving_an_unknown_year_fails() {
        assert_eq!(
            resolve_day(&calendar(), date(2027, 1, 5)),
            Err(DomainError::UnknownYear(2027))
        );
        assert_eq!(
            summarize(&calendar(), 2027),
            Err(DomainError::UnknownYear(2027))
        );
    }

    #[test]
    fn rn_07_leap_years_resolve_366_days() {
        let calendar = Calendar::default().with_year(2024);
        assert_eq!(
            calendar
                .and_then(|calendar| resolve_year(&calendar, 2024))
                .map(|days| days.len()),
            Ok(366)
        );
    }

    #[test]
    fn rn_14_without_vacation_marks_vacation_fills_working_days_from_1_july() {
        // El 2026-07-01 es miércoles; 25 días laborables acaban el 5 de agosto, saltando fines de
        // semana y el festivo del 2 de julio. El reducido del 3 de julio es laborable y se toma.
        let calendar = marked(&[
            (MarkKind::Holiday, "2026-07-02", "2026-07-02"),
            (MarkKind::Reduced, "2026-07-03", "2026-07-03"),
        ]);
        let days = resolve_year(&calendar, 2026);
        let vacation: Result<Vec<NaiveDate>, DomainError> = days.map(|days| {
            days.into_iter()
                .filter(|day| day.origin == Origin::DefaultVacation)
                .map(|day| day.date)
                .collect()
        });
        assert_eq!(vacation.as_ref().map(Vec::len), Ok(25));
        assert_eq!(
            vacation.as_ref().map(|days| days.first().copied()),
            Ok(Some(date(2026, 7, 1)))
        );
        assert_eq!(
            vacation.as_ref().map(|days| days.get(1).copied()),
            Ok(Some(date(2026, 7, 3)))
        );
        assert_eq!(
            vacation.as_ref().map(|days| days.last().copied()),
            Ok(Some(date(2026, 8, 5)))
        );
        [
            (
                date(2026, 7, 2),
                Ok((DayKind::Holiday, Origin::Mark(MarkId(1)))),
            ),
            (date(2026, 7, 4), Ok((DayKind::Weekend, Origin::Weekend))),
            (
                date(2026, 8, 6),
                Ok((DayKind::SummerWorking, Origin::Default)),
            ),
            (date(2026, 6, 30), Ok((DayKind::Working, Origin::Default))),
        ]
        .into_iter()
        .for_each(|(day, expected)| assert_eq!(kind_of(&calendar, day), expected, "{day}"));
    }

    #[test]
    fn rn_14_any_vacation_mark_disables_the_default_vacation() {
        let calendar = marked(&[(MarkKind::Vacation, "2026-12-24", "2026-12-24")]);
        assert_eq!(
            kind_of(&calendar, date(2026, 7, 1)),
            Ok((DayKind::SummerWorking, Origin::Default))
        );
        assert_eq!(
            summarize(&calendar, 2026).map(|summary| summary.vacation_used),
            Ok(1)
        );
    }

    #[test]
    fn rn_14_the_default_vacation_stops_at_the_end_of_the_year() {
        let calendar = Calendar::default()
            .with_year(2026)
            .and_then(|calendar| calendar.with_quota(2026, 500));
        // 184 días de julio a diciembre, menos 52 de fin de semana.
        assert_eq!(
            calendar
                .and_then(|calendar| summarize(&calendar, 2026))
                .map(|summary| summary.vacation_used),
            Ok(132)
        );
    }

    #[test]
    fn rn_24_the_monthly_average_divides_the_year_hours_by_12_and_by_8_for_days() {
        // Con el calendario por defecto, los meses tienen horas distintas; la media no es la de
        // ningún mes, sino el total entre 12.
        let summary = summarize(&calendar(), 2026);
        assert_eq!(
            summary
                .as_ref()
                .map(|summary| summary.monthly_average_hours() * 12.0 - summary.working_hours),
            Ok(0.0)
        );
        let empty = Summary {
            working_hours: 0.0,
            ..summary.clone().expect("summary")
        };
        assert_eq!(empty.monthly_average_hours(), 0.0, "un año sin horas");
        let year = Summary {
            working_hours: 1920.0,
            ..empty
        };
        assert_eq!(
            (year.monthly_average_hours(), year.monthly_average_days()),
            (160.0, 20.0),
            "160 h al mes son 20 días de 8 h"
        );
    }

    #[test]
    fn rn_09_rn_10_rf_08_summarizes_the_year() {
        // 2026 tiene 365 días y 104 de fin de semana. El festivo del sábado 15 de agosto cuenta como
        // festivo, no como fin de semana; las vacaciones del viernes 5 al viernes 12 de junio usan
        // 6 días. Hay 52 viernes (RN-18): 9 en verano y 2 dentro de las vacaciones; el 31 de julio
        // ya es viernes, así que su marca no añade nada.
        let calendar = marked(&[
            (MarkKind::Holiday, "2026-01-01", "2026-01-01"),
            (MarkKind::Holiday, "2026-08-15", "2026-08-15"),
            (MarkKind::Vacation, "2026-06-05", "2026-06-12"),
            (MarkKind::Reduced, "2026-12-24", "2026-12-24"),
            (MarkKind::Reduced, "2026-07-31", "2026-07-31"),
        ]);
        let summary = summarize(&calendar, 2026);
        assert_eq!(
            summary.as_ref().map(|summary| summary.counts),
            Ok([
                (DayKind::Working, 168),
                (DayKind::Reduced, 42),
                (DayKind::SummerWorking, 35),
                (DayKind::SummerReduced, 9),
                (DayKind::Weekend, 103),
                (DayKind::Holiday, 2),
                (DayKind::Vacation, 6),
            ])
        );
        assert_eq!(
            summary.as_ref().map(|summary| summary.working_days),
            Ok(254)
        );
        assert_eq!(
            summary.as_ref().map(|summary| summary.working_hours),
            Ok(203.0 * 8.5 + 51.0 * 6.0)
        );
        assert_eq!(
            summary
                .as_ref()
                .map(|summary| (summary.vacation_remaining(), summary.vacation_excess())),
            Ok((19, 0))
        );
    }

    #[test]
    fn rn_26_a_working_mark_restores_the_full_shift_only_on_reduced_days() {
        // 2026-03-06 y 2026-07-03 son viernes; 2026-03-07 es sábado.
        let calendar = marked(&[
            (MarkKind::Working, "2026-03-02", "2026-03-08"),
            (MarkKind::Working, "2026-07-03", "2026-07-03"),
            (MarkKind::Reduced, "2026-03-04", "2026-03-04"),
            (MarkKind::Holiday, "2026-03-05", "2026-03-05"),
            (MarkKind::Vacation, "2026-03-03", "2026-03-03"),
        ]);
        let resolved =
            |day| resolve_day(&calendar, day).map(|day| (day.kind, day.hours, day.origin));
        [
            (
                date(2026, 3, 6),
                (DayKind::Working, 8.5, Origin::Mark(MarkId(1))),
                "viernes",
            ),
            (
                date(2026, 7, 3),
                (DayKind::SummerWorking, 8.5, Origin::Mark(MarkId(2))),
                "viernes de verano",
            ),
            (
                date(2026, 3, 4),
                (DayKind::Working, 8.5, Origin::Mark(MarkId(1))),
                "gana a un Mark Reduced",
            ),
            (
                date(2026, 3, 5),
                (DayKind::Holiday, 0.0, Origin::Mark(MarkId(4))),
                "no a un festivo",
            ),
            (
                date(2026, 3, 3),
                (DayKind::Vacation, 0.0, Origin::Mark(MarkId(5))),
                "no a unas vacaciones",
            ),
            (
                date(2026, 3, 7),
                (DayKind::Weekend, 0.0, Origin::Weekend),
                "no a un fin de semana",
            ),
            (
                date(2026, 3, 2),
                (DayKind::Working, 8.5, Origin::Mark(MarkId(1))),
                "un lunes ya era laborable",
            ),
        ]
        .into_iter()
        .for_each(|(day, expected, case)| {
            assert_eq!(resolved(day), Ok(expected), "{day}: {case}");
        });
    }

    #[test]
    fn rn_18_fridays_are_reduced_with_summer_and_rest_of_year_hours() {
        let calendar = marked(&[
            (MarkKind::Holiday, "2026-05-01", "2026-05-01"),
            (MarkKind::Vacation, "2026-06-12", "2026-06-12"),
            (MarkKind::Reduced, "2026-06-19", "2026-06-19"),
        ])
        .with_shift_hours(2026, Shift::Reduced, 7.0)
        .and_then(|calendar| calendar.with_shift_hours(2026, Shift::SummerReduced, 5.0))
        .expect("hours");
        let resolved =
            |day| resolve_day(&calendar, day).map(|day| (day.kind, day.hours, day.origin));
        [
            (
                date(2026, 6, 5),
                Ok((DayKind::Reduced, 7.0, Origin::ReducedWeekday)),
            ),
            (
                date(2026, 7, 3),
                Ok((DayKind::SummerReduced, 5.0, Origin::ReducedWeekday)),
            ),
            (
                date(2026, 6, 4),
                Ok((DayKind::Working, 8.5, Origin::Default)),
            ),
            (
                date(2026, 5, 1),
                Ok((DayKind::Holiday, 0.0, Origin::Mark(MarkId(1)))),
            ),
            (
                date(2026, 6, 12),
                Ok((DayKind::Vacation, 0.0, Origin::Mark(MarkId(2)))),
            ),
            (
                date(2026, 6, 19),
                Ok((DayKind::Reduced, 7.0, Origin::Mark(MarkId(3)))),
            ),
        ]
        .into_iter()
        .for_each(|(day, expected)| assert_eq!(resolved(day), expected, "{day}"));
    }

    #[test]
    fn rn_18_without_reduced_days_fridays_are_working_days() {
        let calendar =
            marked(&[(MarkKind::Vacation, "2026-01-02", "2026-01-02")]).with_reduced_days(&[]);
        assert_eq!(
            kind_of(&calendar, date(2026, 6, 5)),
            Ok((DayKind::Working, Origin::Default))
        );
    }

    #[test]
    fn rn_25_changing_the_hours_of_a_year_leaves_the_other_years_alone() {
        let before = calendar().with_year(2027).expect("2027");
        let after = before
            .clone()
            .with_shift_hours(2027, Shift::Working, 7.0)
            .expect("hours");
        let hours = |calendar: &Calendar, year| {
            summarize(calendar, year).map(|summary| summary.working_hours)
        };
        assert_eq!(hours(&after, 2026), hours(&before, 2026), "2026 no cambia");
        assert_ne!(hours(&after, 2027), hours(&before, 2027), "2027 sí");
        assert_eq!(
            resolve_day(&after, date(2027, 3, 3)).map(|day| day.hours),
            Ok(7.0)
        );
    }

    #[test]
    fn rn_09_working_hours_use_the_configured_shifts() {
        let calendar = marked(&[(MarkKind::Vacation, "2026-01-02", "2026-01-02")])
            .with_shift_hours(2026, Shift::SummerWorking, 7.0);
        assert_eq!(
            calendar
                .and_then(|calendar| resolve_day(&calendar, date(2026, 7, 1)))
                .map(|day| day.hours),
            Ok(7.0)
        );
    }

    #[test]
    fn rf_14_months_have_their_working_days_hours_and_cumulative_hours() {
        // Sin marcas de vacaciones, las vacaciones por defecto ocupan todo julio y el 3 y 4 de
        // agosto. Enero: 17 días de 8,5 h y 5 viernes de 6 h (RN-18).
        let summary = summarize(&calendar(), 2026).expect("summary");
        let total = |month| {
            summary.month(month).map(|total| {
                (
                    total.working_days,
                    total.working_hours,
                    total.cumulative_hours,
                )
            })
        };
        assert_eq!(summary.months.len(), 12);
        assert_eq!(total(Month::January), Some((22, 174.5, 174.5)));
        assert_eq!(total(Month::February), Some((20, 160.0, 334.5)));
        // Acumulado hasta junio: 174,5 + 160 + 177 + 177 + 166 + 177.
        assert_eq!(total(Month::June), Some((22, 177.0, 1031.5)));
        assert_eq!(total(Month::July), Some((0, 0.0, 1031.5)));
        assert_eq!(total(Month::August), Some((19, 151.5, 1183.0)));
        assert!(total(Month::July).is_some_and(|(_, hours, _)| hours.is_sign_positive()));
    }

    #[test]
    fn rn_19_months_add_up_to_the_year() {
        [
            calendar(),
            marked(&[
                (MarkKind::Holiday, "2026-01-01", "2026-01-01"),
                (MarkKind::Vacation, "2026-06-05", "2026-06-12"),
                (MarkKind::Reduced, "2026-07-31", "2026-08-05"),
            ]),
            calendar()
                .with_shift_hours(2026, Shift::SummerWorking, 7.5)
                .and_then(|calendar| calendar.with_shift_hours(2026, Shift::SummerReduced, 7.0))
                .expect("hours"),
        ]
        .into_iter()
        .enumerate()
        .for_each(|(case, calendar)| {
            let summary = summarize(&calendar, 2026).expect("summary");
            let days: usize = summary.months.iter().map(|total| total.working_days).sum();
            let hours: f64 = summary.months.iter().map(|total| total.working_hours).sum();
            let december = summary
                .month(Month::December)
                .map(|total| total.cumulative_hours);
            assert_eq!(
                (days, hours, december),
                (
                    summary.working_days,
                    summary.working_hours,
                    Some(summary.working_hours)
                ),
                "caso {case}"
            );
        });
    }

    #[test]
    fn rn_10_rn_11_vacation_above_the_quota_is_counted_as_excess() {
        let summary = marked(&[(MarkKind::Vacation, "2026-06-01", "2026-07-31")])
            .with_quota(2026, 25)
            .and_then(|calendar| summarize(&calendar, 2026));
        // 61 días menos 16 de fin de semana.
        assert_eq!(
            summary.map(|summary| (
                summary.vacation_used,
                summary.vacation_remaining(),
                summary.vacation_excess()
            )),
            Ok((45, 0, 20))
        );
    }

    #[test]
    fn rn_27_taken_vacation_counts_vacation_days_up_to_today_included() {
        let marks = marked(&[(MarkKind::Vacation, "2026-06-05", "2026-06-12")]);
        let progress = |calendar: &Calendar, today: NaiveDate| {
            calendar.year(2026).and_then(|year| {
                resolve_year(calendar, 2026)
                    .map(|days| VacationProgress::at(year, &days, today))
                    .map(|progress| (progress.taken, progress.pending()))
            })
        };
        [
            ("antes de empezar", &marks, date(2026, 6, 4), (0, 25)),
            ("el primer día cuenta", &marks, date(2026, 6, 5), (1, 24)),
            ("sin el fin de semana", &marks, date(2026, 6, 8), (2, 23)),
            ("después de terminar", &marks, date(2026, 12, 31), (6, 19)),
            (
                "por defecto, a mitad",
                &calendar(),
                date(2026, 7, 3),
                (3, 22),
            ),
            (
                "por defecto, completas",
                &calendar(),
                date(2026, 11, 13),
                (25, 0),
            ),
        ]
        .into_iter()
        .for_each(|(case, calendar, today, expected)| {
            assert_eq!(progress(calendar, today), Ok(expected), "caso {case}");
        });
    }

    #[test]
    fn rn_27_pending_vacation_never_goes_below_zero() {
        [(10, 25, 15), (25, 25, 0), (30, 25, 0), (0, 0, 0)]
            .into_iter()
            .for_each(|(taken, quota, pending)| {
                assert_eq!(
                    VacationProgress { taken, quota }.pending(),
                    pending,
                    "{taken} de {quota}"
                );
            });
    }
}
