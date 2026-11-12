//! Casos de uso: una función pura de un `Calendar` y un [`Command`] a un [`Outcome`].
//!
//! Quien la llama carga el calendario, ejecuta [`execute`], guarda el calendario nuevo si lo hay
//! y muestra el resultado. Aquí no hay E/S.

use chrono::{Datelike, NaiveDate, Weekday};

use crate::domain::{
    Calendar, DEFAULT_VACATION_QUOTA, DateRange, DomainError, Mark, MarkId, MarkKind, Marking,
    Month, MonthExt, ResolvedDay, Shift, Summary, Year, resolve_day, resolve_year, summarize,
    summary_of,
};

/// Una operación pedida por el usuario.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// RF-01: añadir un año.
    AddYear(i32),
    /// RF-02: marcar un día, o un rango si se indica `to`.
    Mark {
        /// Lo que se marca.
        kind: MarkKind,
        /// Primer día.
        from: NaiveDate,
        /// Último día; si falta, es `from`.
        to: Option<NaiveDate>,
        /// Texto opcional; un texto en blanco cuenta como ausente.
        description: Option<String>,
    },
    /// RF-03: eliminar un `Mark`.
    Unmark(MarkId),
    /// RF-04: listar los `Mark` de todos los años o de uno.
    List(Option<i32>),
    /// RF-05: mostrar un día.
    Day(NaiveDate),
    /// RF-06: mostrar un mes.
    Month {
        /// El año.
        year: i32,
        /// El mes.
        month: Month,
    },
    /// RF-07: mostrar un año completo.
    Year(i32),
    /// RF-08: resumir un año.
    Summary(i32),
    /// RF-15: consultar la configuración.
    Config,
    /// RF-16: configurar los días de la regla `ReducedWeekday`; vacío, ninguno.
    ReducedDays(Vec<Weekday>),
    /// RF-09: configurar las horas de un `Shift` en un `Year` (RN-25).
    Hours { year: i32, shift: Shift, hours: f64 },
    /// RF-10: configurar el cupo de vacaciones de un año.
    Quota {
        /// El año.
        year: i32,
        /// Días de vacaciones.
        days: u32,
    },
}

/// Lo que ha producido un comando.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// El calendario ha cambiado y hay que guardarlo.
    Changed {
        /// El calendario nuevo.
        calendar: Calendar,
        /// Lo que ha pasado.
        event: Event,
    },
    /// El comando no ha cambiado nada, así que no hay nada que guardar.
    Unchanged(Event),
    /// El resultado de una consulta.
    Report(Report),
}

/// Un cambio hecho, o no hecho, en el calendario.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Se ha añadido un año con el cupo por defecto.
    YearAdded {
        /// El año.
        year: i32,
        /// Su cupo de vacaciones.
        quota: u32,
    },
    /// Se ha añadido un `Mark`.
    Marked {
        /// El `Mark` nuevo.
        mark: Mark,
        /// Días de vacaciones por encima del cupo del año tras añadirlo; 0 en otros tipos (RN-11).
        excess: usize,
    },
    /// Ya existía un `Mark` idéntico (RN-06).
    AlreadyMarked(MarkId),
    /// Se ha eliminado un `Mark`.
    Unmarked(Mark),
    /// Se han configurado los días de jornada reducida, de lunes a domingo.
    ReducedDaysSet(Vec<Weekday>),
    /// Se han configurado las horas de un `Shift` en un `Year`.
    HoursSet { year: i32, shift: Shift, hours: f64 },
    /// Se ha configurado el cupo de un año.
    QuotaSet {
        /// El año.
        year: i32,
        /// Días de vacaciones.
        days: u32,
    },
}

/// El resultado de una consulta.
#[derive(Debug, Clone, PartialEq)]
pub enum Report {
    /// `Mark` ordenados por su primer día.
    Marks(Vec<Mark>),
    /// Un día.
    Day(ResolvedDay),
    /// Un mes y el resumen del año, para las vacaciones restantes.
    Month(MonthView, Summary),
    /// Todos los meses de un año y su resumen.
    Year(Vec<MonthView>, Summary),
    /// El resumen de un año.
    Summary(Summary),
    /// La configuración: el `Calendar`, del que se muestran sus días y cada `Year` (RF-15).
    Config(Calendar),
}

