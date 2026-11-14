//! Adaptador de presentación: el texto de consola de cada `Outcome`, en español (§5, RNF-07).
//!
//! Cada día se muestra con su color y su letra, para que se lea igual sin color.
//! Las funciones devuelven texto con códigos ANSI; `anstream` los quita cuando la salida no es
//! un terminal o está definida `NO_COLOR`.

mod calendar;
mod markets;
mod next_absence;
mod reservoirs;
mod style;
mod text;
mod vacation_grid;

use std::fmt::Display;
use std::iter;

use anstyle::{Ansi256Color, AnsiColor, Color, Style};
use chrono::{Datelike, NaiveDate, Weekday};

use crate::adapters::panel::Panel;
use crate::application::{Event, MonthView, Report};
use crate::domain::{
    Calendar, DayKind, Mark, Month, MonthTotal, Origin, REFERENCE_DAY_HOURS, ResolvedDay, Shift,
    Summary,
};

use calendar::{month, year};
pub use markets::{MARKETS_SOURCE, MARKETS_TITLE};
pub use next_absence::{ABSENCE_TITLE, no_absence};
pub use reservoirs::{
    LIGHT_OFF, LIGHT_ON, PROCESSED_BY, RESERVOIRS_TITLE, UNAVAILABLE, source_line,
};
use style::paint;
pub use text::event;
use text::{config, day, summary};
pub use vacation_grid::GRID_TITLE;

/// Cómo se identifica el tipo de cada día junto a su número (RN-20).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    /// Una barra `▌` del color del tipo, separada del número: `15 ▌`. Solo con color.
    Bar,
    /// La letra del tipo en el mismo sitio: `15 F`. Para la salida sin color, donde la barra no
    /// distingue nada.
    Letter,
}

impl Marker {
    /// La barra si la salida tendrá color, y la letra si no.
    pub const fn for_color(color: bool) -> Self {
        if color { Self::Bar } else { Self::Letter }
    }

    /// El símbolo de `kind`, pintado de su color.
    fn symbol(self, kind: DayKind) -> String {
        match self {
            Self::Bar => paint(kind, "▌"),
            Self::Letter => paint(kind, &kind.letter().to_string()),
        }
    }
}

/// El texto de un cambio y un aviso que se muestra aparte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// La confirmación.
    pub text: String,
    /// Un aviso, como unas vacaciones por encima del cupo (RN-11).
    pub warning: Option<String>,
}

/// Horas con coma decimal y uno o dos decimales: `8,5`, `6,0`, `7,25` (RN-15).
pub fn decimal(hours: f64) -> String {
    let fixed = format!("{hours:.2}");
    let trimmed = fixed.trim_end_matches('0');
    let text = if trimmed.ends_with('.') {
        format!("{trimmed}0")
    } else {
        trimmed.to_owned()
    };
    text.replace('.', ",")
}

/// `n` con su palabra en singular solo si es 1: `1 día`, `0 días`, `25 días`. Vale para cualquier
/// entero sin signo (`usize` de los recuentos, `u32` del cupo) sin convertirlo con `as`.
fn count<N: Display + PartialEq + From<u8>>(n: N, singular: &str, plural: &str) -> String {
    let word = if n == N::from(1) { singular } else { plural };
    format!("{n} {word}")
}

fn days<N: Display + PartialEq + From<u8>>(n: N) -> String {
    count(n, "día", "días")
}

/// El nombre en español del mes.
pub const fn month_name(month: Month) -> &'static str {
    match month {
        Month::January => "Enero",
        Month::February => "Febrero",
        Month::March => "Marzo",
        Month::April => "Abril",
        Month::May => "Mayo",
        Month::June => "Junio",
        Month::July => "Julio",
        Month::August => "Agosto",
        Month::September => "Septiembre",
        Month::October => "Octubre",
        Month::November => "Noviembre",
        Month::December => "Diciembre",
    }
}

pub const fn weekday_name(day: Weekday) -> &'static str {
    match day {
        Weekday::Mon => "lunes",
        Weekday::Tue => "martes",
        Weekday::Wed => "miércoles",
        Weekday::Thu => "jueves",
        Weekday::Fri => "viernes",
        Weekday::Sat => "sábado",
        Weekday::Sun => "domingo",
    }
}

/// `text`, seguido de la descripción tras `separator` si la hay.
fn described(text: String, separator: &str, description: Option<&str>) -> String {
    match description {
        Some(description) => format!("{text}{separator}{description}"),
        None => text,
    }
}

fn mark_line(mark: &Mark) -> String {
    described(
        format!("#{:<3} {:<10} {}", mark.id, mark.kind, mark.range),
        "  ",
        mark.description.as_deref(),
    )
}

