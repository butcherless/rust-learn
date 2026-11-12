//! Las vistas de texto (`summary`, `day` y `config`) y los mensajes de cada `Event`.

use std::iter;

use super::*;

fn origin(day: &ResolvedDay) -> String {
    match day.origin {
        Origin::Mark(id) => format!("marca #{id}"),
        Origin::Weekend => "fin de semana".to_owned(),
        Origin::ReducedWeekday => format!(
            "jornada reducida de los {}",
            weekday_name(day.date.weekday())
        ),
        Origin::Default => "por defecto".to_owned(),
        Origin::DefaultVacation => "vacaciones por defecto desde el 1 de julio".to_owned(),
    }
}

/// La tabla de horas por mes del resumen, con su fila de total (RF-14).
fn month_hours_table(summary: &Summary) -> Vec<String> {
    let row = |name: &str, working_days: usize, hours: f64, cumulative: &str| {
        format!(
            "  {name:<13}{working_days:>6}{:>10}{cumulative:>11}",
            decimal(hours)
        )
    };
    iter::once(format!(
        "{:<15}{:>6}{:>10}{:>11}",
        "Horas por mes", "Días", "Horas", "Acumulado"
    ))
    .chain(summary.months.iter().map(|total| {
        row(
            month_name(total.month),
            total.working_days,
            total.working_hours,
            &decimal(total.cumulative_hours),
        )
    }))
    .chain(iter::once(row(
        "Total",
        summary.working_days,
        summary.working_hours,
        "",
    )))
    .collect()
}

pub(super) fn summary(summary: &Summary, marker: Marker) -> String {
    let counts = summary.counts.iter().map(|(kind, count)| {
        format!(
            "  {} {:<17}{:>4}",
            marker.symbol(*kind),
            kind.label(),
            count
        )
    });
    lines(
        iter::once(format!("Resumen {}", summary.year))
            .chain(counts)
            .chain([
                format!("Días laborables: {}", summary.working_days),
                format!("Horas laborables: {} h", decimal(summary.working_hours)),
                format!(
                    "Vacaciones: {} usadas de {}",
                    summary.vacation_used, summary.vacation_quota
                ),
                vacation_line(summary),
                String::new(),
            ])
            .chain(month_hours_table(summary)),
    )
}

/// Los días de la semana en español: `lunes`, `lunes y viernes` o `lunes, martes y viernes`.
fn weekday_list(days: &[Weekday]) -> String {
    let names: Vec<&str> = days.iter().map(|day| weekday_name(*day)).collect();
    match names.split_last() {
        None => "ninguno".to_owned(),
        Some((last, [])) => (*last).to_owned(),
        Some((last, rest)) => format!("{} y {last}", rest.join(", ")),
    }
}

/// La configuración (RF-15): los días de la semana y una fila por año con su cupo y sus horas.
/// Las columnas de horas llevan el nombre de la jornada en `hours`.
pub(super) fn config(calendar: &Calendar) -> String {
    let row = |year: &str, quota: &str, hours: &[String]| {
        let columns: String = Shift::ALL
            .iter()
            .zip(hours)
            .map(|(shift, hours)| format!("  {hours:>width$}", width = shift.name().len()))
            .collect();
        format!("{year:<4}  {quota:>4}{columns}")
    };
    let header = row(
        "Año",
        "Cupo",
        &Shift::ALL.map(|shift| shift.name().to_owned()),
    );
    let years: Vec<String> = calendar
        .years()
        .iter()
        .map(|year| {
            row(
                &year.number().to_string(),
                &year.vacation_quota().to_string(),
                &Shift::ALL.map(|shift| format!("{} h", decimal(year.hours().of(shift)))),
            )
        })
        .collect();
    let table = if years.is_empty() {
        vec!["No hay años; añade uno con «calendario add-year <año>».".to_owned()]
    } else {
        iter::once(header).chain(years).collect()
    };
    lines(
        [
            format!("Fin de semana: {}", weekday_list(calendar.weekend())),
            format!(
                "Jornada reducida: {}",
                weekday_list(calendar.reduced_days())
            ),
            String::new(),
        ]
        .into_iter()
        .chain(table),
    )
}

pub(super) fn day(day: &ResolvedDay, marker: Marker) -> String {
    let parts = [
        format!("{} ({})", day.date, weekday_name(day.date.weekday())),
        format!("{} {}", marker.symbol(day.kind), day.kind.label()),
        format!("{} h", decimal(day.hours)),
        origin(day),
    ];
    lines(iter::once(described(
        parts.join(" · "),
        " · ",
        day.description.as_deref(),
    )))
}

/// La confirmación de un cambio y su aviso, si lo hay.
pub fn event(event: &Event) -> Message {
    let text = match event {
        Event::YearAdded {
            year,
            quota,
            holidays,
        } => {
            let added = format!(
                "Año {year} añadido con un cupo de {} de vacaciones",
                days(*quota as usize)
            );
            match holidays {
                Some((from, copied)) => format!(
                    "{added} y {} de {from}.",
                    count(*copied, "festivo copiado", "festivos copiados")
                ),
                None => format!("{added}."),
            }
        }
        Event::Marked { mark, .. } => {
            format!("Marca #{} añadida: {} {}.", mark.id, mark.kind, mark.range)
        }
        Event::YearRemoved { year, marks: 0 } => format!("Año {year} eliminado."),
        Event::YearRemoved { year, marks } => {
            format!(
                "Año {year} eliminado, con {}.",
                count(*marks, "marca", "marcas")
            )
        }
        Event::Edited { mark, .. } => {
            format!(
                "Marca #{} modificada: {} {}.",
                mark.id, mark.kind, mark.range
            )
        }
        Event::AlreadyMarked(id) => {
            format!("Ya existe una marca idéntica (#{id}); no se ha añadido.")
        }
        Event::Unmarked(marks) => marks
            .iter()
            .map(|mark| {
                format!(
                    "Marca #{} eliminada: {} {}.",
                    mark.id, mark.kind, mark.range
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Event::HoursSet { year, shift, hours } => {
            format!("Jornada {shift} de {year}: {} h.", decimal(*hours))
        }
        Event::ReducedDaysSet(days) => format!("Jornada reducida: {}.", weekday_list(days)),
        Event::WeekendSet(days) => format!("Fin de semana: {}.", weekday_list(days)),
        Event::QuotaSet {
            year, days: quota, ..
        } => {
            format!("Cupo de vacaciones de {year}: {}.", days(*quota as usize))
        }
    };
    let warning = match event {
        Event::Marked { mark, excess } | Event::Edited { mark, excess } if *excess > 0 => {
            Some(format!(
                "aviso: las vacaciones de {} superan el cupo en {}",
                mark.range.from().year(),
                days(*excess)
            ))
        }
        Event::QuotaSet { year, excess, .. } if *excess > 0 => Some(format!(
            "aviso: las vacaciones de {year} superan el cupo en {}",
            days(*excess)
        )),
        Event::YearAdded {
            holidays: Some(_), ..
        } => Some(
            "aviso: revisa los festivos que cambian de fecha cada año, como Semana Santa o los trasladados a lunes"
                .to_owned(),
        ),
        Event::Marked { .. }
        | Event::Edited { .. }
        | Event::YearRemoved { .. }
        | Event::YearAdded { .. }
        | Event::AlreadyMarked(_)
        | Event::Unmarked(_)
        | Event::HoursSet { .. }
        | Event::ReducedDaysSet(_)
        | Event::WeekendSet(_)
        | Event::QuotaSet { .. } => None,
    };
    Message { text, warning }
}
