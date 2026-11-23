//! Puro, sin E/S: las mesas y las comprobaciones del fichero (`station`), la validación de cada
//! mesa (`validation`), cuándo llega cada una (`schedule`) y cómo viaja en Kafka (`message`).

pub mod message;
pub mod schedule;
pub mod station;
pub mod validation;