fn lines(lines: impl IntoIterator<Item = String>) -> String {
    lines
        .into_iter()
        .map(|line| format!("{}\n", line.trim_end()))
        .collect()
}

fn vacation_line(summary: &Summary) -> String {
    let line = format!(
        "Vacaciones restantes: {} / {}",
        summary.vacation_remaining(),
        summary.vacation_quota
    );
    match summary.vacation_excess() {
        0 => line,
        excess => format!("{line} ({} por encima del cupo)", days(excess)),
    }
}

/// Los días laborables entre los días del año, bajo las vacaciones restantes (RN-29).
fn working_days_line(summary: &Summary) -> String {
    format!(
        "Días trabajados: {} / {} ({}%)",
        summary.working_days,
        summary.year_days(),
        format!("{:.1}", summary.working_percent()).replace('.', ",")
    )
}

/// El texto del resultado de una consulta, terminado en salto de línea. `marker` decide cómo se
/// identifica el tipo de cada día (RN-20).
pub fn report(report: &Report, marker: Marker) -> String {
    report_with(report, marker, None)
}

/// [`report`] con las tablas de embalses y de mercados de la ventana debajo de la próxima
/// ausencia, en las vistas de mes y de año (RN-31, RN-32).
pub fn report_with(report: &Report, marker: Marker, panel: Option<&Panel>) -> String {
    match report {
        Report::Marks(marks) if marks.is_empty() => lines(iter::once("No hay marcas.".to_owned())),
        Report::Marks(marks) => lines(marks.iter().map(mark_line)),
        Report::Day(resolved) => day(resolved, marker),
        Report::Month(view, totals, absence) => {
            month(view, totals, absence.as_ref(), panel, marker)
        }
        Report::Year(months, totals, absence) => {
            year(months, totals, absence.as_ref(), panel, marker)
        }
        Report::Summary(totals) => summary(totals, marker),
        Report::Config(calendar) => config(calendar),
    }
}

#[cfg(test)]
mod tests {
    use super::calendar::MONTH_WIDTH;
    use super::text::HOLIDAYS_TO_CHECK;
    use super::*;
    use crate::application::{Command, Outcome, execute};
    use crate::domain::fixtures::{calendar, date, marked, today};
    use crate::domain::{Calendar, DateRange, MarkId, MarkKind, Month, MonthExt, Shift};

    /// La parte de la cuadrícula de una línea de la vista de mes, sin las vacaciones de su derecha
    /// (RN-27).
    fn grid(line: &str) -> String {
        columns(line, MONTH_WIDTH)
    }

    /// El ancho de la vista de año sin las vacaciones (RN-21).
    const YEAR_WIDTH: usize = 130;

    /// Las primeras `width` columnas de `line`, sin los espacios del final.
    fn columns(line: &str, width: usize) -> String {
        line.chars()
            .take(width)
            .collect::<String>()
            .trim_end()
            .to_owned()
    }

    fn plain(text: &str) -> String {
        anstream::adapter::strip_str(text).to_string()
    }

    fn rendered(calendar: Calendar, command: Command, marker: Marker) -> String {
        match execute(calendar, command) {
            Ok(Outcome::Report(result)) => report(&result, marker),
            other => format!("{other:?}"),
        }
    }

    /// El texto sin color, con letras: el que se ve al redirigir la salida.
    fn shown(calendar: Calendar, command: Command) -> String {
        plain(&rendered(calendar, command, Marker::Letter))
    }

    /// El texto con sus códigos de color y barras: el que se ve en un terminal.
    fn colored(calendar: Calendar, command: Command) -> String {
        rendered(calendar, command, Marker::Bar)
    }

    const LEGEND: &str = "Leyenda: L Laborable · R Reducido · l Laborable verano · r Reducido verano · S Fin de semana · F Festivo · V Vacaciones";

