//! Errores de las reglas del dominio.

use chrono::NaiveDate;
use thiserror::Error;

use crate::domain::MarkId;

/// Se ha incumplido una regla del calendario. Los mensajes se muestran al usuario tal cual.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum DomainError {
    /// RN-01: el texto no es una fecha ISO, o la fecha no existe.
    #[error("fecha no válida: «{0}»; usa el formato AAAA-MM-DD con una fecha que exista")]
    InvalidDate(String),
    /// RN-03: el rango acaba antes de empezar.
    #[error("rango no válido: {from} es posterior a {to}")]
    ReversedRange {
        /// Primer día del rango.
        from: NaiveDate,
        /// Último día del rango.
        to: NaiveDate,
    },
    /// RN-02: el rango abarca dos años.
    #[error("el rango {from} → {to} cruza de año; márcalo con un rango por año")]
    CrossesYear {
        /// Primer día del rango.
        from: NaiveDate,
        /// Último día del rango.
        to: NaiveDate,
    },
    /// El año no se puede escribir como año ISO de cuatro dígitos.
    #[error("año no válido: {0}; debe estar entre 1 y 9999")]
    InvalidYear(i32),
    /// RN-02: el año no se ha añadido al calendario.
    #[error("el año {0} no existe en el calendario; añádelo con «calendario add-year {0}»")]
    UnknownYear(i32),
    /// El año ya está en el calendario.
    #[error("el año {0} ya existe en el calendario")]
    DuplicateYear(i32),
    /// Ningún `Mark` tiene este id.
    #[error("no existe ninguna marca con id {0}")]
    UnknownMark(MarkId),
    /// RF-18: el `Mark` modificado queda idéntico a otro (RN-06).
    #[error("ya existe una marca idéntica (#{0})")]
    DuplicateMark(MarkId),
    /// Dos `Mark` comparten id.
    #[error("hay más de una marca con id {0}")]
    DuplicateMarkId(MarkId),
    /// RN-17: las horas de un `Shift` son negativas, mayores de 24 o no son un número.
    #[error("horas no válidas: {0}; deben estar entre 0 y 24")]
    InvalidHours(f64),
}
