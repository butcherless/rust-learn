//! HTTP client implementing [`PullRequestSource`] against GitHub.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use futures::stream::{self, StreamExt, TryStreamExt};
use reqwest::header::{ACCEPT, HeaderMap, USER_AGENT};
use reqwest::{Client, Method, RequestBuilder, StatusCode, Url};
use serde::de::DeserializeOwned;
use thiserror::Error;
use tokio::sync::{Semaphore, SemaphorePermit};

use crate::adapter::github::dto::{
    MergeRequestDto, PullRequestDto, UpdateRequestDto, WorkflowRunsDto,
};
use crate::adapter::github::mapper::{to_domain, to_running};
use crate::domain::error::SourceError;
use crate::domain::model::{PrNumber, Project, PullRequest, WorkflowRun};
use crate::domain::port::{
    PullRequestCloser, PullRequestMerger, PullRequestSource, WorkflowRunSource,
};

/// The public GitHub API.
pub const DEFAULT_BASE_URL: &str = "https://api.github.com";

/// How many pull requests of one repository are fetched at the same time, for their mergeability.
const DETAIL_CONCURRENCY: usize = 4;
/// How many requests one client has in flight at most, whatever asks for them: GitHub advises
/// against concurrent requests to stay clear of its secondary rate limits, so the rest wait in a
/// queue. See <https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api>.
pub const MAX_IN_FLIGHT: usize = 4;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const API_VERSION_HEADER: &str = "x-github-api-version";
const API_VERSION: &str = "2022-11-28";
const ACCEPT_VALUE: &str = "application/vnd.github+json";
const RATE_LIMIT_REMAINING_HEADER: &str = "x-ratelimit-remaining";
const USER_AGENT_VALUE: &str = concat!("pr-checker/", env!("CARGO_PKG_VERSION"));
/// How pull requests are merged: with a merge commit, GitHub's default.
const MERGE_METHOD: &str = "merge";
/// How many of the newest workflow runs are checked for unfinished ones: one page, which covers
/// what is running now in all but the busiest repositories.
const RUNS_PAGE: &str = "30";
/// The state that closes a pull request.
const CLOSED_STATE: &str = "closed";

/// A GitHub API token. Its `Debug` output is redacted.
#[derive(Clone)]
pub struct GithubToken(String);

impl GithubToken {
    /// Wraps a token value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl fmt::Debug for GithubToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GithubToken(***)")
    }
}

/// Why a [`GithubClient`] could not be created.
#[derive(Debug, Error)]
pub enum GithubClientError {
    /// The base URL cannot be parsed or cannot have path segments.
    #[error("invalid base URL {0:?}")]
    InvalidBaseUrl(String),
    /// The HTTP client could not be initialized.
    #[error("cannot initialize the HTTP client")]
    Build(#[from] reqwest::Error),
}

/// GitHub-backed source of pull requests. Its clones share the queue of [`MAX_IN_FLIGHT`]
/// requests.
#[derive(Debug, Clone)]
pub struct GithubClient {
    http: Client,
    base_url: Url,
    token: GithubToken,
    in_flight: Arc<Semaphore>,
}

impl GithubClient {
    /// Creates a client for the API at `base_url`.
    pub fn new(base_url: &str, token: GithubToken) -> Result<Self, GithubClientError> {
        let base_url = Url::parse(base_url)
            .ok()
            .filter(|url| !url.cannot_be_a_base())
            .ok_or_else(|| GithubClientError::InvalidBaseUrl(base_url.to_owned()))?;
        let http = Client::builder().timeout(REQUEST_TIMEOUT).build()?;
        Ok(Self {
            http,
            base_url,
            token,
            in_flight: Arc::new(Semaphore::new(MAX_IN_FLIGHT)),
        })
    }

    /// The address of `/repos/{owner}/{repo}` followed by the segments of `tail`.
    fn repo_path(&self, project: &Project, tail: &[&str]) -> Result<Url, SourceError> {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .map_err(|()| SourceError::Unavailable("invalid base URL".to_owned()))?
            .pop_if_empty()
            .extend(["repos", project.owner().as_str(), project.repo().as_str()])
            .extend(tail);
        Ok(url)
    }

    /// The address of `/repos/{owner}/{repo}/pulls` followed by the segments of `tail`.
    fn pulls_path(&self, project: &Project, tail: &[&str]) -> Result<Url, SourceError> {
        self.repo_path(project, &[&["pulls"], tail].concat())
    }

