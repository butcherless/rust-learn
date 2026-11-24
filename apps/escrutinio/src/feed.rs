//! El escrutinio del servicio (RN-11, RN-13): `fetch` pide `GET /snapshots` y `update` lo pasa a lo
//! que pinta la TUI. No calcula nada: copia los votantes, las mesas N'/N' y el «completo» que el
//! servicio ya trae, por circunscripción y para el total.
//!
//! El cliente es el bloqueante de reqwest, porque la TUI no es async. Los tests con wiremock lo
//! ejecutan en `spawn_blocking`: no puede usarse dentro de un runtime async.

use std::time::Duration;

use escrutinio_contrato::Snapshots;
use thiserror::Error;

use crate::domain::{Progress, Stations, Update};

/// Cada cuánto se piden los snapshots (RN-11).
pub const REFRESH: Duration = Duration::from_secs(10);
/// Cuánto se espera al servicio: menos que `REFRESH`, para no solapar peticiones.
const TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("no se pudo crear el cliente HTTP")]
    Client(#[source] reqwest::Error),
    #[error("no se pudo pedir {0}")]
    Request(String, #[source] reqwest::Error),
}

/// Un cliente bloqueante con el tiempo máximo. No debe crearse ni usarse dentro de un runtime
/// async.
pub fn client() -> Result<reqwest::blocking::Client, FetchError> {
    reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(FetchError::Client)
}

/// Los snapshots de `service` (su URL base, como `http://localhost:8080`).
pub fn fetch(client: &reqwest::blocking::Client, service: &str) -> Result<Snapshots, FetchError> {
    let url = format!("{}/snapshots", service.trim_end_matches('/'));
    client
        .get(&url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .and_then(reqwest::blocking::Response::json)
        .map_err(|error| FetchError::Request(url, error))
}

/// Lo que trae el servicio, tal cual, para pintarlo.
pub fn update(snapshots: &Snapshots) -> Update {
    Update {
        provinces: snapshots
            .provinces
            .iter()
            .map(|province| {
                let progress = Progress {
                    voters: province.voters,
                    stations: Stations {
                        counted: province.stations,
                        possible: province.possible,
                    },
                    complete: province.complete,
                };
                (province.province.clone(), progress)
            })
            .collect(),
        total: Stations {
            counted: snapshots.total.stations,
            possible: snapshots.total.possible,
        },
        complete: snapshots.total.complete,
    }
}

#[cfg(test)]
pub mod fixtures {
    use escrutinio_contrato::{ProvinceSnapshot, Snapshots, Total};

    /// El snapshot de `province` con `voters` votantes en 1 mesa de 2 posibles.
    pub fn snapshot(province: &str, voters: u64) -> ProvinceSnapshot {
        with_stations(province, voters, 1, 2)
    }

    /// El snapshot de `province` con `voters` votantes en `stations` mesas de `possible`; completo
    /// si están todas.
    pub fn with_stations(
        province: &str,
        voters: u64,
        stations: u32,
        possible: u32,
    ) -> ProvinceSnapshot {
        ProvinceSnapshot {
            province: province.to_owned(),
            stations,
            possible,
            rejected: 0,
            complete: stations >= possible,
            census: voters * 2,
            voters,
            blank: 0,
            invalid: 0,
            votes: vec![],
        }
    }

    /// Las circunscripciones con su total, sumado como lo haría el servicio.
    pub fn snapshots(provinces: Vec<ProvinceSnapshot>) -> Snapshots {
        Snapshots {
            total: Total {
                stations: provinces.iter().map(|p| p.stations).sum(),
                possible: provinces.iter().map(|p| p.possible).sum(),
                rejected: 0,
                complete: !provinces.is_empty() && provinces.iter().all(|p| p.complete),
                voters: provinces.iter().map(|p| p.voters).sum(),
            },
            provinces,
        }
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::fixtures::{snapshot, snapshots, with_stations};
    use super::*;

    /// `fetch` en un hilo aparte, fuera del runtime del test.
    async fn fetch_from(service: String) -> Result<Snapshots, String> {
        tokio::task::spawn_blocking(move || {
            client()
                .and_then(|client| fetch(&client, &service))
                .map_err(|error| error.to_string())
        })
        .await
        .unwrap_or_else(|error| Err(error.to_string()))
    }

    #[tokio::test]
    async fn fetches_the_snapshots_of_the_service() {
        let server = MockServer::start().await;
        let sent = snapshots(vec![snapshot("28", 100), snapshot("51", 7)]);
        let body = serde_json::to_string(&sent).unwrap_or_default();
        Mock::given(method("GET"))
            .and(path("/snapshots"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
            .mount(&server)
            .await;
        let fetched = fetch_from(format!("{}/", server.uri())).await;
        assert_eq!(fetched, Ok(sent));
    }

    #[tokio::test]
    async fn fails_on_an_error_status_or_a_bad_body() {
        let server = MockServer::start().await;
        Mock::given(path("/snapshots"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(path("/snapshots"))
            .respond_with(ResponseTemplate::new(200).set_body_string("no"))
            .mount(&server)
            .await;
        let expected = format!("no se pudo pedir {}/snapshots", server.uri());
        assert_eq!(fetch_from(server.uri()).await, Err(expected.clone()));
        assert_eq!(fetch_from(server.uri()).await, Err(expected));
    }

    #[test]
    fn copies_what_the_service_calculated_without_adding_anything() {
        let update = update(&snapshots(vec![
            snapshot("28", 100),
            with_stations("51", 7, 96, 96),
        ]));
        let progress = |voters, counted, possible, complete| {
            Some(Progress {
                voters,
                stations: Stations { counted, possible },
                complete,
            })
        };
        assert_eq!(
            update.provinces.get("28").copied(),
            progress(100, 1, 2, false)
        );
        assert_eq!(
            update.provinces.get("51").copied(),
            progress(7, 96, 96, true)
        );
        assert_eq!(update.provinces.len(), 2);
        // El total viene del servicio: aquí, lo que la fixture le puso.
        assert_eq!(
            update.total,
            Stations {
                counted: 97,
                possible: 98
            }
        );
        assert!(!update.complete);
    }
}
