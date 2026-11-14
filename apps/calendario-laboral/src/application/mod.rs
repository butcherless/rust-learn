//! Casos de uso: una función pura de un `Calendar` y un [`Command`] a un [`Outcome`].
//!
//! Quien la llama carga el calendario, ejecuta [`execute`], guarda el calendario nuevo si lo hay
//! y muestra el resultado. Aquí no hay E/S.

mod marks;
mod settings;
mod views;
mod years;

use chrono::{Datelike, NaiveDate, Weekday};

use crate::domain::{
    Absence, Calendar, DEFAULT_VACATION_QUOTA, DateRange, DomainError, Mark, MarkId, MarkKind,
    Marking, Month, MonthExt, ResolvedDay, Shift, Summary, Year, next_absence, resolve_day,
    resolve_year, summarize, summary_of,
};

/// Una operación pedida por el usuario.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// RF-01: añadir un año; RF-19: con los festivos de `holidays_from`.
    AddYear {
        year: i32,
        holidays_from: Option<i32>,
    },
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
    /// RF-20: eliminar un año; con `Mark`, solo si está confirmado.
    RemoveYear { year: i32, confirmed: bool },
    /// RF-18: modificar un `Mark`; lo que es `None` se mantiene.
    Edit {
        id: MarkId,
        kind: Option<MarkKind>,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
        /// Una descripción en blanco quita la que tenga.
        description: Option<String>,
    },
    /// RF-03: eliminar uno o varios `Mark`, todos o ninguno.
    Unmark(Vec<MarkId>),
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
        /// La fecha actual, desde la que se busca la próxima ausencia (RN-30).
        today: NaiveDate,
    },
    /// RF-07: mostrar un año completo.
    Year {
        /// El año.
        year: i32,
        /// La fecha actual, desde la que se busca la próxima ausencia (RN-30).
        today: NaiveDate,
    },
    /// RF-08: resumir un año.
    Summary(i32),
    /// RF-15: consultar la configuración.
    Config,
    /// RF-16: configurar los días de la regla `ReducedWeekday`; vacío, ninguno.
    ReducedDays(Vec<Weekday>),
    /// RF-17: configurar los días del `Weekend`; vacío, ninguno.
    Weekend(Vec<Weekday>),
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
        /// RF-19: el año del que se han copiado los festivos y cuántos.
        holidays: Option<(i32, usize)>,
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
    /// Se ha eliminado un año con sus `Mark`.
    YearRemoved { year: i32, marks: usize },
    /// Se ha modificado un `Mark`; `excess` lleva el aviso de RN-11.
    Edited { mark: Mark, excess: usize },
    /// Se han eliminado uno o varios `Mark`, en el orden pedido.
    Unmarked(Vec<Mark>),
    /// Se han configurado los días de jornada reducida, de lunes a domingo.
    ReducedDaysSet(Vec<Weekday>),
    /// Se han configurado los días del fin de semana, de lunes a domingo.
    WeekendSet(Vec<Weekday>),
    /// Se han configurado las horas de un `Shift` en un `Year`.
    HoursSet { year: i32, shift: Shift, hours: f64 },
    /// Se ha configurado el cupo de un año.
    QuotaSet {
        /// El año.
        year: i32,
        /// Días de vacaciones.
        days: u32,
        /// Días de vacaciones usadas por encima del nuevo cupo (RN-11).
        excess: usize,
    },
}

/// El resultado de una consulta.
#[derive(Debug, Clone, PartialEq)]
pub enum Report {
    /// `Mark` ordenados por su primer día.
    Marks(Vec<Mark>),
    /// Un día.
    Day(ResolvedDay),
    /// Un mes, el resumen del año, para las vacaciones restantes, y la próxima ausencia.
    Month(MonthView, Summary, Option<Absence>),
    /// Todos los meses de un año, su resumen y la próxima ausencia.
    Year(Vec<MonthView>, Summary, Option<Absence>),
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
        Command::AddYear {
            year,
            holidays_from,
        } => years::add_year(calendar, year, holidays_from),
        Command::RemoveYear { year, confirmed } => years::remove_year(calendar, year, confirmed),
        Command::Mark {
            kind,
            from,
            to,
            description,
        } => marks::mark(
            calendar,
            kind,
            DateRange::new(from, to.unwrap_or(from))?,
            description,
        ),
        Command::Edit {
            id,
            kind,
            from,
            to,
            description,
        } => marks::edit(calendar, id, kind, (from, to), description),
        Command::Unmark(ids) => marks::unmark(calendar, &ids),
        Command::List(year) => {
            marks::list(&calendar, year).map(|marks| Outcome::Report(Report::Marks(marks)))
        }
        Command::Day(date) => {
            resolve_day(&calendar, date).map(|day| Outcome::Report(Report::Day(day)))
        }
        Command::Month { year, month, today } => views::month(&calendar, year, month, today),
        Command::Year { year, today } => views::year(&calendar, year, today),
        Command::Summary(year) => {
            summarize(&calendar, year).map(|summary| Outcome::Report(Report::Summary(summary)))
        }
        Command::Config => Ok(Outcome::Report(Report::Config(calendar))),
        Command::Weekend(days) => Ok(settings::weekend(calendar, &days)),
        Command::ReducedDays(days) => Ok(settings::reduced_days(calendar, &days)),
        Command::Hours { year, shift, hours } => settings::hours(calendar, year, shift, hours),
        Command::Quota { year, days } => settings::quota(calendar, year, days),
    }
}