/// Los días resueltos de un mes y los `Mark` que lo tocan.
#[derive(Debug, Clone, PartialEq)]
pub struct MonthView {
    /// El año.
    pub year: i32,
    /// El mes.
    pub month: Month,
    /// Todos los días del mes.
    pub days: Vec<ResolvedDay>,
    /// `Mark` con al menos un día en el mes, ordenados por su primer día.
    pub marks: Vec<Mark>,
}

/// Ejecuta `command` sobre `calendar`. Un error deja el calendario como estaba (RN-13).
pub fn execute(calendar: Calendar, command: Command) -> Result<Outcome, DomainError> {
    match command {
        Command::AddYear(year) => Ok(Outcome::Changed {
            calendar: calendar.with_year(year)?,
            event: Event::YearAdded {
                year,
                quota: DEFAULT_VACATION_QUOTA,
            },
        }),
        Command::Mark {
            kind,
            from,
            to,
            description,
        } => mark(
            calendar,
            kind,
            DateRange::new(from, to.unwrap_or(from))?,
            description,
        ),
        Command::Unmark(id) => calendar
            .without_mark(id)
            .map(|(calendar, mark)| Outcome::Changed {
                calendar,
                event: Event::Unmarked(mark),
            }),
        Command::List(year) => {
            list(&calendar, year).map(|marks| Outcome::Report(Report::Marks(marks)))
        }
        Command::Day(date) => {
            resolve_day(&calendar, date).map(|day| Outcome::Report(Report::Day(day)))
        }
        Command::Month { year, month } => {
            let (days, year) = (resolve_year(&calendar, year)?, calendar.year(year)?);
            Ok(Outcome::Report(Report::Month(
                month_view(year, month, &days)?,
                summary_of(year, &days),
            )))
        }
        Command::Year(year) => {
            let (days, year) = (resolve_year(&calendar, year)?, calendar.year(year)?);
            Ok(Outcome::Report(Report::Year(
                Month::ALL
                    .into_iter()
                    .map(|month| month_view(year, month, &days))
                    .collect::<Result<_, _>>()?,
                summary_of(year, &days),
            )))
        }
        Command::Config => Ok(Outcome::Report(Report::Config(calendar))),
        Command::ReducedDays(days) => {
            let calendar = calendar.with_reduced_days(&days);
            Ok(Outcome::Changed {
                event: Event::ReducedDaysSet(calendar.reduced_days().to_vec()),
                calendar,
            })
        }
        Command::Summary(year) => {
            summarize(&calendar, year).map(|summary| Outcome::Report(Report::Summary(summary)))
        }
        Command::Hours { year, shift, hours } => Ok(Outcome::Changed {
            calendar: calendar.with_shift_hours(year, shift, hours)?,
            event: Event::HoursSet { year, shift, hours },
        }),
        Command::Quota { year, days } => Ok(Outcome::Changed {
            calendar: calendar.with_quota(year, days)?,
            event: Event::QuotaSet { year, days },
        }),
    }
}

fn mark(
    calendar: Calendar,
    kind: MarkKind,
    range: DateRange,
    description: Option<String>,
) -> Result<Outcome, DomainError> {
    let description = description
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty());
    match calendar.with_mark(kind, range, description)? {
        (_, Marking::Existing(id)) => Ok(Outcome::Unchanged(Event::AlreadyMarked(id))),
        (calendar, Marking::Added(mark)) => {
            let excess = match mark.kind {
                MarkKind::Vacation => {
                    summarize(&calendar, mark.range.from().year())?.vacation_excess()
                }
                MarkKind::Reduced | MarkKind::Holiday => 0,
            };
            Ok(Outcome::Changed {
                calendar,
                event: Event::Marked { mark, excess },
            })
        }
    }
}

fn sorted(marks: impl Iterator<Item = Mark>) -> Vec<Mark> {
    let mut marks: Vec<Mark> = marks.collect();
    marks.sort_by_key(|mark| (mark.range.from(), mark.id));
    marks
}

fn list(calendar: &Calendar, year: Option<i32>) -> Result<Vec<Mark>, DomainError> {
    match year {
        Some(year) => calendar
            .year(year)
            .map(|year| sorted(year.marks().iter().cloned())),
        None => Ok(sorted(calendar.marks().cloned())),
    }
}

