//! Servicio del escrutinio: consume las mesas de Kafka, las guarda en PostgreSQL y sirve los
//! snapshots por provincia a la TUI. Las reglas están en
//! `apps/escrutinio/docs/requisitos-escrutinio.html`.
//!
//! Hexagonal ligera, con las dependencias hacia dentro: `adapters → application → domain`.

pub mod adapters;
pub mod application;
pub mod domain;
