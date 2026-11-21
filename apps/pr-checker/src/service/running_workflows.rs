//! Use case: list the unfinished workflow runs of several projects.

use futures::stream::{self, Stream, StreamExt};

use crate::domain::error::SourceError;
use crate::domain::model::{Project, WorkflowRun};
use crate::domain::port::WorkflowRunSource;

/// The unfinished workflow runs of one project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunsReport {
    /// The project that was fetched.
    pub project: Project,
    /// Its unfinished workflow runs, or why they could not be retrieved.
    pub outcome: Result<Vec<WorkflowRun>, SourceError>,
}

/// Fetches the unfinished workflow runs of every project, at most `max_concurrency` at a time, and
/// yields each report with the project's position in `projects` as soon as it is ready.
pub fn stream_running_workflows<S: WorkflowRunSource>(
    source: &S,
    projects: Vec<Project>,
    max_concurrency: usize,
) -> impl Stream<Item = (usize, RunsReport)> {
    stream::iter(projects.into_iter().enumerate())
        .map(move |(index, project)| async move {
            let outcome = source.running_workflows(&project).await;
            (index, RunsReport { project, outcome })
        })
        .buffer_unordered(max_concurrency)
}
