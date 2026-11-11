//! Adaptador de presentación: el texto de consola de cada `Outcome`, en español (§5, RNF-07).
//!
//! Cada día se muestra con su color y su letra, para que se lea igual sin color.
//! Las funciones devuelven texto con códigos ANSI; `anstream` los quita cuando la salida no es
//! un terminal o está definida `NO_COLOR`.

use std::iter;

use anstyle::{Ansi256Color, AnsiColor, Color, Style};
use chrono::{Datelike, Weekday};

use crate::application::{Event, MonthView, Report};
use crate::domain::{DayKind, Mark, Month, MonthExt, MonthTotal, Origin, ResolvedDay, Summary};

/// Ancho visible de una celda de día: el número a dos posiciones, un espacio y el símbolo.
const CELL_WIDTH: usize = 4;
/// Separación entre dos días, mayor que la del número y su símbolo para que se vea de qué
/// número es cada símbolo (RN-20).
const CELL_GAP: &str = "  ";
/// Ancho visible de un mes en la cuadrícula: siete celdas y seis separaciones.
const MONTH_WIDTH: usize = 7 * CELL_WIDTH + 6 * CELL_GAP.len();
/// Semanas que puede ocupar un mes, para que todos los meses de la vista de año midan lo mismo.
const MAX_WEEKS: usize = 6;
/// Meses por fila en la vista de año.
const MONTHS_PER_ROW: usize = 3;
/// La línea punteada vertical entre dos meses y en los lados de la vista de año (RN-21).
const GRID_VERTICAL: &str = "┊";
/// La línea punteada horizontal entre dos filas de meses y arriba y abajo (RN-21).
const GRID_HORIZONTAL: &str = "┈";
/// Los días de la semana, alineados con el número de cada celda.
const HEADER: &str = "Lu    Ma    Mi    Ju    Vi    Sá    Do";
const BLANK_CELL: &str = "    ";

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

/// El estilo de cada `DayKind`: su color y, en `Vacation`, negrita para separarla del rojo de
/// `Holiday`. En cada pareja laborable, la jornada reducida es más tenue que la completa: cian
/// frente a azul y, en `Summer`, un verde pálido de la paleta de 256 frente al verde.
const fn style(kind: DayKind) -> Style {
    let style = Style::new().fg_color(Some(color(kind)));
    match kind {
        DayKind::Vacation => style.bold(),
        DayKind::Working
        | DayKind::Reduced
        | DayKind::SummerWorking
        | DayKind::SummerReduced
        | DayKind::Weekend
        | DayKind::Holiday => style,
    }
}

const fn color(kind: DayKind) -> Color {
    match kind {
        DayKind::Working => Color::Ansi(AnsiColor::Blue),
        DayKind::Reduced => Color::Ansi(AnsiColor::Cyan),
        DayKind::SummerWorking => Color::Ansi(AnsiColor::Green),
        DayKind::SummerReduced => Color::Ansi256(Ansi256Color(151)),
        DayKind::Weekend => Color::Ansi(AnsiColor::BrightBlack),
        DayKind::Holiday => Color::Ansi(AnsiColor::Red),
        DayKind::Vacation => Color::Ansi256(Ansi256Color(208)),
    }
}

fn paint(kind: DayKind, text: &str) -> String {
    styled(style(kind), text)
}

fn styled(style: Style, text: &str) -> String {
    format!("{style}{text}{style:#}")
}

/// Las líneas de la rejilla de la vista de año, en gris para que no compitan con los días (RN-21).
fn paint_grid(text: &str) -> String {
    styled(
        Style::new().fg_color(Some(AnsiColor::BrightBlack.into())),
        text,
    )
}

/// Una línea de la vista de año: los meses separados por la línea vertical, con un espacio a
/// cada lado, y entre las dos líneas del marco (RN-21).
fn grid_row(cells: &[String]) -> String {
    let vertical = paint_grid(GRID_VERTICAL);
    format!(
        "{vertical} {} {vertical}",
        cells.join(&format!(" {vertical} "))
    )
}

