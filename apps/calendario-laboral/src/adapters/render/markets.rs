//! La tabla de mercados, debajo de los embalses en la ventana (RN-32).

use super::reservoirs::{NAME_WIDTH, UNAVAILABLE, light};
use super::style::styled;
use super::*;
use crate::adapters::feed::Reading;
use crate::adapters::markets::MarketTable;
use crate::domain::Quote;

/// El título de la sección.
pub const MARKETS_TITLE: &str = "Mercados";
/// La fuente, debajo de la tabla cuando ha llegado algún valor.
pub const MARKETS_SOURCE: &str = "Fuente: Expansión";

const PRICE_WIDTH: usize = 9;
const CHANGE_WIDTH: usize = 9;

/// Las líneas de la sección: el título, la cabecera, una fila por valor y, si ha llegado alguno,
/// la fuente.
pub(super) fn market_table(table: &MarketTable) -> Vec<String> {
    let header = format!(
        "  {:<NAME_WIDTH$} {:>PRICE_WIDTH$} {:>CHANGE_WIDTH$} Fecha",
        "Valor", "Precio", "Variación"
    );
    let rows = table.rows().iter().map(|&(market, reading)| {
        let light = light(&reading, table.lit());
        match reading {
            Reading::Waiting | Reading::Fetching => format!("{light} {}", market.name()),
            Reading::Failed => format!("{light} {:<NAME_WIDTH$} {UNAVAILABLE}", market.name()),
            Reading::Loaded(quote) => {
                format!("{light} {:<NAME_WIDTH$} {}", market.name(), values(&quote))
            }
        }
    });
    iter::once(styled(Style::new().bold(), MARKETS_TITLE))
        .chain(iter::once(header))
        .chain(rows)
        .chain(table.has_quotes().then(|| MARKETS_SOURCE.to_owned()))
        .collect()
}

/// El precio, la variación y la fecha de un valor.
fn values(quote: &Quote) -> String {
    let change = format!("{:+.2} %", quote.change_percent()).replace('.', ",");
    let date = quote.time.map_or_else(
        || quote.date.format("%d-%m-%Y").to_string(),
        |time| format!("{} {}", quote.date.format("%d-%m-%Y"), time.format("%H:%M")),
    );
    format!(
        "{:>PRICE_WIDTH$} {change:>CHANGE_WIDTH$} {date}",
        price(quote.price)
    )
}

/// Dos decimales con coma y punto de miles: `20.020,10`.
fn price(value: f64) -> String {
    let fixed = format!("{:.2}", value.abs());
    let (whole, cents) = fixed.split_once('.').unwrap_or((&fixed, "00"));
    let digits: Vec<char> = whole.chars().collect();
    let grouped = digits
        .rchunks(3)
        .rev()
        .map(|group| group.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(".");
    let sign = if value < 0.0 { "-" } else { "" };
    format!("{sign}{grouped},{cents}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::markets::fixtures::{brent, ibex};
    use crate::domain::Market;

    fn plain(lines: &[String]) -> Vec<String> {
        lines
            .iter()
            .map(|line| anstream::adapter::strip_str(line).to_string())
            .collect()
    }

    #[test]
    fn rn_32_texts_are_fixed() {
        assert_eq!(MARKETS_TITLE, "Mercados");
        assert_eq!(MARKETS_SOURCE, "Fuente: Expansión");
    }

    #[test]
    fn rn_32_prices_have_thousands_and_two_decimals() {
        [
            (20020.1, "20.020,10"),
            (95.9, "95,90"),
            (1234567.891, "1.234.567,89"),
            (0.5, "0,50"),
            (-1500.0, "-1.500,00"),
        ]
        .into_iter()
        .for_each(|(value, expected)| assert_eq!(price(value), expected, "{value}"));
    }

    #[test]
    fn rn_32_rows_show_price_change_and_date_once_the_answer_arrives() {
        let waiting = MarketTable::default();
        assert_eq!(
            plain(&market_table(&waiting)),
            [
                "Mercados",
                "  Valor              Precio Variación Fecha",
                "○ IBEX 35",
                "○ Brent ($)",
            ]
        );
        let loaded = waiting
            .started()
            .fetched(Some(&[(Market::Ibex35, ibex()), (Market::Brent, brent())]));
        assert_eq!(
            plain(&market_table(&loaded))[2..],
            [
                "● IBEX 35         20.020,10   +0,27 % 13-11-2026 17:35",
                "● Brent ($)           95,90   -0,15 % 13-11-2026",
                MARKETS_SOURCE,
            ]
        );
        let failed = MarketTable::default().started().fetched(None);
        assert_eq!(
            plain(&market_table(&failed))[2..],
            [
                "● IBEX 35         no disponible",
                "● Brent ($)       no disponible",
            ]
        );
    }
}
