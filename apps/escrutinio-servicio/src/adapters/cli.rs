//! Los argumentos de la línea de órdenes y los textos de la consola.

use clap::{Parser, Subcommand};

use crate::application::{Tally, Wiped};

/// Servicio del escrutinio: consume las mesas de Kafka, las guarda en PostgreSQL y sirve los
/// snapshots por provincia en GET /snapshots.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Servidores de Kafka.
    #[arg(
        long,
        value_name = "SERVIDORES",
        default_value = "localhost:9092",
        global = true
    )]
    pub kafka: String,

    /// Topic de las mesas.
    #[arg(long, default_value = "escrutinio.mesas", global = true)]
    pub topic: String,

    /// Topic del dead letter.
    #[arg(long, default_value = "escrutinio.mesas.dlq", global = true)]
    pub topic_dlq: String,

    /// Grupo de consumidores de Kafka.
    #[arg(long, default_value = "escrutinio-servicio")]
    pub grupo: String,

    /// Conexión con PostgreSQL. La de por defecto es la de compose/escrutinio.yaml.
    #[arg(
        long,
        value_name = "URL",
        default_value = "postgres://escrutinio:escrutinio@localhost:5432/escrutinio",
        global = true
    )]
    pub postgres: String,

    /// Puerto de la API HTTP.
    #[arg(long, default_value_t = 8080)]
    pub puerto: u16,
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum Command {
    /// Borra el escrutinio: los mensajes de los dos topics y las mesas de PostgreSQL, para empezar
    /// otro desde cero. Hazlo con el simulador parado.
    Borrar,
}

/// La línea tras borrar el escrutinio.
pub fn wiped(wiped: &Wiped) -> String {
    format!(
        "Escrutinio borrado: {} mensajes en Kafka y {} mesas en PostgreSQL.",
        wiped.messages, wiped.stations
    )
}

/// Cada cuántos mensajes se muestra el avance.
pub const REPORT_EVERY: usize = 5000;

/// La línea del avance, cada `REPORT_EVERY` mensajes.
pub fn report(tally: &Tally) -> Option<String> {
    tally.total().is_multiple_of(REPORT_EVERY).then(|| {
        format!(
            "{} mensajes: {} mesas registradas, {} repetidas y {} en el dead letter",
            tally.total(),
            tally.recorded,
            tally.duplicates,
            tally.dead_letters
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_every_5000_messages() {
        let tally = |recorded| Tally {
            recorded,
            duplicates: 2,
            dead_letters: 3,
        };
        assert_eq!(report(&tally(4994)), None);
        assert_eq!(
            report(&tally(4995)).as_deref(),
            Some("5000 mensajes: 4995 mesas registradas, 2 repetidas y 3 en el dead letter")
        );
    }

    #[test]
    fn borrar_is_a_subcommand_that_takes_the_same_options() {
        let args = Args::parse_from(["escrutinio-servicio", "borrar", "--topic", "otro"]);
        assert_eq!(
            (args.command, args.topic.as_str()),
            (Some(Command::Borrar), "otro")
        );
        assert_eq!(Args::parse_from(["escrutinio-servicio"]).command, None);
        assert_eq!(
            wiped(&Wiped {
                messages: 60_314,
                stations: 60_310
            }),
            "Escrutinio borrado: 60314 mensajes en Kafka y 60310 mesas en PostgreSQL."
        );
    }

    #[test]
    fn the_defaults_match_the_compose_file() {
        let args = Args::parse_from(["escrutinio-servicio"]);
        assert_eq!(
            (
                args.kafka.as_str(),
                args.topic.as_str(),
                args.topic_dlq.as_str(),
                args.puerto
            ),
            (
                "localhost:9092",
                "escrutinio.mesas",
                "escrutinio.mesas.dlq",
                8080
            )
        );
    }
}
