//! La cuadrícula de vacaciones, a la derecha del calendario en las vistas de mes y de año
//! (RN-27): un cuadro por día del cupo, cinco por fila, primero los usados.

use std::iter;

use super::style::{color, styled};
use super::*;

/// Días por fila: con el cupo por defecto, 25 días, la cuadrícula es de 5×5.
const DAYS_PER_ROW: usize = 5;
/// Cuadros como mucho, los días de un año bisiesto, para que un cupo enorme no llene la vista.
const MAX_DAYS: usize = 366;
/// Separación entre dos cuadros.
const DAY_GAP: &str = " ";
/// Ancho visible de un cuadro: dos columnas, para que se vea cuadrado.
const DAY_WIDTH: usize = 2;
/// Separación entre la cuadrícula y su leyenda.
const LEGEND_GAP: &str = "   ";
/// El título de la leyenda.
pub const GRID_TITLE: &str = "Vacaciones";

/// El estado de un día del cupo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Share {
    /// Usado, ya disfrutado o reservado con una marca: en el naranja de `Vacation`.
    Used,
    /// Restante, aún libre: en verde.
    Remaining,
}

impl Share {
    /// Un cuadro: un bloque de su color o, sin color, un carácter distinto en cada estado.
    fn square(self, marker: Marker) -> String {
        match (marker, self) {
            (Marker::Bar, Self::Used) => styled(fg(color(DayKind::Vacation)), "██"),
            (Marker::Bar, Self::Remaining) => styled(fg(AnsiColor::Green.into()), "██"),
            (Marker::Letter, Self::Used) => "██".to_owned(),
            (Marker::Letter, Self::Remaining) => "░░".to_owned(),
        }
    }
}

fn fg(color: Color) -> Style {
    Style::new().fg_color(Some(color))
}

/// Las líneas de la cuadrícula de las vacaciones de `summary`, con su leyenda a la derecha
/// (RN-27). Hay un cuadro por día del cupo, o por día usado si pasan del cupo. Usados son todos
/// los días de vacaciones del año, también los que aún no han llegado: ya no están libres.
pub(super) fn vacation_grid(summary: &Summary, marker: Marker) -> Vec<String> {
    let used = summary.vacation_used;
    let quota = usize::try_from(summary.vacation_quota).unwrap_or(MAX_DAYS);
    let days = quota.max(used).min(MAX_DAYS);
    let squares: Vec<String> = (0..days)
        .map(|day| {
            if day < used {
                Share::Used
            } else {
                Share::Remaining
            }
        })
        .map(|share| share.square(marker))
        .collect();
    let blank = " ".repeat(DAY_WIDTH);
    let rows: Vec<String> = squares
        .chunks(DAYS_PER_ROW)
        .map(|row| {
            row.iter()
                .cloned()
                .chain(iter::repeat_n(blank.clone(), DAYS_PER_ROW - row.len()))
                .collect::<Vec<_>>()
                .join(DAY_GAP)
        })
        .collect();
    let legend = [
        styled(Style::new().bold(), GRID_TITLE),
        format!("{} Usadas {:>6}", Share::Used.square(marker), used),
        format!(
            "{} Restantes {:>3}",
            Share::Remaining.square(marker),
            summary.vacation_remaining()
        ),
    ];
    let height = rows.len().max(legend.len());
    let width = DAYS_PER_ROW * DAY_WIDTH + (DAYS_PER_ROW - 1) * DAY_GAP.len();
    // La leyenda, centrada en vertical junto a la cuadrícula.
    let legend_lines = iter::repeat_n(None, (height - legend.len()) / 2)
        .chain(legend.into_iter().map(Some))
        .chain(iter::repeat(None));
    rows.into_iter()
        .chain(iter::repeat(" ".repeat(width)))
        .take(height)
        .zip(legend_lines)
        .map(|(row, text)| match text {
            Some(text) => format!("{row}{LEGEND_GAP}{text}"),
            None => row,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un resumen con `used` días de vacaciones y un cupo de `quota`; el resto no cuenta aquí.
    fn summary(used: usize, quota: u32) -> Summary {
        Summary {
            year: 2026,
            counts: DayKind::ALL.map(|kind| (kind, 0)),
            working_days: 0,
            working_hours: 0.0,
            vacation_used: used,
            vacation_quota: quota,
            months: Vec::new(),
        }
    }

    fn plain(lines: &[String]) -> String {
        lines
            .iter()
            .map(|line| {
                format!(
                    "{}\n",
                    anstream::adapter::strip_str(line).to_string().trim_end()
                )
            })
            .collect()
    }

    #[test]
    fn rn_27_the_title_is_fixed() {
        assert_eq!(GRID_TITLE, "Vacaciones");
    }

    #[test]
    fn rn_27_without_color_used_days_come_first_five_per_row() {
        assert_eq!(
            plain(&vacation_grid(&summary(12, 25), Marker::Letter)),
            [
                "██ ██ ██ ██ ██",
                "██ ██ ██ ██ ██   Vacaciones",
                "██ ██ ░░ ░░ ░░   ██ Usadas     12",
                "░░ ░░ ░░ ░░ ░░   ░░ Restantes  13",
                "░░ ░░ ░░ ░░ ░░",
                "",
            ]
            .join("\n")
        );
    }

    #[test]
    fn rn_27_one_square_per_day_of_the_quota_or_of_the_used_days_above_it() {
        [
            ("cupo de 25", summary(0, 25), 5, 25, 0),
            ("cupo de 23", summary(3, 23), 5, 20, 3),
            ("por encima del cupo", summary(27, 25), 6, 0, 27),
            ("sin cupo", summary(0, 0), 3, 0, 0),
            ("cupo enorme", summary(0, u32::MAX), 74, 366, 0),
        ]
        .into_iter()
        .for_each(|(case, summary, height, remaining, used)| {
            let text = plain(&vacation_grid(&summary, Marker::Letter));
            assert_eq!(text.lines().count(), height, "{case}\n{text}");
            assert_eq!(text.matches("░░").count() - 1, remaining, "{case}\n{text}");
            assert_eq!(text.matches("██").count() - 1, used, "{case}\n{text}");
        });
    }

    #[test]
    fn rn_27_the_legend_stays_beside_a_short_grid() {
        assert_eq!(
            plain(&vacation_grid(&summary(1, 3), Marker::Letter)),
            [
                "██ ░░ ░░         Vacaciones",
                "                 ██ Usadas      1",
                "                 ░░ Restantes   2",
                "",
            ]
            .join("\n")
        );
    }

    #[test]
    fn rn_27_with_color_used_days_are_orange_and_remaining_ones_green() {
        let text = vacation_grid(&summary(10, 25), Marker::Bar).concat();
        [
            ("\u{1b}[38;5;208m██\u{1b}[0m", "usados: naranja"),
            ("\u{1b}[32m██\u{1b}[0m", "restantes: verde"),
            ("\u{1b}[1mVacaciones\u{1b}[0m", "título en negrita"),
        ]
        .into_iter()
        .for_each(|(code, case)| assert!(text.contains(code), "{case}\n{text}"));
        assert!(!text.contains('░'), "{text}");
    }
}
