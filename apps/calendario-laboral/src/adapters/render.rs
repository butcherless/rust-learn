//! Adaptador de presentación: el texto de consola de cada `Outcome`, en español (§5, RNF-07).
//!
//! Cada día se muestra con su color y su letra, para que se lea igual sin color.
//! Las funciones devuelven texto con códigos ANSI; `anstream` los quita cuando la salida no es
//! un terminal o está definida `NO_COLOR`.

use std::iter;

use anstyle::{AnsiColor, Style};
use chrono::{Datelike, Weekday};

use crate::application::{Event, MonthView, Report};
use crate::domain::{DayKind, Mark, Month, MonthTotal, Origin, ResolvedDay, Summary};

/// Ancho visible de un mes en la cuadrícula: siete celdas de tres caracteres y seis espacios.
const MONTH_WIDTH: usize = 27;
/// Semanas que puede ocupar un mes, para que todos los meses de la vista de año midan lo mismo.
const MAX_WEEKS: usize = 6;
/// Meses por fila en la vista de año.
const MONTHS_PER_ROW: usize = 3;
const HEADER: &str = "Lu  Ma  Mi  Ju  Vi  Sá  Do";
const BLANK_CELL: &str = "   ";

/// El texto de un cambio y un aviso que se muestra aparte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// La confirmación.
    pub text: String,
    /// Un aviso, como unas vacaciones por encima del cupo (RN-11).
    pub warning: Option<String>,
}

const fn color(kind: DayKind) -> AnsiColor {
    match kind {
        DayKind::Working => AnsiColor::Green,
        DayKind::Reduced => AnsiColor::Yellow,
        DayKind::SummerWorking => AnsiColor::Cyan,
        DayKind::SummerReduced => AnsiColor::Blue,
        DayKind::Weekend => AnsiColor::BrightBlack,
        DayKind::Holiday => AnsiColor::Red,
        DayKind::Vacation => AnsiColor::Magenta,
    }
}