    #[test]
    fn rf_06_rf_14_rn_12_rn_30_month_is_a_monday_first_grid_with_hours_letters_and_legend() {
        let calendar = calendar()
            .with_mark(
                MarkKind::Vacation,
                DateRange::new(date(2026, 6, 5), date(2026, 6, 12)).expect("range"),
                Some("Viaje".to_owned()),
            )
            .expect("mark")
            .0;
        let expected = [
            "               Junio 2026                   ██ ██ ██ ██ ██",
            "Lu    Ma    Mi    Ju    Vi    Sá    Do      ██ ░░ ░░ ░░ ░░   Vacaciones",
            " 1 L   2 L   3 L   4 L   5 V   6 S   7 S    ░░ ░░ ░░ ░░ ░░   ██ Usadas      6",
            " 8 V   9 V  10 V  11 V  12 V  13 S  14 S    ░░ ░░ ░░ ░░ ░░   ░░ Restantes  19",
            "15 L  16 L  17 L  18 L  19 R  20 S  21 S    ░░ ░░ ░░ ░░ ░░",
            "22 L  23 L  24 L  25 L  26 R  27 S  28 S",
            "29 L  30 L                                  Próxima ausencia",
            "                                            V Vacaciones 5-12 junio",
            "",
            "Marcas:",
            "  #1   vacaciones 2026-06-05 → 2026-06-12  Viaje",
            "",
            "Horas laborables de junio: 131,0 h en 16 días · acumulado del año: 985,5 h",
            LEGEND,
            "Días trabajados: 255 / 365 (69,9%)",
            "",
        ]
        .join("\n");
        assert_eq!(
            shown(
                calendar,
                Command::Month {
                    year: 2026,
                    month: Month::June,
                    today: today(),
                }
            ),
            expected
        );
    }

    #[test]
    fn rf_06_month_starting_on_sunday_leaves_blank_cells() {
        let text = shown(
            marked(&[(MarkKind::Holiday, "2026-11-01", "2026-11-01")]),
            Command::Month {
                year: 2026,
                month: Month::November,
                today: today(),
            },
        );
        let lines: Vec<String> = text.lines().map(grid).collect();
        assert_eq!(
            lines.get(2).map(String::as_str),
            Some("                                     1 F"),
            "{text}"
        );
        assert_eq!(
            lines.get(3).map(String::as_str),
            Some(" 2 L   3 L   4 L   5 L   6 R   7 S   8 S"),
            "{text}"
        );
    }

    #[test]
    fn rf_06_rn_20_colors_identify_each_kind_with_a_bar_next_to_a_plain_number() {
        let calendar = marked(&[
            (MarkKind::Reduced, "2026-07-01", "2026-07-01"),
            (MarkKind::Reduced, "2026-06-01", "2026-06-01"),
            (MarkKind::Holiday, "2026-06-02", "2026-06-02"),
            (MarkKind::Vacation, "2026-06-03", "2026-06-03"),
        ]);
        let colored = colored(
            calendar,
            Command::Year {
                year: 2026,
                today: today(),
            },
        );
        [
            (" 4 \u{1b}[34m▌\u{1b}[0m", "laborable: azul"),
            (" 1 \u{1b}[36m▌\u{1b}[0m", "reducido: cian"),
            (" 2 \u{1b}[32m▌\u{1b}[0m", "laborable verano: verde"),
            (
                " 1 \u{1b}[38;5;151m▌\u{1b}[0m",
                "reducido verano: verde pálido",
            ),
            (" 6 \u{1b}[90m▌\u{1b}[0m", "gris"),
            (" 2 \u{1b}[31m▌\u{1b}[0m", "rojo"),
            (
                " 3 \u{1b}[1m\u{1b}[38;5;208m▌\u{1b}[0m",
                "naranja en negrita",
            ),
        ]
        .into_iter()
        .for_each(|(code, name)| assert!(colored.contains(code), "{name}"));
    }

    #[test]
    fn rn_20_with_color_a_bar_follows_the_number_and_without_color_the_letter() {
        let calendar = marked(&[(MarkKind::Holiday, "2026-06-15", "2026-06-15")]);
        let month = |marker| match execute(
            calendar.clone(),
            Command::Month {
                year: 2026,
                month: Month::June,
                today: today(),
            },
        ) {
            Ok(Outcome::Report(result)) => plain(&report(&result, marker)),
            other => format!("{other:?}"),
        };
        let bars = month(Marker::Bar);
        let letters = month(Marker::Letter);
        [
            (&bars, " 1 ▌   2 ▌   3 ▌   4 ▌   5 ▌   6 ▌   7 ▌"),
            (&bars, "15 ▌  16 ▌  17 ▌  18 ▌  19 ▌  20 ▌  21 ▌"),
            (&letters, " 1 L   2 L   3 L   4 L   5 R   6 S   7 S"),
            (&letters, "15 F  16 L  17 L  18 L  19 R  20 S  21 S"),
        ]
        .into_iter()
        .for_each(|(text, line)| {
            assert!(text.lines().any(|l| grid(l) == line), "{line}\n{text}");
        });
        assert!(
            bars.contains("Leyenda: ▌ Laborable · ▌ Reducido · ▌ Laborable verano"),
            "{bars}"
        );
        assert_eq!(Marker::for_color(true), Marker::Bar);
        assert_eq!(Marker::for_color(false), Marker::Letter);
    }