fn sorted(marks: impl Iterator<Item = Mark>) -> Vec<Mark> {
    let mut marks: Vec<Mark> = marks.collect();
    marks.sort_by_key(|mark| (mark.range.from(), mark.id));
    marks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::{calendar, date, marked, range, today};
    use crate::domain::{AbsencePart, DayKind, Origin, parse_date};

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
    fn rf_19_add_year_copies_the_holidays_of_another_year() {
        let calendar = Calendar::default()
            .with_year(2024)
            .and_then(|calendar| {
                [
                    (
                        MarkKind::Holiday,
                        "2024-01-01",
                        "2024-01-01",
                        Some("Año nuevo"),
                    ),
                    (MarkKind::Holiday, "2024-02-29", "2024-02-29", None),
                    (
                        MarkKind::Holiday,
                        "2024-12-24",
                        "2024-12-26",
                        Some("Navidad"),
                    ),
                    (MarkKind::Vacation, "2024-08-05", "2024-08-09", None),
                    (MarkKind::Reduced, "2024-12-31", "2024-12-31", None),
                ]
                .into_iter()
                .try_fold(calendar, |calendar, (kind, from, to, description)| {
                    let range = DateRange::new(parse_date(from)?, parse_date(to)?)?;
                    calendar
                        .with_mark(kind, range, description.map(str::to_owned))
                        .map(|(calendar, _)| calendar)
                })
            })
            .expect("2024");
        let outcome = changed(execute(
            calendar,
            Command::AddYear {
                year: 2025,
                holidays_from: Some(2024),
            },
        ));
        let copied = outcome.as_ref().map(|(calendar, _)| {
            calendar
                .year(2025)
                .map(|year| {
                    year.marks()
                        .iter()
                        .map(|mark| {
                            (
                                mark.id.0,
                                mark.kind,
                                mark.range.to_string(),
                                mark.description.clone(),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        });
        assert_eq!(
            copied,
            Some(vec![
                (
                    6,
                    MarkKind::Holiday,
                    "2025-01-01".to_owned(),
                    Some("Año nuevo".to_owned())
                ),
                (
                    7,
                    MarkKind::Holiday,
                    "2025-12-24 → 2025-12-26".to_owned(),
                    Some("Navidad".to_owned())
                ),
            ]),
            "solo los festivos, sin el 29 de febrero, con ids nuevos"
        );
        assert_eq!(
            outcome.map(|(_, event)| event),
            Some(Event::YearAdded {
                year: 2025,
                quota: 25,
                holidays: Some((2024, 2)),
            })
        );
    }

    #[test]
    fn rf_20_rn_13_remove_year_needs_confirmation_when_it_has_marks() {
        let with_marks = marked(&[(MarkKind::Holiday, "2026-03-03", "2026-03-03")]);
        let remove = |calendar: &Calendar, confirmed| {
            execute(
                calendar.clone(),
                Command::RemoveYear {
                    year: 2026,
                    confirmed,
                },
            )
        };
        assert_eq!(
            remove(&with_marks, false),
            Err(DomainError::YearHasMarks {
                year: 2026,
                marks: 1
            })
        );
        assert_eq!(
            changed(remove(&with_marks, true))
                .map(|(calendar, event)| (calendar.years().len(), event)),
            Some((
                0,
                Event::YearRemoved {
                    year: 2026,
                    marks: 1
                }
            ))
        );
        assert_eq!(
            changed(remove(&calendar(), false)).map(|(_, event)| event),
            Some(Event::YearRemoved {
                year: 2026,
                marks: 0
            }),
            "sin marcas no hace falta confirmar"
        );
        assert_eq!(
            DomainError::YearHasMarks {
                year: 2026,
                marks: 2
            }
            .to_string(),
            "el año 2026 tiene 2 marcas; repite el comando con --confirmar para eliminarlo con ellas"
        );
    }

    #[test]
    fn rf_01_add_year_changes_the_calendar() {
        let outcome = changed(execute(
            Calendar::default(),
            Command::AddYear {
                year: 2027,
                holidays_from: None,
            },
        ));
        assert_eq!(
            outcome.map(|(calendar, event)| (calendar.years().len(), event)),
            Some((
                1,
                Event::YearAdded {
                    year: 2027,
                    quota: 25,
                    holidays: None,
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

    fn edit_command(id: u32) -> Command {
        Command::Edit {
            id: MarkId(id),
            kind: None,
            from: None,
            to: None,
            description: None,
        }
    }

    #[test]
    fn rf_18_edit_changes_only_what_is_given() {
        let calendar = calendar()
            .with_mark(
                MarkKind::Vacation,
                range(date(2026, 6, 1), date(2026, 6, 5)),
                Some("Viaje".to_owned()),
            )
            .map(|(calendar, _)| calendar)
            .expect("mark");
        let edited = |command| {
            changed(execute(calendar.clone(), command)).and_then(|(_, event)| {
                if let Event::Edited { mark, excess } = event {
                    Some((mark, excess))
                } else {
                    None
                }
            })
        };
        let original = Mark {
            id: MarkId(1),
            kind: MarkKind::Vacation,
            range: range(date(2026, 6, 1), date(2026, 6, 5)),
            description: Some("Viaje".to_owned()),
        };
        [
            (
                Command::Edit {
                    id: MarkId(1),
                    kind: None,
                    from: None,
                    to: Some(date(2026, 6, 12)),
                    description: None,
                },
                Mark {
                    range: range(date(2026, 6, 1), date(2026, 6, 12)),
                    ..original.clone()
                },
                "solo el último día",
            ),
            (
                Command::Edit {
                    id: MarkId(1),
                    kind: Some(MarkKind::Holiday),
                    from: Some(date(2026, 6, 3)),
                    to: None,
                    description: None,
                },
                Mark {
                    kind: MarkKind::Holiday,
                    range: range(date(2026, 6, 3), date(2026, 6, 5)),
                    ..original.clone()
                },
                "tipo y primer día",
            ),
            (
                Command::Edit {
                    id: MarkId(1),
                    kind: None,
                    from: None,
                    to: None,
                    description: Some("  ".to_owned()),
                },
                Mark {
                    description: None,
                    ..original.clone()
                },
                "una descripción en blanco la quita",
            ),
            (
                Command::Edit {
                    id: MarkId(1),
                    kind: None,
                    from: None,
                    to: None,
                    description: Some(" Playa ".to_owned()),
                },
                Mark {
                    description: Some("Playa".to_owned()),
                    ..original.clone()
                },
                "descripción nueva, sin espacios",
            ),
        ]
        .into_iter()
        .for_each(|(command, expected, case)| {
            assert_eq!(edited(command), Some((expected, 0)), "{case}");
        });
        // RN-11: 22 días laborables en junio y 5 hasta el 7 de julio, 2 por encima de 25.
        assert_eq!(
            edited(Command::Edit {
                id: MarkId(1),
                kind: None,
                from: None,
                to: Some(date(2026, 7, 7)),
                description: None,
            })
            .map(|(_, excess)| excess),
            Some(2)
        );
        [
            (
                Command::Edit {
                    id: MarkId(1),
                    kind: None,
                    from: Some(date(2026, 6, 9)),
                    to: None,
                    description: None,
                },
                DomainError::ReversedRange {
                    from: date(2026, 6, 9),
                    to: date(2026, 6, 5),
                },
            ),
            (edit_command(7), DomainError::UnknownMark(MarkId(7))),
        ]
        .into_iter()
        .for_each(|(command, error)| {
            assert_eq!(execute(calendar.clone(), command), Err(error));
        });
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
            (
                Command::AddYear {
                    year: 2026,
                    holidays_from: None,
                },
                DomainError::DuplicateYear(2026),
            ),
            (
                Command::AddYear {
                    year: 2027,
                    holidays_from: Some(2025),
                },
                DomainError::UnknownYear(2025),
            ),
            (
                Command::Unmark(vec![MarkId(3)]),
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
                    today: today(),
                },
                DomainError::UnknownYear(2027),
            ),
            (
                Command::Year {
                    year: 2027,
                    today: today(),
                },
                DomainError::UnknownYear(2027),
            ),
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
        let outcome = changed(execute(calendar, Command::Unmark(vec![MarkId(1)])));
        assert_eq!(
            outcome.map(|(calendar, event)| (calendar.marks().count(), event)),
            Some((
                0,
                Event::Unmarked(vec![Mark {
                    id: MarkId(1),
                    kind: MarkKind::Holiday,
                    range: crate::domain::DateRange::day(date(2026, 3, 3)),
                    description: None,
                }])
            ))
        );
    }

    #[test]
    fn rf_03_rn_13_unmark_removes_several_marks_or_none() {
        let calendar = marked(&[
            (MarkKind::Holiday, "2026-03-03", "2026-03-03"),
            (MarkKind::Holiday, "2026-03-04", "2026-03-04"),
            (MarkKind::Holiday, "2026-03-05", "2026-03-05"),
        ]);
        let removed = |ids: &[u32]| {
            changed(execute(
                calendar.clone(),
                Command::Unmark(ids.iter().map(|id| MarkId(*id)).collect()),
            ))
            .map(|(calendar, event)| {
                let removed: Vec<u32> = if let Event::Unmarked(marks) = event {
                    marks.iter().map(|mark| mark.id.0).collect()
                } else {
                    Vec::new()
                };
                let left: Vec<u32> = calendar.marks().map(|mark| mark.id.0).collect();
                (removed, left)
            })
        };
        assert_eq!(removed(&[3, 1]), Some((vec![3, 1], vec![2])));
        assert_eq!(removed(&[2, 2]), Some((vec![2], vec![1, 3])), "repetido");
        assert_eq!(
            execute(
                calendar.clone(),
                Command::Unmark(vec![MarkId(1), MarkId(9)])
            ),
            Err(DomainError::UnknownMark(MarkId(9))),
            "todo o nada"
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
                today: today(),
            },
        );
        let summary = match month {
            Some(Report::Month(view, summary, _)) => {
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
    fn rn_30_month_and_year_carry_the_next_absence_from_today() {
        [
            Command::Month {
                year: 2026,
                month: Month::December,
                today: today(),
            },
            Command::Year {
                year: 2026,
                today: today(),
            },
        ]
        .into_iter()
        .for_each(|command| {
            let absence = match report(calendar(), command.clone()) {
                Some(Report::Month(_, _, absence) | Report::Year(_, _, absence)) => absence,
                Some(
                    Report::Marks(_) | Report::Day(_) | Report::Summary(_) | Report::Config(_),
                )
                | None => None,
            };
            // Las vacaciones por defecto de 2026 (RN-14), sea cual sea el mes que se muestra.
            assert_eq!(
                absence.map(|absence| absence.parts),
                Some(vec![AbsencePart {
                    kind: DayKind::Vacation,
                    range: range(date(2026, 7, 1), date(2026, 8, 4)),
                }]),
                "{command:?}"
            );
        });
    }

    #[test]
    fn rf_07_year_has_twelve_months() {
        let year = report(
            calendar(),
            Command::Year {
                year: 2026,
                today: today(),
            },
        );
        let months = match year {
            Some(Report::Year(months, _, _)) => Some(
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
    fn rf_17_rn_04_sets_the_weekend_in_week_order() {
        let set = changed(execute(
            calendar(),
            Command::Weekend(vec![Weekday::Sat, Weekday::Fri]),
        ));
        assert_eq!(
            set.as_ref()
                .map(|(calendar, event)| (calendar.weekend().to_vec(), event.clone())),
            Some((
                vec![Weekday::Fri, Weekday::Sat],
                Event::WeekendSet(vec![Weekday::Fri, Weekday::Sat])
            ))
        );
        // El domingo 2026-03-01 deja de ser fin de semana y el viernes 2026-03-06 pasa a serlo.
        let kind = |day| {
            set.as_ref()
                .and_then(|(calendar, _)| resolve_day(calendar, day).ok())
                .map(|day| day.kind)
        };
        assert_eq!(kind(date(2026, 3, 1)), Some(DayKind::Working));
        assert_eq!(kind(date(2026, 3, 6)), Some(DayKind::Weekend));
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
                    days: 22,
                    excess: 0,
                }
            ))
        );
    }

    #[test]
    fn rn_11_a_quota_below_the_used_vacation_is_saved_with_the_excess() {
        // 2026-06-01 a 2026-06-05 son 5 días laborables de vacaciones.
        let calendar = marked(&[(MarkKind::Vacation, "2026-06-01", "2026-06-05")]);
        let event = |days| {
            changed(execute(
                calendar.clone(),
                Command::Quota { year: 2026, days },
            ))
            .map(|(_, event)| event)
        };
        [(3, 2), (5, 0), (25, 0)]
            .into_iter()
            .for_each(|(days, excess)| {
                assert_eq!(
                    event(days),
                    Some(Event::QuotaSet {
                        year: 2026,
                        days,
                        excess
                    }),
                    "cupo {days}"
                );
            });
    }
}
