//! Los argumentos de la línea de órdenes y los textos de la consola.

use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;

use crate::domain::station::Summary;

/// Dispara el escrutinio: publica en Kafka las mesas del Congreso de 2023, cada una en su momento.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Args {
    /// Carpeta con los ficheros de Interior 09, 10, 03 y 07 (de 02202307_MESA.zip).
    #[arg(long, value_name = "CARPETA")]
    pub datos: PathBuf,

    /// Servidores de Kafka.
    #[arg(long, value_name = "SERVIDORES", default_value = "localhost:9092")]
    pub kafka: String,

    /// Topic donde se publican las mesas.
    #[arg(long, default_value = "escrutinio.mesas")]
    pub topic: String,

    /// Topic del dead letter, donde van las mesas que no pasan la validación.
    #[arg(long, default_value = "escrutinio.mesas.dlq")]
    pub topic_dlq: String,

    /// Cuánto dura el escrutinio: un número con h, m o s (2h, 90m, 30s).
    #[arg(long, value_name = "DURACIÓN", default_value = "2h", value_parser = parse_duration)]
    pub duracion: Duration,

    /// Semilla del orden de llegada: con la misma, el orden es el mismo.
    #[arg(long, default_value_t = 2023)]
    pub semilla: u64,

    /// Solo lee y comprueba los ficheros, sin publicar.
    #[arg(long)]
    pub solo_comprobar: bool,
}

/// Una duración positiva escrita como un número y una unidad: `2h`, `90m` o `30s`.
pub fn parse_duration(value: &str) -> Result<Duration, String> {
    let invalid =
        || format!("«{value}» no es una duración: usa un número con h, m o s (2h, 90m, 30s)");
    let (number, unit) = value.split_at(value.len().saturating_sub(1));
    let seconds = match unit {
        "h" => 3600,
        "m" => 60,
        "s" => 1,
        _ => return Err(invalid()),
    };
    number
        .parse::<u64>()
        .ok()
        .filter(|&n| n > 0)
        .map(|n| Duration::from_secs(n * seconds))
        .ok_or_else(invalid)
}

/// `h:mm:ss`.
pub fn clock(duration: Duration) -> String {
    let seconds = duration.as_secs();
    format!(
        "{}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

/// Agrupa los miles con puntos: `35.141.198`.
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    digits
        .char_indices()
        .flat_map(|(i, digit)| {
            let dot = i > 0 && (digits.len() - i).is_multiple_of(3);
            dot.then_some('.').into_iter().chain([digit])
        })
        .collect()
}

/// La línea que se muestra tras comprobar los ficheros.
pub fn checked(summary: &Summary) -> String {
    let turnout = summary.voters as f64 * 100.0 / summary.census.max(1) as f64;
    format!(
        "Comprobadas {} mesas en {} circunscripciones: {} de censo y {} votantes ({}).",
        thousands(summary.stations as u64),
        summary.provinces,
        thousands(summary.census),
        thousands(summary.voters),
        format!("{turnout:.2} %").replace('.', ",")
    ) + &match summary.rejected {
        0 => String::new(),
        1 => " 1 no es válida e irá al dead letter.".to_owned(),
        n => format!(
            " {} no son válidas e irán al dead letter.",
            thousands(n as u64)
        ),
    }
}

/// La línea final.
pub fn published(valid: usize, rejected: usize, elapsed: Duration) -> String {
    format!(
        "Publicadas {} mesas y {} en el dead letter en {}.",
        thousands(valid as u64),
        thousands(rejected as u64),
        clock(elapsed)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_durations_in_hours_minutes_and_seconds() {
        [("2h", 7200), ("90m", 5400), ("30s", 30)]
            .into_iter()
            .for_each(|(value, seconds)| {
                assert_eq!(
                    parse_duration(value),
                    Ok(Duration::from_secs(seconds)),
                    "{value}"
                );
            });
    }

    #[test]
    fn rejects_other_durations() {
        ["", "2", "h", "0s", "-1m", "2d", "1.5h", "2 h"]
            .into_iter()
            .for_each(|value| assert!(parse_duration(value).is_err(), "{value:?}"));
        assert_eq!(
            parse_duration("2d"),
            Err("«2d» no es una duración: usa un número con h, m o s (2h, 90m, 30s)".to_owned())
        );
    }

    #[test]
    fn formats_clock_times_and_thousands() {
        assert_eq!(clock(Duration::from_secs(7322)), "2:02:02");
        assert_eq!(clock(Duration::from_millis(999)), "0:00:00");
        [
            (0, "0"),
            (999, "999"),
            (1000, "1.000"),
            (35_141_198, "35.141.198"),
        ]
        .into_iter()
        .for_each(|(n, text)| assert_eq!(thousands(n), text, "{n}"));
    }

    #[test]
    fn describes_the_checked_data() {
        let summary = Summary {
            stations: 60_314,
            rejected: 0,
            provinces: 52,
            census: 35_141_198,
            voters: 24_744_222,
        };
        assert_eq!(
            checked(&summary),
            "Comprobadas 60.314 mesas en 52 circunscripciones: 35.141.198 de censo y 24.744.222 votantes (70,41 %)."
        );
    }

    #[test]
    fn counts_the_stations_for_the_dead_letter() {
        let summary = |rejected| Summary {
            stations: 10,
            rejected,
            provinces: 1,
            census: 100,
            voters: 50,
        };
        [
            (0, "votantes (50,00 %)."),
            (
                1,
                "votantes (50,00 %). 1 no es válida e irá al dead letter.",
            ),
            (
                4,
                "votantes (50,00 %). 4 no son válidas e irán al dead letter.",
            ),
        ]
        .into_iter()
        .for_each(|(rejected, end)| {
            assert!(checked(&summary(rejected)).ends_with(end), "{rejected}")
        });
        assert_eq!(
            published(60_310, 4, Duration::from_secs(300)),
            "Publicadas 60.310 mesas y 4 en el dead letter en 0:05:00."
        );
    }
}
