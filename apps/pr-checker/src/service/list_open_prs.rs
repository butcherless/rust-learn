//! Use case: list the open pull requests of several projects.

use futures::stream::{self, StreamExt};

use crate::domain::error::SourceError;
use crate::domain::model::{Project, PullRequest};
use crate::domain::port::PullRequestSource;

/// How many repositories are fetched at the same time.
pub const MAX_CONCURRENCY: usize = 2;

/// The result of fetching one project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectReport {
    /// The project that was fetched.
    pub project: Project,
    /// Its open pull requests, or why they could not be retrieved.
    pub outcome: Result<Vec<PullRequest>, SourceError>,
}

/// Fetches every project, at most `max_concurrency` at a time.
///
/// Reports keep the order of `projects`. Failures are isolated in each report.
pub async fn list_open_prs<S: PullRequestSource>(
    source: &S,
    projects: Vec<Project>,
    max_concurrency: usize,
) -> Vec<ProjectReport> {
    stream::iter(projects)
        .map(|project| async move {
            let outcome = source.open_pull_requests(&project).await;
            ProjectReport { project, outcome }
        })
        .buffered(max_concurrency)
        .collect()
        .await
}

/// Whether at least one project failed.
pub fn has_failures(reports: &[ProjectReport]) -> bool {
    reports.iter().any(|report| report.outcome.is_err())
}