/// La vista de un mes a partir de los días del año ya resueltos, que se resuelven una sola vez
/// aunque la vista de año pida los 12 meses.
fn month_view(year: &Year, month: Month, days: &[ResolvedDay]) -> Result<MonthView, DomainError> {
    let range = DateRange::month(year.number(), month)?;
    Ok(MonthView {
        year: year.number(),
        month,
        days: days
            .iter()
            .filter(|day| range.contains(day.date))
            .cloned()
            .collect(),
        marks: sorted(
            year.marks()
                .iter()
                .filter(|mark| mark.range.overlaps(range))
                .cloned(),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::{calendar, date, marked, range};
    use crate::domain::{DayKind, Origin};

    fn mark_command(kind: MarkKind, from: NaiveDate, to: Option<NaiveDate>) -> Command {
        Command::Mark {
            kind,
            from,
            to,
            description: None,
        }
    }

    fn changed(outcome: Result<Outcome, DomainError>) -> Option<(Calendar, Event)> {
        match outcome {
            Ok(Outcome::Changed { calendar, event }) => Some((calendar, event)),
            Ok(Outcome::Unchanged(_) | Outcome::Report(_)) | Err(_) => None,
        }
    }

    fn report(calendar: Calendar, command: Command) -> Option<Report> {
        match execute(calendar, command) {
            Ok(Outcome::Report(report)) => Some(report),
            Ok(Outcome::Changed { .. } | Outcome::Unchanged(_)) | Err(_) => None,
        }
    }

    #[test]
    fn rf_01_add_year_changes_the_calendar() {
        let outcome = changed(execute(Calendar::default(), Command::AddYear(2027)));
        assert_eq!(
            outcome.map(|(calendar, event)| (calendar.years().len(), event)),
            Some((
                1,
                Event::YearAdded {
                    year: 2027,
                    quota: 25
                }
            ))
        );
    }

    #[test]
    fn rf_02_marks_a_day_or_a_range_with_a_trimmed_description() {
        let day = changed(execute(
            calendar(),
            Command::Mark {
                kind: MarkKind::Holiday,
                from: date(2026, 3, 3),
                to: None,
                description: Some("  Fiesta local ".to_owned()),
            },
        ));
        assert_eq!(
            day.map(|(_, event)| event),
            Some(Event::Marked {
                mark: Mark {
                    id: MarkId(1),
                    kind: MarkKind::Holiday,
                    range: crate::domain::DateRange::day(date(2026, 3, 3)),
                    description: Some("Fiesta local".to_owned()),
                },
                excess: 0,
            })
        );
        let blank = changed(execute(
            calendar(),
            Command::Mark {
                kind: MarkKind::Vacation,
                from: date(2026, 6, 5),
                to: Some(date(2026, 6, 12)),
                description: Some("   ".to_owned()),
            },
        ));
        assert_eq!(
            blank.map(|(_, event)| event),
            Some(Event::Marked {
                mark: Mark {
                    id: MarkId(1),
                    kind: MarkKind::Vacation,
                    range: range(date(2026, 6, 5), date(2026, 6, 12)),
                    description: None,
                },
                excess: 0,
            })
        );
    }

    #[test]
    fn rn_06_marking_twice_changes_nothing() {
        let calendar = marked(&[(MarkKind::Holiday, "2026-03-03", "2026-03-03")]);
        assert_eq!(
            execute(
                calendar,
                mark_command(MarkKind::Holiday, date(2026, 3, 3), None)
            ),
            Ok(Outcome::Unchanged(Event::AlreadyMarked(MarkId(1))))
        );
    }

    #[test]
    fn rn_11_vacation_above_the_quota_is_saved_with_the_excess() {
        let calendar = marked(&[(MarkKind::Vacation, "2026-06-01", "2026-06-30")]);
        // 22 días en junio más 5 en la primera semana de julio: 2 por encima de 25.
        let outcome = changed(execute(
            calendar,
            mark_command(MarkKind::Vacation, date(2026, 7, 1), Some(date(2026, 7, 7))),
        ));
        assert_eq!(
            outcome.map(|(calendar, event)| (calendar.marks().count(), event)),
            Some((
                2,
                Event::Marked {
                    mark: Mark {
                        id: MarkId(2),
                        kind: MarkKind::Vacation,
                        range: range(date(2026, 7, 1), date(2026, 7, 7)),
                        description: None,
                    },
                    excess: 2,
                }
            ))
        );
        let holiday = changed(execute(
            marked(&[(MarkKind::Vacation, "2026-01-01", "2026-12-31")]),
            mark_command(MarkKind::Holiday, date(2026, 3, 3), None),
        ));
        assert!(
            matches!(holiday, Some((_, Event::Marked { excess: 0, .. }))),
            "{holiday:?}"
        );
    }

    #[test]
    fn rn_03_rn_13_invalid_operations_fail_without_a_new_calendar() {
        [
            (
                mark_command(MarkKind::Holiday, date(2026, 3, 3), Some(date(2026, 3, 1))),
                DomainError::ReversedRange {
                    from: date(2026, 3, 3),
                    to: date(2026, 3, 1),
                },
            ),
            (
                mark_command(MarkKind::Holiday, date(2027, 3, 3), None),
                DomainError::UnknownYear(2027),
            ),
            (Command::AddYear(2026), DomainError::DuplicateYear(2026)),
            (
                Command::Unmark(MarkId(3)),
                DomainError::UnknownMark(MarkId(3)),
            ),
            (Command::List(Some(2027)), DomainError::UnknownYear(2027)),
            (
                Command::Day(date(2027, 1, 1)),
                DomainError::UnknownYear(2027),
            ),
            (
                Command::Month {
                    year: 2027,
                    month: Month::January,
                },
                DomainError::UnknownYear(2027),
            ),
            (Command::Year(2027), DomainError::UnknownYear(2027)),
            (Command::Summary(2027), DomainError::UnknownYear(2027)),
            (
                Command::Hours {
                    year: 2026,
                    shift: Shift::Reduced,
                    hours: 25.0,
                },
                DomainError::InvalidHours(25.0),
            ),
            (
                Command::Hours {
                    year: 2027,
                    shift: Shift::Reduced,
                    hours: 6.0,
                },
                DomainError::UnknownYear(2027),
            ),
            (
                Command::Quota {
                    year: 2027,
                    days: 3,
                },
                DomainError::UnknownYear(2027),
            ),
        ]
        .into_iter()
        .for_each(|(command, error)| {
            assert_eq!(
                execute(calendar(), command.clone()),
                Err(error),
                "{command:?}"
            );
        });
    }

    #[test]
    fn rf_03_unmark_returns_the_removed_mark() {
        let calendar = marked(&[(MarkKind::Holiday, "2026-03-03", "2026-03-03")]);
        let outcome = changed(execute(calendar, Command::Unmark(MarkId(1))));
        assert_eq!(
            outcome.map(|(calendar, event)| (calendar.marks().count(), event)),
            Some((
                0,
                Event::Unmarked(Mark {
                    id: MarkId(1),
                    kind: MarkKind::Holiday,
                    range: crate::domain::DateRange::day(date(2026, 3, 3)),
                    description: None,
                })
            ))
        );
    }

    #[test]
    fn rf_04_lists_marks_of_every_year_or_one_ordered_by_date() {
        let calendar = marked(&[
            (MarkKind::Holiday, "2026-12-08", "2026-12-08"),
            (MarkKind::Holiday, "2026-03-03", "2026-03-03"),
        ])
        .with_year(2027)
        .and_then(|calendar| {
            calendar.with_mark(
                MarkKind::Holiday,
                crate::domain::DateRange::day(date(2027, 1, 1)),
                None,
            )
        })
        .map(|(calendar, _)| calendar);
        let ids = |year| {
            calendar
                .clone()
                .ok()
                .and_then(|calendar| report(calendar, Command::List(year)))
                .and_then(|report| match report {
                    Report::Marks(marks) => {
                        Some(marks.iter().map(|mark| mark.id.0).collect::<Vec<_>>())
                    }
                    Report::Day(_)
                    | Report::Month(..)
                    | Report::Year(..)
                    | Report::Summary(_)
                    | Report::Config(_) => None,
                })
        };
        assert_eq!(ids(None), Some(vec![2, 1, 3]));
        assert_eq!(ids(Some(2026)), Some(vec![2, 1]));
        assert_eq!(ids(Some(2027)), Some(vec![3]));
    }

    #[test]
    fn rf_05_day_reports_the_resolved_day() {
        let day = report(calendar(), Command::Day(date(2026, 6, 6)));
        assert_eq!(
            day,
            Some(Report::Day(ResolvedDay {
                date: date(2026, 6, 6),
                kind: DayKind::Weekend,
                hours: 0.0,
                origin: Origin::Weekend,
                description: None,
            }))
        );
    }

    #[test]
    fn rf_06_month_has_its_days_and_the_marks_that_touch_it() {
        let calendar = marked(&[
            (MarkKind::Vacation, "2026-05-28", "2026-06-02"),
            (MarkKind::Holiday, "2026-07-01", "2026-07-01"),
            (MarkKind::Holiday, "2026-06-24", "2026-06-24"),
        ]);
        let month = report(
            calendar,
            Command::Month {
                year: 2026,
                month: Month::June,
            },
        );
        let summary = match month {
            Some(Report::Month(view, summary)) => {
                assert_eq!(
                    (view.year, view.month, view.days.len()),
                    (2026, Month::June, 30)
                );
                assert_eq!(
                    view.days.first().map(|day| day.date),
                    Some(date(2026, 6, 1))
                );
                assert_eq!(
                    view.marks.iter().map(|mark| mark.id.0).collect::<Vec<_>>(),
                    [1, 3]
                );
                Some(summary.vacation_used)
            }
            Some(
                Report::Marks(_)
                | Report::Day(_)
                | Report::Year(..)
                | Report::Summary(_)
                | Report::Config(_),
            )
            | None => None,
        };
        assert_eq!(summary, Some(4));
    }

    #[test]
    fn rf_07_year_has_twelve_months() {
        let year = report(calendar(), Command::Year(2026));
        let months = match year {
            Some(Report::Year(months, _)) => Some(
                months
                    .iter()
                    .map(|month| (month.month, month.days.len()))
                    .collect::<Vec<_>>(),
            ),
            Some(
                Report::Marks(_)
                | Report::Day(_)
                | Report::Month(..)
                | Report::Summary(_)
                | Report::Config(_),
            )
            | None => None,
        };
        assert_eq!(
            months,
            Some(vec![
                (Month::January, 31),
                (Month::February, 28),
                (Month::March, 31),
                (Month::April, 30),
                (Month::May, 31),
                (Month::June, 30),
                (Month::July, 31),
                (Month::August, 31),
                (Month::September, 30),
                (Month::October, 31),
                (Month::November, 30),
                (Month::December, 31)
            ])
        );
    }

    #[test]
    fn rf_08_summary_reports_the_year() {
        let summary = report(calendar(), Command::Summary(2026));
        let used = match summary {
            Some(Report::Summary(summary)) => Some((summary.year, summary.vacation_used)),
            Some(
                Report::Marks(_)
                | Report::Day(_)
                | Report::Month(..)
                | Report::Year(..)
                | Report::Config(_),
            )
            | None => None,
        };
        // Sin marcas de vacaciones: las vacaciones por defecto usan todo el cupo (RN-14).
        assert_eq!(used, Some((2026, 25)));
    }

    #[test]
    fn rf_16_sets_the_reduced_days_in_week_order() {
        let set = changed(execute(
            calendar(),
            Command::ReducedDays(vec![Weekday::Fri, Weekday::Mon]),
        ));
        assert_eq!(
            set.map(|(calendar, event)| (calendar.reduced_days().to_vec(), event)),
            Some((
                vec![Weekday::Mon, Weekday::Fri],
                Event::ReducedDaysSet(vec![Weekday::Mon, Weekday::Fri])
            ))
        );
        let none = changed(execute(calendar(), Command::ReducedDays(Vec::new())));
        assert_eq!(
            none.map(|(calendar, _)| calendar.reduced_days().is_empty()),
            Some(true)
        );
    }

    #[test]
    fn rf_15_config_reports_the_whole_configuration() {
        let calendar = calendar().with_year(2027).expect("2027");
        assert_eq!(
            execute(calendar.clone(), Command::Config),
            Ok(Outcome::Report(Report::Config(calendar)))
        );
    }

    #[test]
    fn rf_09_rf_10_set_hours_and_quota() {
        let hours = changed(execute(
            calendar(),
            Command::Hours {
                year: 2026,
                shift: Shift::SummerWorking,
                hours: 7.5,
            },
        ));
        assert_eq!(
            hours.map(|(calendar, event)| (
                calendar
                    .year(2026)
                    .map(|year| year.hours().of(Shift::SummerWorking)),
                event
            )),
            Some((
                Ok(7.5),
                Event::HoursSet {
                    year: 2026,
                    shift: Shift::SummerWorking,
                    hours: 7.5
                }
            ))
        );
        let quota = changed(execute(
            calendar(),
            Command::Quota {
                year: 2026,
                days: 22,
            },
        ));
        assert_eq!(
            quota.map(|(calendar, event)| (
                calendar.year(2026).map(crate::domain::Year::vacation_quota),
                event
            )),
            Some((
                Ok(22),
                Event::QuotaSet {
                    year: 2026,
                    days: 22
                }
            ))
        );
    }
}
