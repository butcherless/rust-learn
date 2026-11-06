//! `GithubClient` against a local mock server. No real network is used.
//!
//! The full status-to-error table is unit tested next to `map_status`; these tests check the
//! wiring: the request sent, the body decoded, and the status and headers reaching the mapping.

mod common;

use std::net::TcpListener;

use common::{project, pull_request};
use futures::future::join_all;
use rust_learn::adapter::github::{GithubClient, GithubToken};
use rust_learn::domain::error::SourceError;
use rust_learn::domain::model::{PrState, PullRequest};
use rust_learn::domain::port::PullRequestSource;
use serde_json::{Value, json};
use wiremock::matchers::{header, header_exists, method, path, query_param};
use wiremock::{Mock, MockBuilder, MockServer, ResponseTemplate};

const PULLS_PATH: &str = "/repos/acme/widgets/pulls";
const TOKEN: &str = "test-token";

/// A mock of the pulls endpoint, to refine with more matchers.
fn pulls() -> MockBuilder {
    Mock::given(method("GET")).and(path(PULLS_PATH))
}

/// Fetches the project from the API at `base_url`.
async fn fetch_at(base_url: &str) -> Result<Vec<PullRequest>, SourceError> {
    GithubClient::new(base_url, GithubToken::new(TOKEN))
        .unwrap()
        .open_pull_requests(&project("acme/widgets"))
        .await
}

/// Fetches the project from a server that only knows `mock`. Expectations set on `mock` are
/// verified when the server is dropped at the end.
async fn fetch_from(mock: Mock) -> Result<Vec<PullRequest>, SourceError> {
    let server = MockServer::start().await;
    mock.mount(&server).await;
    fetch_at(&server.uri()).await
}

/// Fetches the project from a server that always answers with `response`.
async fn fetch(response: ResponseTemplate) -> Result<Vec<PullRequest>, SourceError> {
    fetch_from(pulls().respond_with(response)).await
}

/// The API's JSON for `pull_request(number, …)`.
fn pull_request_json(number: u64, draft: bool) -> Value {
    json!({
        "number": number,
        "title": format!("title {number}"),
        "state": "open",
        "html_url": format!("https://github.com/o/r/pull/{number}"),
        "draft": draft,
    })
}

/// `object` with one more field, such as one the client does not know.
fn with_field(mut object: Value, key: &str, value: Value) -> Value {
    object[key] = value;
    object
}

#[tokio::test]
async fn maps_pull_requests() {
    let body = json!([
        with_field(pull_request_json(1, false), "ignored", json!("unknown")),
        pull_request_json(2, true),
    ]);

    assert_eq!(
        fetch(ResponseTemplate::new(200).set_body_json(body)).await,
        Ok(vec![
            pull_request(1, PrState::Open),
            pull_request(2, PrState::Draft)
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
    let mock = pulls()
        .and(query_param("state", "open"))
        .and(query_param("per_page", "100"))
        .and(header("authorization", format!("Bearer {TOKEN}").as_str()))
        .and(header("accept", "application/vnd.github+json"))
        .and(header("x-github-api-version", "2022-11-28"))
        .and(header_exists("user-agent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(1);

    assert_eq!(fetch_from(mock).await, Ok(vec![]));
}

#[tokio::test]
async fn maps_error_statuses() {
    let cases = [
        (ResponseTemplate::new(404), SourceError::NotFound),
        (
            ResponseTemplate::new(403).insert_header("x-ratelimit-remaining", "0"),
            SourceError::RateLimited,
        ),
    ];

    let results = join_all(
        cases.map(|(response, expected)| async move { (fetch(response).await, expected) }),
    )
    .await;

    results
        .into_iter()
        .for_each(|(result, expected)| assert_eq!(result, Err(expected)));
}

#[tokio::test]
async fn malformed_json_is_invalid_response() {
    let result = fetch(ResponseTemplate::new(200).set_body_string("not json")).await;
    assert!(
        matches!(result, Err(SourceError::InvalidResponse(_))),
        "result: {result:?}"
    );
}

#[tokio::test]
async fn unreachable_server_is_unavailable_without_leaking_the_url() {
    // Bind a free port, then close it, so connecting is refused.
    let address = TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .unwrap();
    let base_url = format!("http://{address}");

    let result = fetch_at(&base_url).await;

    match result {
        Err(SourceError::Unavailable(message)) => assert!(
            !message.contains(&address.to_string()),
            "the message must not include the URL: {message}"
        ),
        other => panic!("expected Unavailable, got {other:?}"),
    }
}