    #[test]
    fn rn_20_day_and_summary_use_the_marker() {
        let day = plain(&colored(calendar(), Command::Day(date(2026, 6, 5))));
        assert_eq!(
            day,
            "2026-06-05 (viernes) · ▌ Reducido · 6,0 h · jornada reducida de los viernes\n"
        );
        let summary = plain(&colored(calendar(), Command::Summary(2026)));
        assert!(
            summary.contains("\n  ▌ Fin de semana     104\n"),
            "{summary}"
        );
    }

    #[test]
    fn rf_07_rf_14_year_shows_three_months_per_row_with_their_hours() {
        let text = shown(
            marked(&[(MarkKind::Vacation, "2026-12-24", "2026-12-24")]),
            Command::Year {
                year: 2026,
                today: today(),
            },
        );
        let lines: Vec<&str> = text.lines().collect();
        let frame = |index: usize| lines.get(index).map(|line| columns(line, YEAR_WIDTH));
        assert_eq!(
            frame(3).as_deref(),
            Some(
                "┊                  Enero                   ┊                 Febrero                  ┊                  Marzo                   ┊"
            )
        );
        assert_eq!(
            frame(4).as_deref(),
            Some(
                "┊ Lu    Ma    Mi    Ju    Vi    Sá    Do   ┊ Lu    Ma    Mi    Ju    Vi    Sá    Do   ┊ Lu    Ma    Mi    Ju    Vi    Sá    Do   ┊"
            )
        );
        assert_eq!(
            frame(5).as_deref(),
            Some(
                "┊                    1 L   2 R   3 S   4 S ┊                                      1 S ┊                                      1 S ┊"
            )
        );
        // El marco superior (RN-21) y la franja del año con su línea (RN-23); cuatro filas de tres
        // meses, de 8 líneas, la de horas (RF-14) y la línea de la rejilla debajo; una en blanco
        // y, después, leyenda, días trabajados (RN-29), horas del año y media mensual (RN-24). Las
        // vacaciones restantes no se repiten: las da la cuadrícula (RN-27).
        assert_eq!(lines.len(), 3 + 4 * 10 + 5, "{text}");
        let hours = format!(
            "┊ {a:^40} ┊ {b:^40} ┊ {c:^40} ┊",
            a = "174,5 h · 22 días",
            b = "160,0 h · 20 días",
            c = "177,0 h · 22 días"
        );
        assert_eq!(lines.get(11), Some(&hours.as_str()));
        assert_eq!(lines.get(44), Some(&LEGEND));
        assert!(
            lines
                .get(45)
                .is_some_and(|line| line.starts_with("Días trabajados: ")),
            "{text}"
        );
        assert!(!text.contains("Vacaciones restantes"), "{text}");
        assert_eq!(
            lines.get(33),
            Some(
                &"┊                 Octubre                  ┊                Noviembre                 ┊                Diciembre                 ┊"
            )
        );
    }

    #[test]
    fn rn_27_the_vacation_grid_is_beside_the_first_row_of_the_year_and_of_the_month() {
        let calendar = marked(&[(MarkKind::Vacation, "2026-06-01", "2026-06-12")]);
        let year = shown(
            calendar.clone(),
            Command::Year {
                year: 2026,
                today: today(),
            },
        );
        let month = shown(
            calendar,
            Command::Month {
                year: 2026,
                month: Month::June,
                today: today(),
            },
        );
        // Las 5 líneas de la cuadrícula: 10 días usados de 25.
        let vacations = [
            "██ ██ ██ ██ ██",
            "██ ██ ██ ██ ██   Vacaciones",
            "░░ ░░ ░░ ░░ ░░   ██ Usadas     10",
            "░░ ░░ ░░ ░░ ░░   ░░ Restantes  15",
            "░░ ░░ ░░ ░░ ░░",
        ];
        [
            ("año", &year, YEAR_WIDTH, 3_usize),
            ("mes", &month, MONTH_WIDTH, 0),
        ]
        .into_iter()
        .for_each(|(case, text, width, from)| {
            let lines: Vec<&str> = text.lines().collect();
            let beside = |index: usize| {
                lines
                    .get(index)
                    .map(|line| line.chars().skip(width).collect::<String>())
            };
            assert_eq!(
                from.checked_sub(1).and_then(beside).unwrap_or_default(),
                "",
                "{case}: antes"
            );
            vacations.iter().enumerate().for_each(|(row, expected)| {
                assert_eq!(
                    beside(from + row),
                    Some(format!("    {expected}")),
                    "{case}: línea {row}\n{text}"
                );
            });
            assert_eq!(
                beside(from + vacations.len()),
                Some(String::new()),
                "{case}: después"
            );
        });
    }

