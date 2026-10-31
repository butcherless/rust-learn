//! Outbound port.

use crate::domain::error::SourceError;
use crate::domain::model::{Project, PullRequest};

/// A source of pull request data.
pub trait PullRequestSource {
    /// Retrieves the open pull requests of `project`.
    fn open_pull_requests(
        &self,
        project: &Project,
    ) -> impl Future<Output = Result<Vec<PullRequest>, SourceError>> + Send;
}
