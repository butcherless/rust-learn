//! Las vistas de mes y de año, que resuelven el año una sola vez (RF-06, RF-07, RF-14), con la
//! próxima ausencia (RN-30).

use super::*;

/// La vista de un mes y el resumen de su año (RF-06, RF-14).
pub(super) fn month(
    calendar: &Calendar,
    year: i32,
    month: Month,
    today: NaiveDate,
) -> Result<Outcome, DomainError> {
    let (days, year) = (resolve_year(calendar, year)?, calendar.year(year)?);
    Ok(Outcome::Report(Report::Month(
        month_view(year, month, &days)?,
        summary_of(year, &days),
        next_absence(calendar, year.number(), today)?,
    )))
}

/// Los 12 meses y el resumen de un año, con el año resuelto una sola vez (RF-07, RF-14).
pub(super) fn year(
    calendar: &Calendar,
    year: i32,
    today: NaiveDate,
) -> Result<Outcome, DomainError> {
    let (days, year) = (resolve_year(calendar, year)?, calendar.year(year)?);
    Ok(Outcome::Report(Report::Year(
        Month::ALL
            .into_iter()
            .map(|month| month_view(year, month, &days))
            .collect::<Result<_, _>>()?,
        summary_of(year, &days),
        next_absence(calendar, year.number(), today)?,
    )))
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