    #[test]
    fn rn_21_year_frames_the_months_in_a_grey_dotted_grid() {
        let colored = colored(
            calendar(),
            Command::Year {
                year: 2026,
                today: today(),
            },
        );
        let text = plain(&colored);
        let lines: Vec<&str> = text.lines().collect();
        let segment = "┈".repeat(40);
        let rule = |[left, cross, right]: [char; 3]| {
            format!("{left}┈{segment}┈{cross}┈{segment}┈{cross}┈{segment}┈{right}")
        };
        [
            (
                0,
                rule(['┌', '┈', '┐']),
                "marco superior, sobre la franja del año",
            ),
            (2, rule(['├', '┬', '┤']), "entre la franja y la 1.ª fila"),
            (12, rule(['├', '┼', '┤']), "entre la 1.ª y la 2.ª fila"),
            (22, rule(['├', '┼', '┤']), "entre la 2.ª y la 3.ª fila"),
            (32, rule(['├', '┼', '┤']), "entre la 3.ª y la 4.ª fila"),
            (42, rule(['└', '┴', '┘']), "marco inferior"),
            (43, String::new(), "línea en blanco tras el marco"),
        ]
        .into_iter()
        .for_each(|(index, expected, case)| {
            assert_eq!(lines.get(index), Some(&expected.as_str()), "{case}");
        });
        lines
            .iter()
            .enumerate()
            .skip(3)
            .take(9)
            .for_each(|(index, line)| {
                let positions: Vec<usize> = line
                    .chars()
                    .enumerate()
                    .filter_map(|(position, c)| (c == '┊').then_some(position))
                    .collect();
                assert_eq!(positions, [0, 43, 86, 129], "línea {index}: {line}");
            });
        assert!(colored.contains(" \u{1b}[90m┊\u{1b}[0m "), "{colored}");
        assert!(
            colored.contains(&format!("\u{1b}[90m{}\u{1b}[0m", rule(['┌', '┈', '┐']))),
            "{colored}"
        );
    }

    #[test]
    fn rn_30_in_the_year_view_the_next_absence_starts_at_the_second_row_of_months() {
        let text = shown(
            marked(&[(MarkKind::Vacation, "2026-08-17", "2026-08-28")]),
            Command::Year {
                year: 2026,
                today: today(),
            },
        );
        // Lo que queda a la derecha de la rejilla.
        let right: Vec<String> = text
            .lines()
            .map(|line| {
                line.chars()
                    .skip(YEAR_WIDTH)
                    .collect::<String>()
                    .trim()
                    .to_owned()
            })
            .collect();
        assert_eq!(
            right.get(11..17),
            Some(
                &["", "", ABSENCE_TITLE, "V Vacaciones 17-28 agosto", "", "",].map(String::from)[..]
            ),
            "{text}"
        );
    }

