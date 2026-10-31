//! `GithubClient` against a local mock server. No real network is used.

use rust_learn::adapter::github::{GithubClient, GithubToken};
use rust_learn::domain::error::SourceError;
use rust_learn::domain::model::{PrNumber, PrState, PrTitle, Project, PullRequest};
use rust_learn::domain::port::PullRequestSource;
use serde_json::json;
use wiremock::matchers::{header, header_exists, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PULLS_PATH: &str = "/repos/acme/widgets/pulls";

fn project() -> Project {
    "acme/widgets".parse().unwrap()
}

/// Fetches the project from a server that always answers with `response`.
async fn fetch(response: ResponseTemplate) -> Result<Vec<PullRequest>, SourceError> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(PULLS_PATH))
        .respond_with(response)
        .mount(&server)
        .await;
    GithubClient::new(&server.uri(), GithubToken::new("test-token"))
        .unwrap()
        .open_pull_requests(&project())
        .await
}

#[tokio::test]
async fn maps_pull_requests() {
    let body = json!([
        {"number": 1, "title": "first", "state": "open", "draft": false, "ignored": "field"},
        {"number": 2, "title": "second", "state": "open", "draft": true},
    ]);

    let result = fetch(ResponseTemplate::new(200).set_body_json(body)).await;

    assert_eq!(
        result,
        Ok(vec![
            PullRequest {
                number: PrNumber(1),
                title: PrTitle("first".to_owned()),
                state: PrState::Open,
            },
            PullRequest {
                number: PrNumber(2),
                title: PrTitle("second".to_owned()),
                state: PrState::Draft,
            },
        ])
    );
}

#[tokio::test]
async fn empty_array_gives_empty_vec() {
    let result = fetch(ResponseTemplate::new(200).set_body_json(json!([]))).await;
    assert_eq!(result, Ok(vec![]));
}

#[tokio::test]
async fn sends_expected_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(PULLS_PATH))
        .and(query_param("state", "open"))
        .and(query_param("per_page", "100"))
        .and(header("authorization", "Bearer test-token"))
        .and(header("accept", "application/vnd.github+json"))
        .and(header("x-github-api-version", "2022-11-28"))
        .and(header_exists("user-agent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(1)
        .mount(&server)
        .await;

    let result = GithubClient::new(&server.uri(), GithubToken::new("test-token"))
        .unwrap()
        .open_pull_requests(&project())
        .await;

    assert_eq!(result, Ok(vec![]));
}

#[tokio::test]
async fn maps_404_to_not_found() {
    assert_eq!(
        fetch(ResponseTemplate::new(404)).await,
        Err(SourceError::NotFound)
    );
}

#[tokio::test]
async fn maps_401_to_unauthorized() {
    assert_eq!(
        fetch(ResponseTemplate::new(401)).await,
        Err(SourceError::Unauthorized)
    );
}

#[tokio::test]
async fn maps_403_with_exhausted_rate_limit_to_rate_limited() {
    let response = ResponseTemplate::new(403).insert_header("x-ratelimit-remaining", "0");
    assert_eq!(fetch(response).await, Err(SourceError::RateLimited));
}

#[tokio::test]
async fn maps_500_to_unavailable() {
    assert_eq!(
        fetch(ResponseTemplate::new(500)).await,
        Err(SourceError::Unavailable("HTTP 500".to_owned()))
    );
}

#[tokio::test]
async fn malformed_json_is_invalid_response() {
    let result = fetch(ResponseTemplate::new(200).set_body_string("not json")).await;
    assert!(
        matches!(result, Err(SourceError::InvalidResponse(_))),
        "result: {result:?}"
    );
}
