//! La API para la TUI (requisitos de escrutinio, «Servicio»): `GET /snapshots` devuelve en JSON los
//! snapshots de las circunscripciones con mesas escrutadas, ordenados por código de provincia.

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::application::SnapshotSource;

/// El texto de la respuesta 503 cuando no se pueden leer los snapshots.
pub const UNAVAILABLE: &str = "no se pueden leer los snapshots";

pub fn router<S: SnapshotSource + Send + Sync + 'static>(source: Arc<S>) -> Router {
    Router::new()
        .route("/snapshots", get(snapshots::<S>))
        .with_state(source)
}

async fn snapshots<S: SnapshotSource + Send + Sync + 'static>(
    State(source): State<Arc<S>>,
) -> Response {
    match source.snapshots().await {
        Ok(snapshots) => Json(snapshots).into_response(),
        Err(error) => {
            eprintln!("error: {UNAVAILABLE}: {error}");
            (StatusCode::SERVICE_UNAVAILABLE, UNAVAILABLE).into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use escrutinio_contrato::{CandidacyTotal, ProvinceSnapshot};
    use tower::ServiceExt;

    use super::*;
    use crate::application::fakes::FakeSnapshots;

    async fn get(source: FakeSnapshots, uri: &str) -> (StatusCode, String) {
        let response = router(Arc::new(source))
            .oneshot(Request::get(uri).body(Body::empty()).unwrap_or_default())
            .await
            .unwrap_or_else(|never| match never {});
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap_or_default();
        (status, String::from_utf8_lossy(&body).into_owned())
    }

    fn snapshot() -> ProvinceSnapshot {
        ProvinceSnapshot {
            province: "51".to_owned(),
            stations: 1,
            census: 100,
            voters: 53,
            blank: 1,
            invalid: 2,
            votes: vec![CandidacyTotal {
                candidacy: 1,
                acronym: "AAA".to_owned(),
                votes: 50,
            }],
        }
    }

    #[tokio::test]
    async fn serves_the_snapshots_as_json() {
        assert_eq!(
            get(FakeSnapshots(Ok(vec![snapshot()])), "/snapshots").await,
            (
                StatusCode::OK,
                r#"[{"province":"51","stations":1,"census":100,"voters":53,"blank":1,"invalid":2,"votes":[{"candidacy":1,"acronym":"AAA","votes":50}]}]"#.to_owned()
            )
        );
        assert_eq!(
            get(FakeSnapshots(Ok(vec![])), "/snapshots").await,
            (StatusCode::OK, "[]".to_owned())
        );
    }

    #[tokio::test]
    async fn answers_503_when_postgres_fails_and_404_elsewhere() {
        assert_eq!(UNAVAILABLE, "no se pueden leer los snapshots");
        assert_eq!(
            get(FakeSnapshots(Err(())), "/snapshots").await,
            (StatusCode::SERVICE_UNAVAILABLE, UNAVAILABLE.to_owned())
        );
        assert_eq!(
            get(FakeSnapshots(Ok(vec![])), "/otra").await.0,
            StatusCode::NOT_FOUND
        );
    }
}
