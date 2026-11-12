//! Casos de uso de la configuración: fin de semana, jornada reducida, horas y cupo (RF-09, RF-10,
//! RF-16, RF-17).

use super::*;

/// Los días del fin de semana (RF-17).
pub(super) fn weekend(calendar: Calendar, days: &[Weekday]) -> Outcome {
    let calendar = calendar.with_weekend(days);
    Outcome::Changed {
        event: Event::WeekendSet(calendar.weekend().to_vec()),
        calendar,
    }
}

/// Los días de jornada reducida (RF-16).
pub(super) fn reduced_days(calendar: Calendar, days: &[Weekday]) -> Outcome {
    let calendar = calendar.with_reduced_days(days);
    Outcome::Changed {
        event: Event::ReducedDaysSet(calendar.reduced_days().to_vec()),
        calendar,
    }
}

/// Las horas de una jornada en un año (RF-09, RN-25).
pub(super) fn hours(
    calendar: Calendar,
    year: i32,
    shift: Shift,
    hours: f64,
) -> Result<Outcome, DomainError> {
    Ok(Outcome::Changed {
        calendar: calendar.with_shift_hours(year, shift, hours)?,
        event: Event::HoursSet { year, shift, hours },
    })
}

/// El cupo de un año, con el aviso si queda por debajo de las vacaciones usadas (RF-10, RN-11).
pub(super) fn quota(calendar: Calendar, year: i32, days: u32) -> Result<Outcome, DomainError> {
    let calendar = calendar.with_quota(year, days)?;
    let excess = summarize(&calendar, year)?.vacation_excess();
    Ok(Outcome::Changed {
        calendar,
        event: Event::QuotaSet { year, days, excess },
    })
}
