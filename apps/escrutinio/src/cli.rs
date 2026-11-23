//! Los argumentos de la línea de órdenes.

use clap::Parser;

use crate::domain::DEFAULT_TURNOUT;

/// Escrutinio por provincia: la maqueta de las Elecciones generales España 2026, con votos
/// simulados, o, con --servicio, el escrutinio de 2023 que sirve escrutinio-servicio.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Args {
    /// Participación estimada, en % del censo (1-100).
    #[arg(long, value_name = "PORCENTAJE", default_value_t = DEFAULT_TURNOUT,
          value_parser = clap::value_parser!(u8).range(1..=100))]
    pub participacion: u8,

    /// Milisegundos entre dos avances del escrutinio simulado (mínimo 50).
    #[arg(long, value_name = "MS", default_value_t = 500,
          value_parser = clap::value_parser!(u64).range(50..))]
    pub intervalo: u64,

    /// Semilla de la simulación: con la misma, el escrutinio es el mismo.
    #[arg(long, default_value_t = 2026)]
    pub semilla: u64,

    /// Sigue el escrutinio de 2023 de escrutinio-servicio en esta URL, cada 10 s, en lugar de la
    /// maqueta.
    #[arg(long, value_name = "URL")]
    pub servicio: Option<String>,
}
