//! Calendario laboral: el `Calendar` de uno o varios años, gestionado desde la consola.
//!
//! Capas, con las dependencias hacia dentro: [`adapters`] → [`application`] → [`domain`].

pub mod adapters;
pub mod application;
pub mod domain;