/// Una línea horizontal de la rejilla para `months` meses: `left` y `right` en los extremos y
/// `cross` bajo cada separación entre meses (RN-21).
fn grid_rule(months: usize, [left, cross, right]: [char; 3]) -> String {
    let segment = GRID_HORIZONTAL.repeat(MONTH_WIDTH);
    let inner = vec![segment; months].join(&format!("{GRID_HORIZONTAL}{cross}{GRID_HORIZONTAL}"));
    paint_grid(&format!(
        "{left}{GRID_HORIZONTAL}{inner}{GRID_HORIZONTAL}{right}"
    ))
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

fn legend(marker: Marker) -> String {
    let items: Vec<String> = DayKind::ALL
        .into_iter()
        .map(|kind| format!("{} {}", marker.symbol(kind), kind.label()))
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

/// Un día de la cuadrícula: el número sin color y, a su derecha, el símbolo de su tipo (RN-20).
fn cell(day: &ResolvedDay, marker: Marker) -> String {
    format!("{:>2} {}", day.date.day(), marker.symbol(day.kind))
}

/// Las líneas de un mes, de [`MONTH_WIDTH`] caracteres, completadas hasta `weeks` semanas.
fn month_grid(view: &MonthView, title: &str, weeks: usize, marker: Marker) -> Vec<String> {
    let leading = view
        .days
        .first()
        .map_or(0, |day| day.date.weekday().num_days_from_monday() as usize);
    let cells: Vec<String> = iter::repeat_n(BLANK_CELL.to_owned(), leading)
        .chain(view.days.iter().map(|day| cell(day, marker)))
        .collect();
    let rows: Vec<String> = cells
        .chunks(7)
        .map(|week| {
            week.iter()
                .cloned()
                .chain(iter::repeat_n(BLANK_CELL.to_owned(), 7 - week.len()))
                .collect::<Vec<_>>()
                .join(CELL_GAP)
        })
        .collect();
    let blank_rows = weeks.saturating_sub(rows.len());
    [
        format!("{title:^MONTH_WIDTH$}"),
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

/// Las horas del año y su media mensual, que cierran la vista de año (RN-24).
fn year_hours_lines(summary: &Summary) -> [String; 2] {
    let total = decimal(summary.working_hours);
    [
        format!("Horas laborables del año: {total} h"),
        format!(
            "Media mensual: {} h ({total} h / {} meses)",
            decimal(summary.monthly_average_hours()),
            Month::ALL.len()
        ),
    ]
}

fn month(view: &MonthView, summary: &Summary, marker: Marker) -> String {
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
        month_grid(
            view,
            &format!("{} {}", month_name(view.month), view.year),
            0,
            marker,
        )
        .into_iter()
        .chain(marks)
        .chain(iter::once(String::new()))
        .chain(default_vacation_note(&view.days))
        .chain(summary.month(view.month).map(month_hours_line))
        .chain([legend(marker), vacation_line(summary)]),
    )
}

/// Los meses de tres en tres, dentro de una rejilla gris punteada con marco (RN-21).
fn year(months: &[MonthView], summary: &Summary, marker: Marker) -> String {
    let row_count = months.chunks(MONTHS_PER_ROW).len();
    let columns = months.len().min(MONTHS_PER_ROW);
    // RN-23: la franja con el año, dentro del marco y sobre la primera fila de meses.
    let band_width = columns * MONTH_WIDTH + columns.saturating_sub(1) * 3;
    let title = format!("Calendario laboral {}", summary.year);
    let band =
        format!("{title:^band_width$}").replacen(&title, &styled(Style::new().bold(), &title), 1);
    let top = [
        grid_rule(columns, ['┌', '┈', '┐']),
        grid_row(&[band]),
        grid_rule(columns, ['├', '┬', '┤']),
    ];
    let rows = months
        .chunks(MONTHS_PER_ROW)
        .enumerate()
        .flat_map(|(position, row)| {
            let grids: Vec<Vec<String>> = row
                .iter()
                .map(|view| {
                    month_grid(view, month_name(view.month), MAX_WEEKS, marker)
                        .into_iter()
                        .chain(iter::once(month_hours_cell(summary, view.month)))
                        .collect()
                })
                .collect();
            let height = grids.first().map_or(0, Vec::len);
            let corners = if position + 1 < row_count {
                ['├', '┼', '┤']
            } else {
                ['└', '┴', '┘']
            };
            (0..height)
                .map(move |index| {
                    grid_row(
                        &grids
                            .iter()
                            .filter_map(|grid| grid.get(index).cloned())
                            .collect::<Vec<_>>(),
                    )
                })
                .chain(iter::once(grid_rule(row.len(), corners)))
        });
    let all_days: Vec<ResolvedDay> = months.iter().flat_map(|view| view.days.clone()).collect();
    lines(
        top.into_iter()
            .chain(rows)
            .chain(iter::once(String::new()))
            .chain(default_vacation_note(&all_days))
            .chain([legend(marker), vacation_line(summary)])
            .chain(year_hours_lines(summary)),
    )
}

fn summary(summary: &Summary, marker: Marker) -> String {
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

fn day(day: &ResolvedDay, marker: Marker) -> String {
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

/// El texto del resultado de una consulta, terminado en salto de línea. `marker` decide cómo se
/// identifica el tipo de cada día (RN-20).
pub fn report(report: &Report, marker: Marker) -> String {
    match report {
        Report::Marks(marks) if marks.is_empty() => lines(iter::once("No hay marcas.".to_owned())),
        Report::Marks(marks) => lines(marks.iter().map(mark_line)),
        Report::Day(resolved) => day(resolved, marker),
        Report::Month(view, totals) => month(view, totals, marker),
        Report::Year(months, totals) => year(months, totals, marker),
        Report::Summary(totals) => summary(totals, marker),
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
    use crate::domain::{Calendar, DateRange, MarkId, MarkKind, Month, MonthExt, Shift};

    fn plain(text: &str) -> String {
        anstream::adapter::strip_str(text).to_string()
    }

    fn shown(calendar: Calendar, command: Command) -> String {
        match execute(calendar, command) {
            Ok(Outcome::Report(result)) => plain(&report(&result, Marker::Letter)),
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
            "               Junio 2026",
            "Lu    Ma    Mi    Ju    Vi    Sá    Do",
            " 1 L   2 L   3 L   4 L   5 V   6 S   7 S",
            " 8 V   9 V  10 V  11 V  12 V  13 S  14 S",
            "15 L  16 L  17 L  18 L  19 R  20 S  21 S",
            "22 L  23 L  24 L  25 L  26 R  27 S  28 S",
            "29 L  30 L",
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
        assert_eq!(
            lines.get(2),
            Some(&"                                     1 F"),
            "{text}"
        );
        assert_eq!(
            lines.get(3),
            Some(&" 2 L   3 L   4 L   5 L   6 R   7 S   8 S"),
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
        let colored = match execute(calendar, Command::Year(2026)) {
            Ok(Outcome::Report(result)) => report(&result, Marker::Bar),
            other => format!("{other:?}"),
        };
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
        .for_each(|(text, line)| assert!(text.lines().any(|l| l == line), "{line}\n{text}"));
        assert!(
            bars.contains("Leyenda: ▌ Laborable · ▌ Reducido · ▌ Laborable verano"),
            "{bars}"
        );
        assert_eq!(Marker::for_color(true), Marker::Bar);
        assert_eq!(Marker::for_color(false), Marker::Letter);
    }

    #[test]
    fn rn_20_day_and_summary_use_the_marker() {
        let day = match execute(calendar(), Command::Day(date(2026, 6, 5))) {
            Ok(Outcome::Report(result)) => plain(&report(&result, Marker::Bar)),
            other => format!("{other:?}"),
        };
        assert_eq!(
            day,
            "2026-06-05 (viernes) · ▌ Reducido · 6,0 h · jornada reducida de los viernes\n"
        );
        let summary = match execute(calendar(), Command::Summary(2026)) {
            Ok(Outcome::Report(result)) => plain(&report(&result, Marker::Bar)),
            other => format!("{other:?}"),
        };
        assert!(
            summary.contains("\n  ▌ Fin de semana     104\n"),
            "{summary}"
        );
    }

    #[test]
    fn rf_07_rf_14_year_shows_three_months_per_row_with_their_hours() {
        let text = shown(
            marked(&[(MarkKind::Vacation, "2026-12-24", "2026-12-24")]),
            Command::Year(2026),
        );
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines.get(3),
            Some(
                &"┊                  Enero                   ┊                 Febrero                  ┊                  Marzo                   ┊"
            )
        );
        assert_eq!(
            lines.get(4),
            Some(
                &"┊ Lu    Ma    Mi    Ju    Vi    Sá    Do   ┊ Lu    Ma    Mi    Ju    Vi    Sá    Do   ┊ Lu    Ma    Mi    Ju    Vi    Sá    Do   ┊"
            )
        );
        assert_eq!(
            lines.get(5),
            Some(
                &"┊                    1 L   2 R   3 S   4 S ┊                                      1 S ┊                                      1 S ┊"
            )
        );
        // El marco superior (RN-21) y la franja del año con su línea (RN-23); cuatro filas de tres
        // meses, de 8 líneas, la de horas (RF-14) y la línea de la rejilla debajo; una en blanco
        // y, después, leyenda, cupo, horas del año y media mensual (RN-24).
        assert_eq!(lines.len(), 3 + 4 * 10 + 5, "{text}");
        let hours = format!(
            "┊ {a:^40} ┊ {b:^40} ┊ {c:^40} ┊",
            a = "174,5 h · 22 días",
            b = "160,0 h · 20 días",
            c = "177,0 h · 22 días"
        );
        assert_eq!(lines.get(11), Some(&hours.as_str()));
        assert_eq!(lines.get(44), Some(&LEGEND));
        assert_eq!(lines.get(45), Some(&"Vacaciones restantes: 24 / 25"));
        assert_eq!(
            lines.get(33),
            Some(
                &"┊                 Octubre                  ┊                Noviembre                 ┊                Diciembre                 ┊"
            )
        );
    }

    #[test]
    fn rn_21_year_frames_the_months_in_a_grey_dotted_grid() {
        let colored = match execute(calendar(), Command::Year(2026)) {
            Ok(Outcome::Report(result)) => report(&result, Marker::Bar),
            other => format!("{other:?}"),
        };
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
    fn rn_24_year_ends_with_the_year_hours_and_the_monthly_average() {
        let text = shown(calendar(), Command::Year(2026));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines.get(lines.len().saturating_sub(3)..),
            Some(
                &[
                    "Vacaciones restantes: 0 / 25",
                    "Horas laborables del año: 1888,5 h",
                    "Media mensual: 157,38 h (1888,5 h / 12 meses)",
                ][..]
            ),
            "{text}"
        );
    }

    #[test]
    fn rn_23_year_shows_the_year_once_in_a_band_above_the_months() {
        let colored = match execute(calendar(), Command::Year(2026)) {
            Ok(Outcome::Report(result)) => report(&result, Marker::Bar),
            other => format!("{other:?}"),
        };
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
            },
        );
        assert!(text.contains(" 1 V   2 V   3 V   4 S   5 S"), "{text}");
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
