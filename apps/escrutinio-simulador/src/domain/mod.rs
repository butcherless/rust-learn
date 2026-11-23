//! Puro, sin E/S: las mesas y las comprobaciones del fichero (`station`), cuándo llega cada una
//! (`schedule`) y su mensaje de Kafka (`message`). La validación de cada mesa y los mensajes son del
//! contrato compartido, `escrutinio-contrato`.

pub mod message;
pub mod schedule;
pub mod station;
