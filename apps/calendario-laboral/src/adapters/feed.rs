//! Lo común a los datos que la ventana pide a la red (RN-31, RN-32): el cliente HTTP y el estado
//! de cada fila mientras llega su respuesta.

use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(10);
/// El `User-Agent` de todas las peticiones.
pub const USER_AGENT: &str = concat!("calendario-laboral/", env!("CARGO_PKG_VERSION"));

/// Un cliente bloqueante con el tiempo máximo y el `User-Agent` de la aplicación. No debe crearse
/// ni usarse dentro de un runtime async.
pub fn http_client() -> reqwest::Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
}

/// El estado de una fila que espera un `T` de la red.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reading<T> {
    /// Aún no se ha pedido.
    Waiting,
    /// Se está pidiendo.
    Fetching,
    /// Ha llegado.
    Loaded(T),
    /// Ha fallado.
    Failed,
}

impl<T> Reading<T> {
    /// Si aún falta la respuesta.
    pub const fn is_pending(&self) -> bool {
        match self {
            Self::Waiting | Self::Fetching => true,
            Self::Loaded(_) | Self::Failed => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rn_31_rn_32_a_row_is_pending_until_its_answer_arrives() {
        [
            ("waiting", Reading::Waiting, true),
            ("fetching", Reading::Fetching, true),
            ("loaded", Reading::Loaded(1), false),
            ("failed", Reading::Failed, false),
        ]
        .into_iter()
        .for_each(|(case, reading, expected)| {
            assert_eq!(reading.is_pending(), expected, "{case}");
        });
        assert_eq!(
            USER_AGENT,
            concat!("calendario-laboral/", env!("CARGO_PKG_VERSION"))
        );
    }
}
