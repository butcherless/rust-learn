//! The use case against in-memory fakes of the port.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use common::{project, pull_request};
use pr_checker::domain::error::SourceError;
use pr_checker::domain::model::{PrState, Project, PullRequest};
use pr_checker::domain::port::PullRequestSource;
use pr_checker::service::{ProjectReport, has_failures, list_open_prs};

fn report(name: &str, outcome: Result<Vec<PullRequest>, SourceError>) -> ProjectReport {
    ProjectReport {
        project: project(name),
        outcome,
    }
}

/// Answers by repository name, slowest first, to expose any reordering.
struct ScriptedSource;

impl PullRequestSource for ScriptedSource {
    async fn open_pull_requests(&self, project: &Project) -> Result<Vec<PullRequest>, SourceError> {
        match project.repo().as_str() {
            "success" => {
                tokio::time::sleep(Duration::from_millis(30)).await;
                Ok(vec![pull_request(1, PrState::Open)])
            }
            "empty" => {
                tokio::time::sleep(Duration::from_millis(10)).await;
                Ok(vec![])
            }
            _ => Err(SourceError::NotFound),
        }
    }
}

/// Records the peak number of calls in flight.
#[derive(Default)]
struct TrackingSource {
    in_flight: AtomicUsize,
    peak: AtomicUsize,
}

impl PullRequestSource for TrackingSource {
    async fn open_pull_requests(
        &self,
        _project: &Project,
    ) -> Result<Vec<PullRequest>, SourceError> {
        let current = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(current, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(20)).await;
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        Ok(vec![])
    }
}

#[tokio::test]
async fn mixed_outcomes_keep_input_order() {
    let projects = ["a/success", "a/empty", "a/failure"].map(project).to_vec();

    assert_eq!(
        list_open_prs(&ScriptedSource, projects, 2).await,
        [
            report("a/success", Ok(vec![pull_request(1, PrState::Open)])),
            report("a/empty", Ok(vec![])),
            report("a/failure", Err(SourceError::NotFound)),
        ]
    );
}

#[tokio::test]
async fn concurrency_is_capped() {
    let source = TrackingSource::default();
    let projects = (0..5).map(|n| project(&format!("a/r{n}"))).collect();

    let reports = list_open_prs(&source, projects, 2).await;

    assert_eq!(reports.len(), 5);
    assert_eq!(source.peak.load(Ordering::SeqCst), 2);
}

#[test]
fn has_failures_detects_errors() {
    let succeeded = report("a/ok", Ok(vec![]));
    let failed = report("a/ko", Err(SourceError::RateLimited));

    assert!(!has_failures(&[]));
    assert!(!has_failures(std::slice::from_ref(&succeeded)));
    assert!(has_failures(&[succeeded, failed]));
}