    #[test]
    fn rn_24_year_ends_with_the_year_hours_and_the_monthly_average() {
        let text = shown(
            calendar(),
            Command::Year {
                year: 2026,
                today: today(),
            },
        );
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines.get(lines.len().saturating_sub(3)..),
            Some(
                &[
                    "Días trabajados: 236 / 365 (64,7%)",
                    "Horas laborables del año: 1888,5 h",
                    "Media mensual: 157,38 h (19,67 días de 8 h)",
                ][..]
            ),
            "{text}"
        );
    }

    #[test]
    fn rn_23_year_shows_the_year_once_in_a_band_above_the_months() {
        // Con una ausencia marcada, para que el aviso de RN-30, que lleva el año, no salga.
        let colored = colored(
            marked(&[(MarkKind::Holiday, "2026-12-08", "2026-12-08")]),
            Command::Year {
                year: 2026,
                today: today(),
            },
        );
        let text = plain(&colored);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines.get(1),
            Some(&format!("┊ {:^126} ┊", "Calendario laboral 2026").as_str())
        );
        assert_eq!(
            text.matches("2026").count(),
            1,
            "el año solo aparece en la franja: {text}"
        );
        assert!(
            colored.contains("\u{1b}[1mCalendario laboral 2026\u{1b}[0m"),
            "título en negrita: {colored}"
        );
    }

    #[test]
    fn rn_14_views_explain_the_default_vacation() {
        let text = shown(
            calendar(),
            Command::Month {
                year: 2026,
                month: Month::July,
                today: today(),
            },
        );
        assert!(text.contains(" 1 V   2 V   3 V   4 S   5 S"), "{text}");
        assert!(
            text.contains(
                "Vacaciones por defecto desde el 1 de julio: el año no tiene marcas de vacaciones."
            ),
            "{text}"
        );
        let year = shown(
            calendar(),
            Command::Year {
                year: 2026,
                today: today(),
            },
        );
        assert!(
            year.contains("Vacaciones por defecto desde el 1 de julio"),
            "{year}"
        );
        let june = shown(
            calendar(),
            Command::Month {
                year: 2026,
                month: Month::June,
                today: today(),
            },
        );
        assert!(!june.contains("por defecto"), "{june}");
    }

    #[test]
    fn rf_08_rf_14_summary_lists_days_hours_vacation_and_hours_per_month() {
        let calendar = marked(&[(MarkKind::Vacation, "2026-06-05", "2026-06-12")]);
        let expected = [
            "Resumen 2026",
            "  L Laborable         170",
            "  R Reducido           41",
            "  l Laborable verano   35",
            "  r Reducido verano     9",
            "  S Fin de semana     104",
            "  F Festivo             0",
            "  V Vacaciones          6",
            "Días laborables: 255",
            "Horas laborables: 2042,5 h",
            "Vacaciones: 6 usadas de 25",
            "Vacaciones restantes: 19 / 25",
            "Días trabajados: 255 / 365 (69,9%)",
            "",
            "Horas por mes    Días     Horas  Acumulado",
            "  Enero            22     174,5      174,5",
            "  Febrero          20     160,0      334,5",
            "  Marzo            22     177,0      511,5",
            "  Abril            22     177,0      688,5",
            "  Mayo             21     166,0      854,5",
            "  Junio            16     131,0      985,5",
            "  Julio            23     183,0     1168,5",
            "  Agosto           21     168,5     1337,0",
            "  Septiembre       22     177,0     1514,0",
            "  Octubre          22     174,5     1688,5",
            "  Noviembre        21     168,5     1857,0",
            "  Diciembre        23     185,5     2042,5",
            "  Total           255    2042,5",
            "",
        ]
        .join("\n");
        assert_eq!(shown(calendar, Command::Summary(2026)), expected);
    }

    #[test]
    fn rn_11_summary_shows_the_excess() {
        let calendar = marked(&[(MarkKind::Vacation, "2026-06-01", "2026-07-07")]);
        assert!(
            shown(calendar, Command::Summary(2026))
                .contains("Vacaciones restantes: 0 / 25 (2 días por encima del cupo)\n")
        );
    }

    #[test]
    fn rf_05_day_shows_kind_hours_origin_and_description() {
        let described = calendar()
            .with_mark(
                MarkKind::Holiday,
                DateRange::day(date(2026, 3, 3)),
                Some("Fiesta local".to_owned()),
            )
            .expect("mark")
            .0;
        [
            (
                described,
                date(2026, 3, 3),
                "2026-03-03 (martes) · F Festivo · 0,0 h · marca #1 · Fiesta local\n",
            ),
            (calendar(), date(2026, 6, 8), "2026-06-08 (lunes) · L Laborable · 8,5 h · por defecto\n"),
            (calendar(), date(2026, 6, 7), "2026-06-07 (domingo) · S Fin de semana · 0,0 h · fin de semana\n"),
            (
                calendar(),
                date(2026, 6, 19),
                "2026-06-19 (viernes) · R Reducido · 6,0 h · jornada reducida de los viernes\n",
            ),
            (
                calendar(),
                date(2026, 7, 1),
                "2026-07-01 (miércoles) · V Vacaciones · 0,0 h · vacaciones por defecto desde el 1 de julio\n",
            ),
        ]
        .into_iter()
        .for_each(|(calendar, date, expected)| {
            assert_eq!(shown(calendar, Command::Day(date)), expected, "{date}");
        });
    }

    #[test]
    fn rf_04_list_shows_one_mark_per_line() {
        let calendar = marked(&[
            (MarkKind::Vacation, "2026-06-05", "2026-06-12"),
            (MarkKind::Holiday, "2026-03-03", "2026-03-03"),
        ]);
        assert_eq!(
            shown(calendar, Command::List(None)),
            "#2   festivo    2026-03-03\n#1   vacaciones 2026-06-05 → 2026-06-12\n"
        );
        assert_eq!(
            shown(Calendar::default(), Command::List(None)),
            "No hay marcas.\n"
        );
    }

    #[test]
    fn rf_15_config_shows_the_weekend_the_reduced_days_and_each_year() {
        let calendar = calendar()
            .with_reduced_days(&[Weekday::Mon, Weekday::Fri])
            .with_year(2027)
            .and_then(|calendar| calendar.with_shift_hours(2027, Shift::Working, 8.0))
            .and_then(|calendar| calendar.with_quota(2027, 23))
            .expect("calendar");
        let text = shown(calendar, Command::Config);
        let words: Vec<Vec<&str>> = text
            .lines()
            .map(|line| line.split_whitespace().collect())
            .collect();
        assert_eq!(
            words,
            [
                vec!["Fin", "de", "semana:", "sábado", "y", "domingo"],
                vec!["Jornada", "reducida:", "lunes", "y", "viernes"],
                vec![],
                vec![
                    "Año",
                    "Cupo",
                    "laborable",
                    "reducido",
                    "laborable-verano",
                    "reducido-verano"
                ],
                vec!["2026", "25", "8,5", "h", "6,0", "h", "8,5", "h", "6,0", "h"],
                vec!["2027", "23", "8,0", "h", "6,0", "h", "8,5", "h", "6,0", "h"],
            ],
            "{text}"
        );
        assert!(
            text.lines().skip(3).all(|line| line.chars().count()
                == text
                    .lines()
                    .nth(3)
                    .map_or(0, |header| header.chars().count())),
            "columnas alineadas: {text}"
        );
        assert_eq!(
            shown(
                Calendar::default()
                    .with_weekend(&[Weekday::Sun])
                    .with_reduced_days(&[]),
                Command::Config
            ),
            "Fin de semana: domingo\nJornada reducida: ninguno\n\nNo hay años; añade uno con «calendario add-year <año>».\n"
        );
    }

    /// El único sitio con el texto del aviso de RF-19; los demás tests usan la constante.
    #[test]
    fn rf_19_the_holiday_warning_is_fixed() {
        assert_eq!(
            HOLIDAYS_TO_CHECK,
            "aviso: revisa los festivos que cambian de fecha cada año, como Semana Santa o los trasladados a lunes"
        );
    }

    #[test]
    fn counts_use_the_singular_only_for_one() {
        [(0, "0 días"), (1, "1 día"), (2, "2 días"), (25, "25 días")]
            .into_iter()
            .for_each(|(n, expected)| assert_eq!(count(n, "día", "días"), expected, "{n}"));
    }

    #[test]
    fn rn_15_hours_use_a_decimal_comma() {
        [
            (8.5, "8,5"),
            (6.0, "6,0"),
            (7.25, "7,25"),
            (0.0, "0,0"),
            (2184.5, "2184,5"),
            (24.0, "24,0"),
        ]
        .into_iter()
        .for_each(|(hours, text)| assert_eq!(decimal(hours), text, "{hours}"));
    }

    #[test]
    fn rf_01_to_rf_10_events_confirm_the_change() {
        let mark = Mark {
            id: MarkId(3),
            kind: MarkKind::Vacation,
            range: DateRange::new(date(2026, 6, 5), date(2026, 6, 12)).expect("range"),
            description: None,
        };
        [
            (
                Event::YearRemoved { year: 2025, marks: 0 },
                "Año 2025 eliminado.",
                None,
            ),
            (
                Event::YearRemoved { year: 2025, marks: 1 },
                "Año 2025 eliminado, con 1 marca.",
                None,
            ),
            (
                Event::YearRemoved { year: 2025, marks: 12 },
                "Año 2025 eliminado, con 12 marcas.",
                None,
            ),
            (
                Event::YearAdded {
                    year: 2027,
                    quota: 25,
                    holidays: None,
                },
                "Año 2027 añadido con un cupo de 25 días de vacaciones.",
                None,
            ),
            (
                Event::YearAdded {
                    year: 2027,
                    quota: 25,
                    holidays: Some((2026, 1)),
                },
                "Año 2027 añadido con un cupo de 25 días de vacaciones y 1 festivo copiado de 2026.",
                Some(HOLIDAYS_TO_CHECK),
            ),
            (
                Event::YearAdded {
                    year: 2027,
                    quota: 25,
                    holidays: Some((2026, 9)),
                },
                "Año 2027 añadido con un cupo de 25 días de vacaciones y 9 festivos copiados de 2026.",
                Some(HOLIDAYS_TO_CHECK),
            ),
            (
                Event::Marked {
                    mark: mark.clone(),
                    excess: 0,
                },
                "Marca #3 añadida: vacaciones 2026-06-05 → 2026-06-12.",
                None,
            ),
            (
                Event::Marked {
                    mark: mark.clone(),
                    excess: 1,
                },
                "Marca #3 añadida: vacaciones 2026-06-05 → 2026-06-12.",
                Some("aviso: las vacaciones de 2026 superan el cupo en 1 día"),
            ),
            (
                Event::Edited {
                    mark: mark.clone(),
                    excess: 0,
                },
                "Marca #3 modificada: vacaciones 2026-06-05 → 2026-06-12.",
                None,
            ),
            (
                Event::Edited {
                    mark: mark.clone(),
                    excess: 3,
                },
                "Marca #3 modificada: vacaciones 2026-06-05 → 2026-06-12.",
                Some("aviso: las vacaciones de 2026 superan el cupo en 3 días"),
            ),
            (
                Event::AlreadyMarked(MarkId(3)),
                "Ya existe una marca idéntica (#3); no se ha añadido.",
                None,
            ),
            (
                Event::Unmarked(vec![mark.clone()]),
                "Marca #3 eliminada: vacaciones 2026-06-05 → 2026-06-12.",
                None,
            ),
            (
                Event::Unmarked(vec![
                    mark.clone(),
                    Mark {
                        id: MarkId(4),
                        ..mark
                    },
                ]),
                "Marca #3 eliminada: vacaciones 2026-06-05 → 2026-06-12.\nMarca #4 eliminada: vacaciones 2026-06-05 → 2026-06-12.",
                None,
            ),
            (
                Event::HoursSet {
                    year: 2026,
                    shift: Shift::SummerWorking,
                    hours: 8.5,
                },
                "Jornada laborable-verano de 2026: 8,5 h.",
                None,
            ),
            (
                Event::ReducedDaysSet(vec![Weekday::Mon, Weekday::Fri]),
                "Jornada reducida: lunes y viernes.",
                None,
            ),
            (
                Event::ReducedDaysSet(Vec::new()),
                "Jornada reducida: ninguno.",
                None,
            ),
            (
                Event::WeekendSet(vec![Weekday::Sat, Weekday::Sun]),
                "Fin de semana: sábado y domingo.",
                None,
            ),
            (
                Event::QuotaSet {
                    year: 2026,
                    days: 22,
                    excess: 0,
                },
                "Cupo de vacaciones de 2026: 22 días.",
                None,
            ),
            (
                Event::QuotaSet {
                    year: 2026,
                    days: 3,
                    excess: 2,
                },
                "Cupo de vacaciones de 2026: 3 días.",
                Some("aviso: las vacaciones de 2026 superan el cupo en 2 días"),
            ),
        ]
        .into_iter()
        .for_each(|(event_, text, warning)| {
            assert_eq!(
                event(&event_),
                Message {
                    text: text.to_owned(),
                    warning: warning.map(str::to_owned),
                },
                "{event_:?}"
            );
        });
    }

    #[test]
    fn month_names_are_spanish() {
        let names: Vec<&str> = Month::ALL.into_iter().map(month_name).collect();
        assert_eq!(
            names,
            [
                "Enero",
                "Febrero",
                "Marzo",
                "Abril",
                "Mayo",
                "Junio",
                "Julio",
                "Agosto",
                "Septiembre",
                "Octubre",
                "Noviembre",
                "Diciembre"
            ]
        );
    }

    #[test]
    fn rn_31_rn_32_reservoirs_and_markets_go_below_the_next_absence_only_in_the_window() {
        use crate::adapters::reservoirs::fixtures::RESERVOIRS;

        let table = Panel::new(&RESERVOIRS);
        [
            Command::Month {
                year: 2026,
                month: Month::June,
                today: today(),
            },
            Command::Year {
                year: 2026,
                today: today(),
            },
        ]
        .into_iter()
        .for_each(|command| {
            let result = match execute(calendar(), command.clone()) {
                Ok(Outcome::Report(result)) => result,
                other => panic!("{other:?}"),
            };
            let window = plain(&report_with(&result, Marker::Letter, Some(&table)));
            let lines: Vec<&str> = window.lines().collect();
            let find = |title: &str| {
                lines
                    .iter()
                    .enumerate()
                    .find_map(|(index, line)| line.find(title).map(|column| (index, column)))
            };
            let (absence, absence_column) = find(ABSENCE_TITLE).expect("absence");
            let (title, title_column) = find(RESERVOIRS_TITLE).expect("reservoirs");
            // La ausencia ocupa dos líneas (el título y el aviso), y luego va una en blanco.
            assert_eq!(title, absence + 3, "{command:?}");
            assert_eq!(
                lines[title].chars().take(title_column).count(),
                lines[absence].chars().take(absence_column).count(),
                "{command:?}"
            );
            assert!(lines[title + 3].contains("○ El Pardo"), "{command:?}");
            // RN-32: los mercados, tras el título, la cabecera, los tres embalses (aún sin fuente) y
            // una línea en blanco, en la misma columna.
            let (markets, markets_column) = find(MARKETS_TITLE).expect("markets");
            assert_eq!(markets, title + 6, "{command:?}");
            assert_eq!(
                lines[markets].chars().take(markets_column).count(),
                lines[title].chars().take(title_column).count(),
                "{command:?}"
            );
            assert!(lines[markets + 2].contains("○ IBEX 35"), "{command:?}");
            assert!(
                !plain(&report(&result, Marker::Letter)).contains(RESERVOIRS_TITLE),
                "the console has no reservoirs: {command:?}"
            );
        });
    }
}
