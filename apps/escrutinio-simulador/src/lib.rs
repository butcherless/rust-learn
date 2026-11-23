//! Simulador del escrutinio: publica en Kafka las mesas del Congreso de 2023, cada una en su
//! momento. Las reglas están en `apps/escrutinio/docs/requisitos-escrutinio.html`.
//!
//! Hexagonal ligera, con las dependencias hacia dentro: `adapters → application → domain`.

pub mod adapters;
pub mod application;
pub mod domain;