fn paint(kind: DayKind, text: &str) -> String {
    let style = Style::new().fg_color(Some(color(kind).into()));
    format!("{style}{text}{style:#}")
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

fn days(count: usize) -> String {
    match count {
        1 => "1 día".to_owned(),
        _ => format!("{count} días"),
    }
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

const fn weekday_name(day: Weekday) -> &'static str {
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

fn mark_line(mark: &Mark) -> String {
    let text = format!("#{:<3} {:<10} {}", mark.id, mark.kind, mark.range);
    mark.description
        .as_ref()
        .map_or(text.clone(), |description| format!("{text}  {description}"))
}

fn legend() -> String {
    let items: Vec<String> = DayKind::ALL
        .into_iter()
        .map(|kind| {
            format!(
                "{} {}",
                paint(kind, &kind.letter().to_string()),
                kind.label()
            )
        })
        .collect();
    format!("Leyenda: {}", items.join(" · "))
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

fn cell(day: &ResolvedDay) -> String {
    paint(
        day.kind,
        &format!("{:>2}{}", day.date.day(), day.kind.letter()),
    )
}

/// Las líneas de un mes, de [`MONTH_WIDTH`] caracteres, completadas hasta `weeks` semanas.
fn month_grid(view: &MonthView, weeks: usize) -> Vec<String> {
    let leading = view
        .days
        .first()
        .map_or(0, |day| day.date.weekday().num_days_from_monday() as usize);
    let cells: Vec<String> = iter::repeat_n(BLANK_CELL.to_owned(), leading)
        .chain(view.days.iter().map(cell))
        .collect();
    let rows: Vec<String> = cells
        .chunks(7)
        .map(|week| {
            week.iter()
                .cloned()
                .chain(iter::repeat_n(BLANK_CELL.to_owned(), 7 - week.len()))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    let blank_rows = weeks.saturating_sub(rows.len());
    [
        format!(
            "{:^MONTH_WIDTH$}",
            format!("{} {}", month_name(view.month), view.year)
        ),
        format!("{HEADER:<MONTH_WIDTH$}"),
    ]
    .into_iter()
    .chain(rows)
    .chain(iter::repeat_n(" ".repeat(MONTH_WIDTH), blank_rows))
    .collect()
}

fn lines(lines: impl IntoIterator<Item = String>) -> String {
    lines
        .into_iter()
        .map(|line| format!("{}\n", line.trim_end()))
        .collect()
}

fn default_vacation_note(days: &[ResolvedDay]) -> Option<String> {
    days.iter()
        .any(|day| day.origin == Origin::DefaultVacation)
        .then(|| {
            "Vacaciones por defecto desde el 1 de julio: el año no tiene marcas de vacaciones."
                .to_owned()
        })
}

/// La línea de horas de la vista de mes (RF-14).
fn month_hours_line(total: &MonthTotal) -> String {
    format!(
        "Horas laborables de {}: {} h en {} · acumulado del año: {} h",
        month_name(total.month).to_lowercase(),
        decimal(total.working_hours),
        days(total.working_days),
        decimal(total.cumulative_hours)
    )
}

/// La línea de horas bajo cada mes de la vista de año, de [`MONTH_WIDTH`] caracteres (RF-14).
fn month_hours_cell(summary: &Summary, month: Month) -> String {
    let text = summary.month(month).map_or_else(String::new, |total| {
        format!(
            "{} h · {}",
            decimal(total.working_hours),
            days(total.working_days)
        )
    });
    format!("{text:^MONTH_WIDTH$}")
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

fn month(view: &MonthView, summary: &Summary) -> String {
    let marks = (!view.marks.is_empty())
        .then(|| {
            iter::once(String::new())
                .chain(iter::once("Marcas:".to_owned()))
                .chain(
                    view.marks
                        .iter()
                        .map(|mark| format!("  {}", mark_line(mark))),
                )
        })
        .into_iter()
        .flatten();
    lines(
        month_grid(view, 0)
            .into_iter()
            .chain(marks)
            .chain(iter::once(String::new()))
            .chain(default_vacation_note(&view.days))
            .chain(summary.month(view.month).map(month_hours_line))
            .chain([legend(), vacation_line(summary)]),
    )
}

fn year(months: &[MonthView], summary: &Summary) -> String {
    let rows = months.chunks(MONTHS_PER_ROW).flat_map(|row| {
        let grids: Vec<Vec<String>> = row
            .iter()
            .map(|view| {
                month_grid(view, MAX_WEEKS)
                    .into_iter()
                    .chain(iter::once(month_hours_cell(summary, view.month)))
                    .collect()
            })
            .collect();
        let height = grids.first().map_or(0, Vec::len);
        (0..height)
            .map(move |index| {
                grids
                    .iter()
                    .filter_map(|grid| grid.get(index).cloned())
                    .collect::<Vec<_>>()
                    .join("   ")
            })
            .chain(iter::once(String::new()))
    });
    let all_days: Vec<ResolvedDay> = months.iter().flat_map(|view| view.days.clone()).collect();
    lines(
        rows.chain(default_vacation_note(&all_days))
            .chain([legend(), vacation_line(summary)]),
    )
}

fn summary(summary: &Summary) -> String {
    let counts = summary.counts.iter().map(|(kind, count)| {
        format!(
            "  {} {:<17}{:>4}",
            paint(*kind, &kind.letter().to_string()),
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

fn day(day: &ResolvedDay) -> String {
    let parts = [
        format!("{} ({})", day.date, weekday_name(day.date.weekday())),
        paint(
            day.kind,
            &format!("{} {}", day.kind.letter(), day.kind.label()),
        ),
        format!("{} h", decimal(day.hours)),
        origin(day),
    ];
    let text = parts.join(" · ");
    lines(iter::once(
        day.description
            .as_ref()
            .map_or(text.clone(), |description| {
                format!("{text} · {description}")
            }),
    ))
}

/// El texto del resultado de una consulta, terminado en salto de línea.
pub fn report(report: &Report) -> String {
    match report {
        Report::Marks(marks) if marks.is_empty() => lines(iter::once("No hay marcas.".to_owned())),
        Report::Marks(marks) => lines(marks.iter().map(mark_line)),
        Report::Day(resolved) => day(resolved),
        Report::Month(view, totals) => month(view, totals),
        Report::Year(months, totals) => year(months, totals),
        Report::Summary(totals) => summary(totals),
    }
}

/// La confirmación de un cambio y su aviso, si lo hay.
pub fn event(event: &Event) -> Message {
    let text = match event {
        Event::YearAdded { year, quota } => {
            format!(
                "Año {year} añadido con un cupo de {} de vacaciones.",
                days(*quota as usize)
            )
        }
        Event::Marked { mark, .. } => {
            format!("Marca #{} añadida: {} {}.", mark.id, mark.kind, mark.range)
        }
        Event::AlreadyMarked(id) => {
            format!("Ya existe una marca idéntica (#{id}); no se ha añadido.")
        }
        Event::Unmarked(mark) => {
            format!(
                "Marca #{} eliminada: {} {}.",
                mark.id, mark.kind, mark.range
            )
        }
        Event::HoursSet(shift, hours) => format!("Jornada {shift}: {} h.", decimal(*hours)),
        Event::QuotaSet { year, days: quota } => {
            format!("Cupo de vacaciones de {year}: {}.", days(*quota as usize))
        }
    };
    let warning = match event {
        Event::Marked { mark, excess } if *excess > 0 => Some(format!(
            "aviso: las vacaciones de {} superan el cupo en {}",
            mark.range.from().year(),
            days(*excess)
        )),
        Event::Marked { .. }
        | Event::YearAdded { .. }
        | Event::AlreadyMarked(_)
        | Event::Unmarked(_)
        | Event::HoursSet(..)
        | Event::QuotaSet { .. } => None,
    };
    Message { text, warning }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{Command, Outcome, execute};
    use crate::domain::fixtures::{calendar, date, marked};
    use crate::domain::{Calendar, DateRange, MarkId, MarkKind, Shift};

    fn plain(text: &str) -> String {
        anstream::adapter::strip_str(text).to_string()
    }

    fn shown(calendar: Calendar, command: Command) -> String {
        match execute(calendar, command) {
            Ok(Outcome::Report(result)) => plain(&report(&result)),
            other => format!("{other:?}"),
        }
    }

    const LEGEND: &str = "Leyenda: L Laborable · R Reducido · l Laborable verano · r Reducido verano · S Fin de semana · F Festivo · V Vacaciones";

    #[test]
    fn rf_06_rf_14_rn_12_month_is_a_monday_first_grid_with_hours_letters_and_legend() {
        let calendar = calendar()
            .with_mark(
                MarkKind::Vacation,
                DateRange::new(date(2026, 6, 5), date(2026, 6, 12)).expect("range"),
                Some("Viaje".to_owned()),
            )
            .expect("mark")
            .0;
        let expected = [
            "        Junio 2026",
            "Lu  Ma  Mi  Ju  Vi  Sá  Do",
            " 1L  2L  3L  4L  5V  6S  7S",
            " 8V  9V 10V 11V 12V 13S 14S",
            "15L 16L 17L 18L 19R 20S 21S",
            "22L 23L 24L 25L 26R 27S 28S",
            "29L 30L",
            "",
            "Marcas:",
            "  #1   vacaciones 2026-06-05 → 2026-06-12  Viaje",
            "",
            "Horas laborables de junio: 131,0 h en 16 días · acumulado del año: 985,5 h",
            LEGEND,
            "Vacaciones restantes: 19 / 25",
            "",
        ]
        .join("\n");
        assert_eq!(
            shown(
                calendar,
                Command::Month {
                    year: 2026,
                    month: Month::June
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
            },
        );
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.get(2), Some(&"                         1F"), "{text}");
        assert_eq!(lines.get(3), Some(&" 2L  3L  4L  5L  6R  7S  8S"), "{text}");
    }

    #[test]
    fn rf_06_colors_identify_each_kind() {
        let calendar = marked(&[
            (MarkKind::Reduced, "2026-07-01", "2026-07-01"),
            (MarkKind::Reduced, "2026-06-01", "2026-06-01"),
            (MarkKind::Holiday, "2026-06-02", "2026-06-02"),
            (MarkKind::Vacation, "2026-06-03", "2026-06-03"),
        ]);
        let colored = match execute(calendar, Command::Year(2026)) {
            Ok(Outcome::Report(result)) => report(&result),
            other => format!("{other:?}"),
        };
        [
            ("\u{1b}[32m 4L\u{1b}[0m", "verde"),
            ("\u{1b}[33m 1R\u{1b}[0m", "amarillo"),
            ("\u{1b}[36m 2l\u{1b}[0m", "cian"),
            ("\u{1b}[34m 1r\u{1b}[0m", "azul"),
            ("\u{1b}[90m 6S\u{1b}[0m", "gris"),
            ("\u{1b}[31m 2F\u{1b}[0m", "rojo"),
            ("\u{1b}[35m 3V\u{1b}[0m", "magenta"),
        ]
        .into_iter()
        .for_each(|(code, name)| assert!(colored.contains(code), "{name}"));
    }

    #[test]
    fn rf_07_rf_14_year_shows_three_months_per_row_with_their_hours() {
        let text = shown(
            marked(&[(MarkKind::Vacation, "2026-12-24", "2026-12-24")]),
            Command::Year(2026),
        );
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines.first(),
            Some(&"        Enero 2026                   Febrero 2026                   Marzo 2026")
        );
        assert_eq!(
            lines.get(1),
            Some(
                &"Lu  Ma  Mi  Ju  Vi  Sá  Do    Lu  Ma  Mi  Ju  Vi  Sá  Do    Lu  Ma  Mi  Ju  Vi  Sá  Do"
            )
        );
        assert_eq!(
            lines.get(2),
            Some(
                &"             1L  2R  3S  4S                            1S                            1S"
            )
        );
        // Cuatro filas de tres meses, de 8 líneas, la de horas (RF-14) y una en blanco; después,
        // leyenda y cupo.
        assert_eq!(lines.len(), 4 * 10 + 2, "{text}");
        let hours = format!(
            "{a:^27}   {b:^27}   {c:^27}",
            a = "174,5 h · 22 días",
            b = "160,0 h · 20 días",
            c = "177,0 h · 22 días"
        );
        assert_eq!(lines.get(8), Some(&hours.trim_end()));
        assert_eq!(lines.get(40), Some(&LEGEND));
        assert_eq!(lines.get(41), Some(&"Vacaciones restantes: 24 / 25"));
        assert_eq!(
            lines.get(30),
            Some(
                &"       Octubre 2026                 Noviembre 2026                Diciembre 2026"
            )
        );
    }

    #[test]
    fn rn_14_views_explain_the_default_vacation() {
        let text = shown(
            calendar(),
            Command::Month {
                year: 2026,
                month: Month::July,
            },
        );
        assert!(text.contains(" 1V  2V  3V  4S  5S"), "{text}");
        assert!(
            text.contains(
                "Vacaciones por defecto desde el 1 de julio: el año no tiene marcas de vacaciones."
            ),
            "{text}"
        );
        assert!(text.ends_with("Vacaciones restantes: 0 / 25\n"), "{text}");
        let year = shown(calendar(), Command::Year(2026));
        assert!(
            year.contains("Vacaciones por defecto desde el 1 de julio"),
            "{year}"
        );
        let june = shown(
            calendar(),
            Command::Month {
                year: 2026,
                month: Month::June,
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
                Event::YearAdded {
                    year: 2027,
                    quota: 25,
                },
                "Año 2027 añadido con un cupo de 25 días de vacaciones.",
                None,
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
                Event::AlreadyMarked(MarkId(3)),
                "Ya existe una marca idéntica (#3); no se ha añadido.",
                None,
            ),
            (
                Event::Unmarked(mark),
                "Marca #3 eliminada: vacaciones 2026-06-05 → 2026-06-12.",
                None,
            ),
            (
                Event::HoursSet(Shift::SummerWorking, 8.5),
                "Jornada laborable-verano: 8,5 h.",
                None,
            ),
            (
                Event::QuotaSet {
                    year: 2026,
                    days: 22,
                },
                "Cupo de vacaciones de 2026: 22 días.",
                None,
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
}
