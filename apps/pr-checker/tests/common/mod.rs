//! Builders shared by the integration tests. Each test file is its own crate, so it includes this
//! module with `mod common;`.

use pr_checker::domain::model::{
    Mergeable, PrNumber, PrState, PrTitle, PrUrl, Project, PullRequest,
};

/// Parses an `owner/repo` project.
pub fn project(name: &str) -> Project {
    name.parse().unwrap()
}

/// The pull request #`number`, titled `title {number}`, at `https://github.com/o/r/pull/{number}`.
pub fn pull_request(number: u64, state: PrState, mergeable: Mergeable) -> PullRequest {
    PullRequest {
        number: PrNumber::from(number),
        title: PrTitle::from(format!("title {number}")),
        state,
        mergeable,
        url: PrUrl::from(format!("https://github.com/o/r/pull/{number}")),
    }
}
