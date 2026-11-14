//! La tabla de embalses de Madrid, debajo de la próxima ausencia en la ventana (RN-31).

use anstyle::RgbColor;

use super::style::styled;
use super::*;
use crate::adapters::feed::Reading;
use crate::adapters::reservoirs::ReservoirTable;
use crate::domain::ReservoirLevel;

/// El título de la sección.
pub const RESERVOIRS_TITLE: &str = "Embalses de Madrid";
/// Lo que dice una fila cuya petición ha fallado.
pub const UNAVAILABLE: &str = "no disponible";
/// La luz encendida.
pub const LIGHT_ON: &str = "●";
/// La luz apagada: la fila espera o está en la mitad oscura del parpadeo.
pub const LIGHT_OFF: &str = "○";

/// La línea de la fuente, con la fecha del dato más reciente.
pub fn source_line(date: NaiveDate) -> String {
    format!("Fuente: MITECO, semana del {}", date.format("%d-%m-%Y"))
}
/// La atribución que pide la API, debajo de la fuente.
pub const PROCESSED_BY: &str = "Datos procesados por MapasDeEmbalses.es";

/// El naranja de pr-checker para lo que está en curso.
const ORANGE: Color = Color::Rgb(RgbColor(255, 165, 0));
/// El ancho de los nombres, también en la tabla de mercados (RN-32).
pub(super) const NAME_WIDTH: usize = 15;
const HM3_WIDTH: usize = 9;
const PERCENT_WIDTH: usize = 6;
const AVERAGE_HEADER: &str = "Media 10 años";

/// Las líneas de la sección: el título, la cabecera, una fila por embalse y, si ha llegado algún
/// dato, la fuente.
pub(super) fn reservoir_table(table: &ReservoirTable) -> Vec<String> {
    let header = format!(
        "  {:<NAME_WIDTH$} {:>HM3_WIDTH$} {:>HM3_WIDTH$} {:>PERCENT_WIDTH$} {AVERAGE_HEADER}",
        "Embalse", "Capacidad", "Volumen", "%"
    );
    let rows = table.rows().iter().map(|&(reservoir, reading)| {
        let light = light(&reading, table.lit());
        match reading {
            Reading::Waiting | Reading::Fetching => format!("{light} {}", reservoir.name),
            Reading::Failed => format!("{light} {:<NAME_WIDTH$} {UNAVAILABLE}", reservoir.name),
            Reading::Loaded(level) => {
                format!("{light} {:<NAME_WIDTH$} {}", reservoir.name, values(&level))
            }
        }
    });
    iter::once(styled(Style::new().bold(), RESERVOIRS_TITLE))
        .chain(iter::once(header))
        .chain(rows)
        .chain(
            table
                .latest_date()
                .into_iter()
                .flat_map(|date| [source_line(date), PROCESSED_BY.to_owned()]),
        )
        .collect()
}

/// La capacidad, el volumen, el porcentaje y la media de 10 años de una fila.
fn values(level: &ReservoirLevel) -> String {
    let hm3 = |value: f64| format!("{value:.0} hm³");
    let average = level
        .ten_year_percent
        .map_or_else(|| "—".to_owned(), percent);
    format!(
        "{:>HM3_WIDTH$} {:>HM3_WIDTH$} {:>PERCENT_WIDTH$} {average:>width$}",
        hm3(level.latest.capacity_hm3),
        hm3(level.latest.volume_hm3),
        percent(level.latest.percent),
        width = AVERAGE_HEADER.chars().count()
    )
}

/// `63,6 %`.
fn percent(value: f64) -> String {
    format!("{value:.1} %").replace('.', ",")
}

