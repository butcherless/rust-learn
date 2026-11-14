//! Las vistas de mes y de año: la rejilla de días, la franja del año y sus totales (RF-06, RF-07,
//! RF-14, RN-21, RN-23, RN-24).

use std::iter;

use super::next_absence::next_absence;
use super::style::{paint_grid, styled};
use super::vacation_grid::vacation_grid;
use super::*;
use crate::domain::Absence;

/// Ancho visible de una celda de día: el número a dos posiciones, un espacio y el símbolo.
const CELL_WIDTH: usize = 4;
/// Separación entre dos días, mayor que la del número y su símbolo para que se vea de qué
/// número es cada símbolo (RN-20).
const CELL_GAP: &str = "  ";
/// Ancho visible de un mes en la cuadrícula: siete celdas y seis separaciones.
pub(super) const MONTH_WIDTH: usize = 7 * CELL_WIDTH + 6 * CELL_GAP.len();
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

fn legend(marker: Marker) -> String {
    let items: Vec<String> = DayKind::ALL
        .into_iter()
        .map(|kind| format!("{} {}", marker.symbol(kind), kind.label()))
        .collect();
    format!("Leyenda: {}", items.join(" · "))
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

/// Recorre los días solo hasta el primero de vacaciones por defecto, sin copiarlos.
fn default_vacation_note<'a>(days: impl IntoIterator<Item = &'a ResolvedDay>) -> Option<String> {
    days.into_iter()
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

/// Las horas del año y su media mensual, que cierran la vista de año (RN-24).
fn year_hours_lines(summary: &Summary) -> [String; 2] {
    [
        format!(
            "Horas laborables del año: {} h",
            decimal(summary.working_hours)
        ),
        format!(
            // `Display` de `f64` escribe 8.0 como «8».
            "Media mensual: {} h ({} días de {REFERENCE_DAY_HOURS} h)",
            decimal(summary.monthly_average_hours()),
            decimal(summary.monthly_average_days()),
        ),
    ]
}

/// Separación entre el calendario, del mes o del año, y la cuadrícula de vacaciones (RN-27).
const VACATION_GAP: &str = "    ";

/// La columna de la derecha: la cuadrícula de vacaciones y, debajo, tras al menos una línea en
/// blanco, la próxima ausencia desde la línea `absence_from` (RN-27, RN-30).
fn right_column(
    summary: &Summary,
    absence: Option<&Absence>,
    absence_from: usize,
    marker: Marker,
) -> Vec<String> {
    let grid = vacation_grid(summary, marker);
    let gap = absence_from.max(grid.len() + 1) - grid.len();
    grid.into_iter()
        .chain(iter::repeat_n(String::new(), gap))
        .chain(next_absence(absence, marker))
        .collect()
}

/// Las líneas de una fila de meses de la vista de año con la línea de la rejilla de debajo: el
/// título, los días de la semana, las semanas y las horas.
const YEAR_ROW_LINES: usize = 2 + MAX_WEEKS + 1 + 1;

/// `lines` con las líneas de `vacations` a la derecha, desde la línea `from` (RN-27).
fn beside_vacations(
    lines: impl Iterator<Item = String>,
    from: usize,
    vacations: Vec<String>,
) -> impl Iterator<Item = String> {
    let vacation_lines = iter::repeat_n(None, from)
        .chain(vacations.into_iter().map(Some))
        .chain(iter::repeat(None));
    lines
        .zip(vacation_lines)
        .map(|(line, vacations)| match vacations {
            Some(vacations) => format!("{line}{VACATION_GAP}{vacations}"),
            None => line,
        })
}

pub(super) fn month(
    view: &MonthView,
    summary: &Summary,
    absence: Option<&Absence>,
    marker: Marker,
) -> String {
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
    // RN-27, RN-30: el mes se alarga con semanas en blanco si la columna de la derecha es más
    // alta, para que no quede junto a las marcas.
    let vacations = right_column(summary, absence, 0, marker);
    let weeks = vacations.len().saturating_sub(2);
    let grid = beside_vacations(
        month_grid(
            view,
            &format!("{} {}", month_name(view.month), view.year),
            weeks,
            marker,
        )
        .into_iter(),
        0,
        vacations,
    );
    lines(
        grid.chain(marks)
            .chain(iter::once(String::new()))
            .chain(default_vacation_note(&view.days))
            .chain(summary.month(view.month).map(month_hours_line))
            .chain([
                legend(marker),
                vacation_line(summary),
                working_days_line(summary),
            ]),
    )
}

/// Los meses de tres en tres, dentro de una rejilla gris punteada con marco (RN-21), con la
/// cuadrícula de vacaciones y la próxima ausencia a la derecha de la primera fila (RN-27, RN-30).
pub(super) fn year(
    months: &[MonthView],
    summary: &Summary,
    absence: Option<&Absence>,
    marker: Marker,
) -> String {
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
    // RN-27: la cuadrícula de vacaciones empieza en la línea del título de la primera fila.
    let from = top.len();
    lines(
        beside_vacations(
            top.into_iter().chain(rows),
            from,
            // RN-30: la próxima ausencia empieza en el título de la segunda fila de meses.
            right_column(summary, absence, YEAR_ROW_LINES, marker),
        )
        .chain(iter::once(String::new()))
        .chain(default_vacation_note(
            months.iter().flat_map(|view| &view.days),
        ))
        .chain([
            legend(marker),
            vacation_line(summary),
            working_days_line(summary),
        ])
        .chain(year_hours_lines(summary)),
    )
}