    fn pulls_url(&self, project: &Project) -> Result<Url, SourceError> {
        let mut url = self.pulls_path(project, &[])?;
        url.query_pairs_mut()
            .append_pair("state", "open")
            .append_pair("per_page", "100");
        Ok(url)
    }

    /// The address of `/repos/{owner}/{repo}/actions/runs` with the newest page of runs, without
    /// their pull requests.
    fn runs_url(&self, project: &Project) -> Result<Url, SourceError> {
        let mut url = self.repo_path(project, &["actions", "runs"])?;
        url.query_pairs_mut()
            .append_pair("per_page", RUNS_PAGE)
            .append_pair("exclude_pull_requests", "true");
        Ok(url)
    }

    fn pull_url(&self, project: &Project, number: u64) -> Result<Url, SourceError> {
        self.pulls_path(project, &[&number.to_string()])
    }

    fn close_url(&self, project: &Project, number: PrNumber) -> Result<Url, SourceError> {
        self.pulls_path(project, &[&number.to_string()])
    }

    fn merge_url(&self, project: &Project, number: PrNumber) -> Result<Url, SourceError> {
        self.pulls_path(project, &[&number.to_string(), "merge"])
    }

    /// A request with the headers that every call carries.
    fn request(&self, method: Method, url: Url) -> RequestBuilder {
        self.http
            .request(method, url)
            .bearer_auth(&self.token.0)
            .header(ACCEPT, ACCEPT_VALUE)
            .header(API_VERSION_HEADER, API_VERSION)
            .header(USER_AGENT, USER_AGENT_VALUE)
    }

    /// Waits for a free place among the [`MAX_IN_FLIGHT`] requests. It is held until the response
    /// is read.
    async fn queue(&self) -> Result<SemaphorePermit<'_>, SourceError> {
        self.in_flight
            .acquire()
            .await
            .map_err(|_| SourceError::Unavailable("request queue closed".to_owned()))
    }

    async fn get<T: DeserializeOwned>(&self, url: Url) -> Result<T, SourceError> {
        let _permit = self.queue().await?;
        let response = self
            .request(Method::GET, url)
            .send()
            .await
            .map_err(transport_error)?;

        let status = response.status();
        if !status.is_success() {
            return Err(map_status(status, ratelimit_remaining(response.headers())));
        }
        response.json::<T>().await.map_err(body_error)
    }

    /// Merges the pull request. Any success status means it was merged, so the body is not read.
    async fn put_merge(&self, project: &Project, number: PrNumber) -> Result<(), SourceError> {
        let _permit = self.queue().await?;
        let response = self
            .request(Method::PUT, self.merge_url(project, number)?)
            .json(&MergeRequestDto {
                merge_method: MERGE_METHOD,
            })
            .send()
            .await
            .map_err(transport_error)?;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(map_merge_status(
                status,
                ratelimit_remaining(response.headers()),
            ))
        }
    }

    /// Closes the pull request. Any success status means it was closed, so the body is not read.
    async fn patch_closed(&self, project: &Project, number: PrNumber) -> Result<(), SourceError> {
        let _permit = self.queue().await?;
        let response = self
            .request(Method::PATCH, self.close_url(project, number)?)
            .json(&UpdateRequestDto {
                state: CLOSED_STATE,
            })
            .send()
            .await
            .map_err(transport_error)?;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(map_status(status, ratelimit_remaining(response.headers())))
        }
    }

    /// Lists the open pull requests, then gets each one, because only the single pull request
    /// endpoint says whether it is mergeable.
    async fn fetch(&self, project: &Project) -> Result<Vec<PullRequest>, SourceError> {
        let listed: Vec<PullRequestDto> = self.get(self.pulls_url(project)?).await?;
        stream::iter(listed)
            .map(|dto| async move { self.get(self.pull_url(project, dto.number)?).await })
            .buffered(DETAIL_CONCURRENCY)
            .map_ok(to_domain)
            .try_collect()
            .await
    }
}

impl PullRequestSource for GithubClient {
    fn open_pull_requests(
        &self,
        project: &Project,
    ) -> impl Future<Output = Result<Vec<PullRequest>, SourceError>> + Send {
        self.fetch(project)
    }
}

