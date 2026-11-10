//! Wire types of the GitHub REST API.

use serde::Deserialize;

/// A pull request as returned by `GET /repos/{owner}/{repo}/pulls`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PullRequestDto {
    /// The pull request number.
    pub number: u64,
    /// The pull request title.
    pub title: String,
    /// The API state. Unused by the mapper: only open pull requests are requested.
    pub state: String,
    /// The pull request's page on github.com.
    pub html_url: String,
    /// Whether the pull request is a draft.
    #[serde(default)]
    pub draft: bool,
}
