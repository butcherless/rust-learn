//! El escrutinio del servicio (RN-11): `fetch` pide `GET /snapshots` y `counted` da los votantes
//! y las mesas escrutadas de cada circunscripción, por su código de provincia.
//!
//! El cliente es el bloqueante de reqwest, porque la TUI no es async. Los tests con wiremock lo
//! ejecutan en `spawn_blocking`: no puede usarse dentro de un runtime async.

use std::collections::BTreeMap;
use std::time::Duration;

use escrutinio_contrato::ProvinceSnapshot;
use thiserror::Error;

use crate::domain::Progress;

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
pub fn fetch(
    client: &reqwest::blocking::Client,
    service: &str,
) -> Result<Vec<ProvinceSnapshot>, FetchError> {
    let url = format!("{}/snapshots", service.trim_end_matches('/'));
    client
        .get(&url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .and_then(reqwest::blocking::Response::json)
        .map_err(|error| FetchError::Request(url, error))
}

/// Los votantes y las mesas escrutadas de cada circunscripción, por su código de provincia.
pub fn counted(snapshots: &[ProvinceSnapshot]) -> BTreeMap<String, Progress> {
    snapshots
        .iter()
        .map(|snapshot| {
            let progress = Progress {
                voters: snapshot.voters,
                stations: snapshot.stations,
            };
            (snapshot.province.clone(), progress)
        })
        .collect()
}

#[cfg(test)]
pub mod fixtures {
    use escrutinio_contrato::ProvinceSnapshot;

    /// El snapshot de `province` con `voters` votantes en una mesa.
    pub fn snapshot(province: &str, voters: u64) -> ProvinceSnapshot {
        with_stations(province, voters, 1)
    }

    /// El snapshot de `province` con `voters` votantes en `stations` mesas.
    pub fn with_stations(province: &str, voters: u64, stations: u32) -> ProvinceSnapshot {
        ProvinceSnapshot {
            province: province.to_owned(),
            stations,
            census: voters * 2,
            voters,
            blank: 0,
            invalid: 0,
            votes: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::fixtures::snapshot;
    use super::*;

    /// `fetch` en un hilo aparte, fuera del runtime del test.
    async fn fetch_from(service: String) -> Result<Vec<ProvinceSnapshot>, String> {
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
        let body =
            serde_json::to_string(&[snapshot("28", 100), snapshot("51", 7)]).unwrap_or_default();
        Mock::given(method("GET"))
            .and(path("/snapshots"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
            .mount(&server)
            .await;
        let fetched = fetch_from(format!("{}/", server.uri())).await;
        assert_eq!(fetched, Ok(vec![snapshot("28", 100), snapshot("51", 7)]));
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
    fn counts_the_voters_by_province_code() {
        let counted = counted(&[snapshot("28", 100), snapshot("51", 7)]);
        let progress = |voters| {
            Some(Progress {
                voters,
                stations: 1,
            })
        };
        assert_eq!(counted.get("28").copied(), progress(100));
        assert_eq!(counted.get("51").copied(), progress(7));
        assert_eq!(counted.len(), 2);
    }
}
