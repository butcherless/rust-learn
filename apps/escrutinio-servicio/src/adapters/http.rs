//! La API para la TUI (requisitos de escrutinio, «Servicio»): `GET /snapshots` devuelve en JSON las
//! 52 circunscripciones, por código de provincia, y el total de España, con todo calculado.

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::application::{SnapshotSource, snapshots as consult};

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
    match consult(&*source).await {
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
    use escrutinio_contrato::Snapshots;
    use tower::ServiceExt;

    use super::*;
    use crate::application::fakes::FakeSnapshots;
    use crate::domain::snapshot::fixtures::totals;

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

    #[tokio::test]
    async fn serves_the_52_districts_and_the_total_already_calculated() {
        let (status, body) = get(FakeSnapshots(Ok(vec![totals("51", 96, 0)])), "/snapshots").await;
        assert_eq!(status, StatusCode::OK);
        let snapshots: Snapshots = serde_json::from_str(&body).expect("snapshots json");
        assert_eq!(snapshots.provinces.len(), 52);
        let ceuta = snapshots.provinces.iter().find(|p| p.province == "51");
        assert_eq!(
            ceuta.map(|p| (p.stations, p.possible, p.rejected, p.complete)),
            Some((96, 96, 0, true))
        );
        assert_eq!(
            (
                snapshots.total.stations,
                snapshots.total.possible,
                snapshots.total.complete
            ),
            (96, 60_314, false)
        );
        // Los nombres de los campos son el contrato con la TUI.
        assert!(
            body.starts_with(r#"{"provinces":[{"province":"01","stations":0,"possible":"#),
            "{body}"
        );
        assert!(body.ends_with(r#""total":{"stations":96,"possible":60314,"rejected":0,"complete":false,"voters":5088}}"#), "{body}");
    }

    #[tokio::test]
    async fn serves_all_the_districts_with_zeros_before_any_station() {
        let (status, body) = get(FakeSnapshots(Ok(vec![])), "/snapshots").await;
        assert_eq!(status, StatusCode::OK);
        let snapshots: Snapshots = serde_json::from_str(&body).expect("snapshots json");
        assert_eq!(snapshots.provinces.len(), 52);
        assert!(
            snapshots
                .provinces
                .iter()
                .all(|p| p.stations == 0 && !p.complete)
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