/// La luz de una fila: gris mientras espera, naranja que parpadea mientras se pide, y verde o
/// roja según la respuesta, como en pr-checker. La usan también los mercados (RN-32).
pub(super) fn light<T>(reading: &Reading<T>, lit: bool) -> String {
    let (symbol, color) = match reading {
        Reading::Waiting => (LIGHT_OFF, Color::Ansi(AnsiColor::BrightBlack)),
        Reading::Fetching if lit => (LIGHT_ON, ORANGE),
        Reading::Fetching => (LIGHT_OFF, Color::Ansi(AnsiColor::BrightBlack)),
        Reading::Loaded(_) => (LIGHT_ON, Color::Ansi(AnsiColor::Green)),
        Reading::Failed => (LIGHT_ON, Color::Ansi(AnsiColor::Red)),
    };
    styled(Style::new().fg_color(Some(color)), symbol)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::reservoirs::Fetched;
    use crate::adapters::reservoirs::fixtures::{RESERVOIRS, atazar};
    use crate::domain::fixtures::date;

    fn plain(lines: &[String]) -> Vec<String> {
        lines
            .iter()
            .map(|line| anstream::adapter::strip_str(line).to_string())
            .collect()
    }

    #[test]
    fn rn_31_texts_are_fixed() {
        assert_eq!(RESERVOIRS_TITLE, "Embalses de Madrid");
        assert_eq!(UNAVAILABLE, "no disponible");
        assert_eq!((LIGHT_ON, LIGHT_OFF), ("●", "○"));
        assert_eq!(
            source_line(date(2026, 11, 10)),
            "Fuente: MITECO, semana del 10-11-2026"
        );
        assert_eq!(PROCESSED_BY, "Datos procesados por MapasDeEmbalses.es");
    }

    #[test]
    fn rn_31_rows_fill_in_as_their_answers_arrive() {
        let table = ReservoirTable::new(&RESERVOIRS).fetched(Fetched {
            index: 0,
            level: Some(atazar()),
        });
        assert_eq!(
            plain(&reservoir_table(&table)),
            [
                "Embalses de Madrid",
                "  Embalse         Capacidad   Volumen      % Media 10 años",
                "● El Atazar         426 hm³   271 hm³ 63,6 %        66,2 %",
                "● El Pardo",
                "○ La Jarosa",
                &source_line(date(2026, 11, 10)),
                PROCESSED_BY,
            ]
        );
        let no_average = ReservoirLevel {
            ten_year_percent: None,
            ..atazar()
        };
        let done = table
            .fetched(Fetched {
                index: 1,
                level: None,
            })
            .fetched(Fetched {
                index: 2,
                level: Some(no_average),
            });
        assert_eq!(
            plain(&reservoir_table(&done))[3..5],
            [
                "● El Pardo        no disponible",
                "● La Jarosa         426 hm³   271 hm³ 63,6 %             —",
            ]
        );
        assert_eq!(
            plain(&reservoir_table(&ReservoirTable::new(&RESERVOIRS))).len(),
            5,
            "without any answer there are no source lines"
        );
    }

    #[test]
    fn rn_31_lights_use_the_colors_of_pr_checker() {
        let orange = styled(Style::new().fg_color(Some(ORANGE)), LIGHT_ON);
        let grey = |symbol| {
            styled(
                Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightBlack))),
                symbol,
            )
        };
        let green = styled(
            Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green))),
            LIGHT_ON,
        );
        let red = styled(
            Style::new().fg_color(Some(Color::Ansi(AnsiColor::Red))),
            LIGHT_ON,
        );
        [
            ("waiting", Reading::Waiting, true, grey(LIGHT_OFF)),
            ("fetching, lit", Reading::Fetching, true, orange),
            ("fetching, dark", Reading::Fetching, false, grey(LIGHT_OFF)),
            ("loaded", Reading::Loaded(atazar()), false, green),
            ("failed", Reading::Failed, true, red),
        ]
        .into_iter()
        .for_each(|(case, reading, lit, expected)| {
            assert_eq!(light(&reading, lit), expected, "{case}");
        });
        assert_eq!(orange_rgb(), "\u{1b}[38;2;255;165;0m");
    }

    fn orange_rgb() -> String {
        Style::new().fg_color(Some(ORANGE)).render().to_string()
    }
}
