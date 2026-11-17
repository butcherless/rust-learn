//! Outbound port.

use crate::domain::error::SourceError;
use crate::domain::model::{PrNumber, Project, PullRequest};

/// A source of pull request data.
pub trait PullRequestSource {
    /// Retrieves the open pull requests of `project`.
    fn open_pull_requests(
        &self,
        project: &Project,
    ) -> impl Future<Output = Result<Vec<PullRequest>, SourceError>> + Send;
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
