//! `GithubClient` against a local mock server. No real network is used.
//!
//! The full status-to-error table is unit tested next to `map_status`; these tests check the
//! wiring: the request sent, the body decoded, and the status and headers reaching the mapping.

mod common;

use std::net::TcpListener;

use common::{project, pull_request};
use futures::future::join_all;
use pr_checker::adapter::github::{GithubClient, GithubToken};
use pr_checker::domain::error::SourceError;
use pr_checker::domain::model::{Mergeable, PrState, PullRequest};
use pr_checker::domain::port::PullRequestSource;
use serde_json::{Value, json};
use wiremock::matchers::{header, header_exists, method, path, query_param};
use wiremock::{Mock, MockBuilder, MockServer, ResponseTemplate};

const PULLS_PATH: &str = "/repos/acme/widgets/pulls";
const TOKEN: &str = "test-token";

/// A mock of the pulls endpoint, to refine with more matchers.
fn pulls() -> MockBuilder {
    Mock::given(method("GET")).and(path(PULLS_PATH))
}

/// A mock of the endpoint of pull request #`number`.
fn pull(number: u64) -> MockBuilder {
    Mock::given(method("GET")).and(path(format!("{PULLS_PATH}/{number}")))
}

/// Fetches the project from the API at `base_url`.
async fn fetch_at(base_url: &str) -> Result<Vec<PullRequest>, SourceError> {
    GithubClient::new(base_url, GithubToken::new(TOKEN))
        .unwrap()
        .open_pull_requests(&project("acme/widgets"))
        .await
}

/// Fetches the project from a server that only knows `mocks`. Expectations set on them are
/// verified when the server is dropped at the end.
async fn fetch_from(
    mocks: impl IntoIterator<Item = Mock>,
) -> Result<Vec<PullRequest>, SourceError> {
    let server = MockServer::start().await;
    join_all(mocks.into_iter().map(|mock| mock.mount(&server))).await;
    fetch_at(&server.uri()).await
}

/// Fetches the project from a server whose list endpoint always answers with `response`.
async fn fetch(response: ResponseTemplate) -> Result<Vec<PullRequest>, SourceError> {
    fetch_from([pulls().respond_with(response)]).await
}

/// The API's JSON for `pull_request(number, …)`, as the list endpoint returns it.
fn pull_request_json(number: u64, draft: bool) -> Value {
    json!({
        "number": number,
        "title": format!("title {number}"),
        "state": "open",
        "html_url": format!("https://github.com/o/r/pull/{number}"),
        "draft": draft,
    })
}

/// A response with the list of pull requests `numbers`, none of them a draft.
fn listing(numbers: &[u64]) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(
        numbers
            .iter()
            .map(|&number| pull_request_json(number, false))
            .collect::<Value>(),
    )
}

/// A response with pull request #`number`, as the single pull request endpoint returns it.
fn detail(number: u64, draft: bool, mergeable: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(with_field(
        pull_request_json(number, draft),
        "mergeable",
        mergeable,
    ))
}

/// `object` with one more field, such as one the client does not know.
fn with_field(mut object: Value, key: &str, value: Value) -> Value {
    object[key] = value;
    object
}

#[tokio::test]
async fn maps_pull_requests_with_their_mergeability() {
    let body = json!([
        with_field(pull_request_json(1, false), "ignored", json!("unknown")),
        pull_request_json(2, true),
        pull_request_json(3, false),
    ]);
    let mocks = [
        pulls().respond_with(ResponseTemplate::new(200).set_body_json(body)),
        pull(1).respond_with(detail(1, false, json!(true))),
        pull(2).respond_with(detail(2, true, json!(false))),
        pull(3).respond_with(detail(3, false, Value::Null)),
    ];

    assert_eq!(
        fetch_from(mocks).await,
        Ok(vec![
            pull_request(1, PrState::Open, Mergeable::Yes),
            pull_request(2, PrState::Draft, Mergeable::No),
            pull_request(3, PrState::Open, Mergeable::Unknown),
        ])
    );
}

#[tokio::test]
async fn empty_array_gives_empty_vec() {
    let result = fetch(ResponseTemplate::new(200).set_body_json(json!([]))).await;
    assert_eq!(result, Ok(vec![]));
}

/// `mock` refined with the headers that every request carries.
fn with_api_headers(mock: MockBuilder) -> MockBuilder {
    mock.and(header("authorization", format!("Bearer {TOKEN}").as_str()))
        .and(header("accept", "application/vnd.github+json"))
        .and(header("x-github-api-version", "2022-11-28"))
        .and(header_exists("user-agent"))
}

#[tokio::test]
async fn sends_expected_requests() {
    let mocks = [
        with_api_headers(pulls())
            .and(query_param("state", "open"))
            .and(query_param("per_page", "100"))
            .respond_with(listing(&[1]))
            .expect(1),
        with_api_headers(pull(1))
            .respond_with(detail(1, false, json!(true)))
            .expect(1),
    ];

    assert_eq!(
        fetch_from(mocks).await,
        Ok(vec![pull_request(1, PrState::Open, Mergeable::Yes)])
    );
}

#[tokio::test]
async fn a_failing_pull_request_fails_the_project() {
    let mocks = [
        pulls().respond_with(listing(&[1, 2])),
        pull(1).respond_with(detail(1, false, json!(true))),
        pull(2).respond_with(ResponseTemplate::new(404)),
    ];

    assert_eq!(fetch_from(mocks).await, Err(SourceError::NotFound));
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