impl WorkflowRunSource for GithubClient {
    async fn running_workflows(&self, project: &Project) -> Result<Vec<WorkflowRun>, SourceError> {
        let page: WorkflowRunsDto = self.get(self.runs_url(project)?).await?;
        Ok(page
            .workflow_runs
            .into_iter()
            .filter_map(to_running)
            .collect())
    }
}

impl PullRequestCloser for GithubClient {
    fn close(
        &self,
        project: &Project,
        number: PrNumber,
    ) -> impl Future<Output = Result<(), SourceError>> + Send {
        self.patch_closed(project, number)
    }
}

impl PullRequestMerger for GithubClient {
    fn merge(
        &self,
        project: &Project,
        number: PrNumber,
    ) -> impl Future<Output = Result<(), SourceError>> + Send {
        self.put_merge(project, number)
    }
}

fn ratelimit_remaining(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(RATE_LIMIT_REMAINING_HEADER)
        .and_then(|value| value.to_str().ok())
}

fn transport_error(error: reqwest::Error) -> SourceError {
    SourceError::Unavailable(error.without_url().to_string())
}

fn body_error(error: reqwest::Error) -> SourceError {
    if error.is_decode() {
        SourceError::InvalidResponse(error.without_url().to_string())
    } else {
        transport_error(error)
    }
}

/// Classifies a non-success response.
fn map_status(status: StatusCode, ratelimit_remaining: Option<&str>) -> SourceError {
    match status {
        StatusCode::NOT_FOUND => SourceError::NotFound,
        StatusCode::UNAUTHORIZED => SourceError::Unauthorized,
        StatusCode::TOO_MANY_REQUESTS => SourceError::RateLimited,
        StatusCode::FORBIDDEN if ratelimit_remaining == Some("0") => SourceError::RateLimited,
        StatusCode::FORBIDDEN => SourceError::Unauthorized,
        other => SourceError::Unavailable(format!("HTTP {}", other.as_u16())),
    }
}

/// Classifies a non-success response to a merge: GitHub refuses one that cannot be done with 405,
/// a head that changed with 409, and a failed validation with 422.
fn map_merge_status(status: StatusCode, ratelimit_remaining: Option<&str>) -> SourceError {
    match status {
        StatusCode::METHOD_NOT_ALLOWED
        | StatusCode::CONFLICT
        | StatusCode::UNPROCESSABLE_ENTITY => SourceError::NotMergeable,
        other => map_status(other, ratelimit_remaining),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_statuses_to_errors() {
        [
            (StatusCode::NOT_FOUND, None, SourceError::NotFound),
            (StatusCode::UNAUTHORIZED, None, SourceError::Unauthorized),
            (StatusCode::FORBIDDEN, Some("0"), SourceError::RateLimited),
            (StatusCode::FORBIDDEN, None, SourceError::Unauthorized),
            (StatusCode::FORBIDDEN, Some("10"), SourceError::Unauthorized),
            (
                StatusCode::TOO_MANY_REQUESTS,
                None,
                SourceError::RateLimited,
            ),
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                None,
                SourceError::Unavailable("HTTP 500".to_owned()),
            ),
        ]
        .into_iter()
        .for_each(|(status, remaining, expected)| {
            assert_eq!(
                map_status(status, remaining),
                expected,
                "status {status}, x-ratelimit-remaining {remaining:?}"
            );
        });
    }

    #[test]
    fn maps_merge_statuses_to_errors() {
        [
            (StatusCode::METHOD_NOT_ALLOWED, SourceError::NotMergeable),
            (StatusCode::CONFLICT, SourceError::NotMergeable),
            (StatusCode::UNPROCESSABLE_ENTITY, SourceError::NotMergeable),
            (StatusCode::FORBIDDEN, SourceError::Unauthorized),
            (StatusCode::NOT_FOUND, SourceError::NotFound),
        ]
        .into_iter()
        .for_each(|(status, expected)| {
            assert_eq!(map_merge_status(status, None), expected, "status {status}");
        });
    }

    #[test]
    fn token_debug_is_redacted() {
        let token = GithubToken::new("super-secret");
        assert!(!format!("{token:?}").contains("super-secret"));
    }

    #[test]
    fn rejects_invalid_base_urls() {
        ["not a url", "mailto:someone@example.com"]
            .iter()
            .for_each(|base_url| {
                assert!(matches!(
                    GithubClient::new(base_url, GithubToken::new("t")),
                    Err(GithubClientError::InvalidBaseUrl(_))
                ));
            });
    }
}
