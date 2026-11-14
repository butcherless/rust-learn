//! La próxima ausencia, debajo de la cuadrícula de vacaciones de las vistas de mes y de año
//! (RN-30).

use chrono::NaiveDate;

use super::style::styled;
use super::*;
use crate::domain::{Absence, DateRange, MonthExt};

/// El título de la sección.
pub const ABSENCE_TITLE: &str = "Próxima ausencia";
/// El texto cuando no quedan ausencias en el calendario.
pub const NO_ABSENCE: &str = "No hay más ausencias";

/// Las líneas de la sección: el título y una por cada parte de la ausencia, con la barra de su
/// tipo, su etiqueta, alineada con la más larga, y sus fechas; o el aviso de que no hay más.
pub(super) fn next_absence(absence: Option<&Absence>, marker: Marker) -> Vec<String> {
    let title = styled(Style::new().bold(), ABSENCE_TITLE);
    let parts: Vec<String> = match absence {
        None => vec![NO_ABSENCE.to_owned()],
        Some(absence) => {
            let width = absence
                .parts
                .iter()
                .map(|part| part.kind.label().chars().count())
                .max()
                .unwrap_or(0);
            absence
                .parts
                .iter()
                .map(|part| {
                    format!(
                        "{} {:<width$} {}",
                        marker.symbol(part.kind),
                        part.kind,
                        dates(part.range)
                    )
                })
                .collect()
        }
    };
    iter::once(title).chain(parts).collect()
}

/// `8 diciembre`.
fn day_of_month(date: NaiveDate) -> String {
    format!(
        "{} {}",
        date.day(),
        month_name(Month::of(date)).to_lowercase()
    )
}

/// `8 diciembre`, `7-8 diciembre` o `28 diciembre-6 enero`.
fn dates(range: DateRange) -> String {
    let (from, to) = (range.from(), range.to());
    if from == to {
        day_of_month(from)
    } else if Month::of(from) == Month::of(to) && from.year() == to.year() {
        format!("{}-{}", from.day(), day_of_month(to))
    } else {
        format!("{}-{}", day_of_month(from), day_of_month(to))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::AbsencePart;
    use crate::domain::fixtures::{date, range};

    fn absence(parts: &[(DayKind, NaiveDate, NaiveDate)]) -> Absence {
        Absence {
            parts: parts
                .iter()
                .map(|&(kind, from, to)| AbsencePart {
                    kind,
                    range: range(from, to),
                })
                .collect(),
        }
    }

    fn plain(lines: &[String]) -> Vec<String> {
        lines
            .iter()
            .map(|line| anstream::adapter::strip_str(line).to_string())
            .collect()
    }

    #[test]
    fn rn_30_texts_are_fixed() {
        assert_eq!(ABSENCE_TITLE, "Próxima ausencia");
        assert_eq!(NO_ABSENCE, "No hay más ausencias");
    }

    #[test]
    fn rn_30_one_line_per_part_with_its_kind_and_dates() {
        let (holiday, vacation) = (DayKind::Holiday, DayKind::Vacation);
        [
            (
                absence(&[(holiday, date(2026, 12, 8), date(2026, 12, 8))]),
                vec!["F Festivo 8 diciembre"],
            ),
            (
                absence(&[(holiday, date(2026, 12, 7), date(2026, 12, 8))]),
                vec!["F Festivo 7-8 diciembre"],
            ),
            (
                absence(&[
                    (vacation, date(2026, 12, 4), date(2026, 12, 4)),
                    (holiday, date(2026, 12, 7), date(2026, 12, 8)),
                ]),
                vec!["V Vacaciones 4 diciembre", "F Festivo    7-8 diciembre"],
            ),
            (
                absence(&[(vacation, date(2026, 12, 28), date(2027, 1, 6))]),
                vec!["V Vacaciones 28 diciembre-6 enero"],
            ),
        ]
        .into_iter()
        .for_each(|(absence, expected)| {
            let lines = plain(&next_absence(Some(&absence), Marker::Letter));
            assert_eq!(lines.first().map(String::as_str), Some(ABSENCE_TITLE));
            assert_eq!(lines[1..], expected, "{absence:?}");
        });
        assert_eq!(
            plain(&next_absence(None, Marker::Letter)),
            [ABSENCE_TITLE, NO_ABSENCE]
        );
    }

    #[test]
    fn rn_30_with_color_the_title_is_bold_and_each_part_has_its_bar() {
        let lines = next_absence(
            Some(&absence(&[(
                DayKind::Holiday,
                date(2026, 12, 8),
                date(2026, 12, 8),
            )])),
            Marker::Bar,
        );
        assert_eq!(lines[0], "\u{1b}[1mPróxima ausencia\u{1b}[0m");
        assert_eq!(
            lines[1],
            format!(
                "{} Festivo 8 diciembre",
                Marker::Bar.symbol(DayKind::Holiday)
            )
        );
        assert!(
            lines[1].contains('▌') && lines[1].contains('\u{1b}'),
            "{lines:?}"
        );
    }
}
