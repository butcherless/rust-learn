//! La próxima ausencia, debajo de la cuadrícula de vacaciones de las vistas de mes y de año
//! (RN-30).

use chrono::NaiveDate;

use super::style::styled;
use super::*;
use crate::domain::{Absence, MonthExt};

/// El título de la sección.
pub const ABSENCE_TITLE: &str = "Próxima ausencia";
/// El texto cuando no quedan ausencias en el calendario.
pub const NO_ABSENCE: &str = "No hay más ausencias";

/// Las líneas de la sección: el título y, si hay ausencia, sus tipos, sus fechas y sus días.
pub(super) fn next_absence(absence: Option<&Absence>, marker: Marker) -> Vec<String> {
    let title = styled(Style::new().bold(), ABSENCE_TITLE);
    match absence {
        None => vec![title, NO_ABSENCE.to_owned()],
        Some(absence) => vec![
            title,
            absence
                .kinds
                .iter()
                .map(|&kind| format!("{} {}", marker.symbol(kind), kind.label()))
                .collect::<Vec<_>>()
                .join(" · "),
            dates(absence.range.from(), absence.range.to()),
            format!(
                "{} · {}",
                count(absence.days, "día laborable", "días laborables"),
                when(absence.days_until)
            ),
        ],
    }
}

/// `8 de diciembre`.
fn day_of_month(date: NaiveDate) -> String {
    format!(
        "{} de {}",
        date.day(),
        month_name(Month::of(date)).to_lowercase()
    )
}

/// `el 8 de diciembre`, `del 7 al 8 de diciembre` o `del 28 de diciembre al 6 de enero`.
fn dates(from: NaiveDate, to: NaiveDate) -> String {
    if from == to {
        format!("el {}", day_of_month(from))
    } else if Month::of(from) == Month::of(to) && from.year() == to.year() {
        format!("del {} al {}", from.day(), day_of_month(to))
    } else {
        format!("del {} al {}", day_of_month(from), day_of_month(to))
    }
}

/// Cuándo empieza: `hoy`, `mañana` o `en 23 días`.
fn when(days_until: i64) -> String {
    match days_until {
        0 => "hoy".to_owned(),
        1 => "mañana".to_owned(),
        days_until => format!("en {}", days(days_until)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::{date, range};

    fn absence(from: NaiveDate, to: NaiveDate, kinds: &[DayKind], days_until: i64) -> Absence {
        Absence {
            range: range(from, to),
            kinds: kinds.to_vec(),
            days: 2,
            days_until,
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
    fn rn_30_the_section_shows_the_kinds_the_dates_and_when_it_starts() {
        let (holiday, vacation) = (DayKind::Holiday, DayKind::Vacation);
        [
            (
                absence(date(2026, 12, 8), date(2026, 12, 8), &[holiday], 0),
                ["F Festivo", "el 8 de diciembre", "2 días laborables · hoy"],
            ),
            (
                absence(date(2026, 12, 7), date(2026, 12, 8), &[holiday], 1),
                [
                    "F Festivo",
                    "del 7 al 8 de diciembre",
                    "2 días laborables · mañana",
                ],
            ),
            (
                absence(
                    date(2026, 12, 28),
                    date(2027, 1, 6),
                    &[vacation, holiday],
                    23,
                ),
                [
                    "V Vacaciones · F Festivo",
                    "del 28 de diciembre al 6 de enero",
                    "2 días laborables · en 23 días",
                ],
            ),
            (
                absence(date(2026, 5, 29), date(2026, 6, 1), &[vacation], 2),
                [
                    "V Vacaciones",
                    "del 29 de mayo al 1 de junio",
                    "2 días laborables · en 2 días",
                ],
            ),
        ]
        .into_iter()
        .for_each(|(absence, expected)| {
            let lines = plain(&next_absence(Some(&absence), Marker::Letter));
            assert_eq!(lines[0], ABSENCE_TITLE);
            assert_eq!(lines[1..], expected, "{absence:?}");
        });
        assert_eq!(
            plain(&next_absence(None, Marker::Letter)),
            [ABSENCE_TITLE, NO_ABSENCE]
        );
    }

    #[test]
    fn rn_30_one_day_absence_counts_in_the_singular_and_uses_the_bar_with_color() {
        let one = Absence {
            days: 1,
            ..absence(date(2026, 12, 8), date(2026, 12, 8), &[DayKind::Holiday], 3)
        };
        let lines = next_absence(Some(&one), Marker::Bar);
        assert_eq!(lines[0], "\u{1b}[1mPróxima ausencia\u{1b}[0m");
        assert!(
            lines[1].contains("▌") && lines[1].contains('\u{1b}'),
            "{lines:?}"
        );
        assert_eq!(plain(&lines)[3], "1 día laborable · en 3 días");
    }
}
