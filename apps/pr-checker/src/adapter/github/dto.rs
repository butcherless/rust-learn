//! Wire types of the GitHub REST API.

use serde::Deserialize;

/// A pull request as returned by `GET /repos/{owner}/{repo}/pulls` and
/// `GET /repos/{owner}/{repo}/pulls/{number}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PullRequestDto {
    /// The pull request number.
    pub number: u64,
    /// The pull request title.
    pub title: String,
    /// The pull request's page on github.com.
    pub html_url: String,
    /// Whether the pull request is a draft.
    #[serde(default)]
    pub draft: bool,
    /// Whether it merges without conflicts. Only the single pull request endpoint returns it, and
    /// it is `null` while GitHub computes it.
    #[serde(default)]
    pub mergeable: Option<bool>,
}
