//! Use case: list the open pull requests of several projects.

use futures::stream::{self, Stream, StreamExt};

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

/// Fetches every project, at most `max_concurrency` at a time, and yields each report with the
/// project's position in `projects` as soon as it is ready, so a slow project doesn't hold back
/// the others.
pub fn stream_open_prs<S: PullRequestSource>(
    source: &S,
    projects: Vec<Project>,
    max_concurrency: usize,
) -> impl Stream<Item = (usize, ProjectReport)> {
    stream::iter(projects.into_iter().enumerate())
        .map(move |(index, project)| async move {
            let outcome = source.open_pull_requests(&project).await;
            (index, ProjectReport { project, outcome })
        })
        .buffer_unordered(max_concurrency)
}

/// Fetches every project, at most `max_concurrency` at a time.
///
/// Reports keep the order of `projects`. Failures are isolated in each report.
pub async fn list_open_prs<S: PullRequestSource>(
    source: &S,
    projects: Vec<Project>,
    max_concurrency: usize,
) -> Vec<ProjectReport> {
    let mut indexed: Vec<(usize, ProjectReport)> =
        stream_open_prs(source, projects, max_concurrency)
            .collect()
            .await;
    indexed.sort_by_key(|(index, _)| *index);
    indexed.into_iter().map(|(_, report)| report).collect()
}

/// Whether at least one project failed.
pub fn has_failures(reports: &[ProjectReport]) -> bool {
    reports.iter().any(|report| report.outcome.is_err())
}
