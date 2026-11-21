//! Wire types of the GitHub REST API.

use serde::{Deserialize, Serialize};

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

/// The page returned by `GET /repos/{owner}/{repo}/actions/runs`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WorkflowRunsDto {
    /// The runs, newest first.
    pub workflow_runs: Vec<WorkflowRunDto>,
}

/// A workflow run, with the fields shown.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WorkflowRunDto {
    /// The workflow's name.
    pub name: Option<String>,
    /// The branch it runs on.
    pub head_branch: Option<String>,
    /// What triggered it.
    pub event: String,
    /// `queued`, `in_progress`, `requested`, `waiting`, `pending` or `completed`.
    pub status: Option<String>,
}

/// The body of `PATCH /repos/{owner}/{repo}/pulls/{number}` that closes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateRequestDto {
    /// `open` or `closed`.
    pub state: &'static str,
}

/// The body of `PUT /repos/{owner}/{repo}/pulls/{number}/merge`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MergeRequestDto {
    /// `merge`, `squash` or `rebase`.
    pub merge_method: &'static str,
}
