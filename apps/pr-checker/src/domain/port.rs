//! Outbound port.

use crate::domain::error::SourceError;
use crate::domain::model::{PrNumber, Project, PullRequest, WorkflowRun};

/// A source of pull request data.
pub trait PullRequestSource {
    /// Retrieves the open pull requests of `project`.
    fn open_pull_requests(
        &self,
        project: &Project,
    ) -> impl Future<Output = Result<Vec<PullRequest>, SourceError>> + Send;
}

/// A source of the workflow runs of a project's continuous integration.
pub trait WorkflowRunSource {
    /// Retrieves the workflow runs of `project` that have not finished.
    fn running_workflows(
        &self,
        project: &Project,
    ) -> impl Future<Output = Result<Vec<WorkflowRun>, SourceError>> + Send;
}

/// Something that closes pull requests without merging them.
pub trait PullRequestCloser {
    /// Closes pull request `number` of `project`.
    fn close(
        &self,
        project: &Project,
        number: PrNumber,
    ) -> impl Future<Output = Result<(), SourceError>> + Send;
}

/// Something that merges pull requests.
pub trait PullRequestMerger {
    /// Merges pull request `number` of `project` into its base branch.
    fn merge(
        &self,
        project: &Project,
        number: PrNumber,
    ) -> impl Future<Output = Result<(), SourceError>> + Send;
}
