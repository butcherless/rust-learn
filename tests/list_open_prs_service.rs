//! The use case against an in-memory fake of the port.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use rust_learn::domain::error::SourceError;
use rust_learn::domain::model::{PrNumber, PrState, PrTitle, Project, PullRequest};
use rust_learn::domain::port::PullRequestSource;
use rust_learn::service::{ProjectReport, has_failures, list_open_prs};

fn project(name: &str) -> Project {
    name.parse().unwrap()
}

fn pull_request(number: u64) -> PullRequest {
    PullRequest {
        number: PrNumber(number),
        title: PrTitle(format!("title {number}")),
        state: PrState::Open,
    }
}

/// Answers by repository name, slowest first, to expose any reordering.
struct ScriptedSource;

impl PullRequestSource for ScriptedSource {
    async fn open_pull_requests(&self, project: &Project) -> Result<Vec<PullRequest>, SourceError> {
        match project.repo().as_str() {
            "success" => {
                tokio::time::sleep(Duration::from_millis(30)).await;
                Ok(vec![pull_request(1)])
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
    let projects = vec![
        project("a/success"),
        project("a/empty"),
        project("a/failure"),
    ];

    let reports = list_open_prs(&ScriptedSource, projects, 2).await;

    assert_eq!(
        reports,
        [
            ProjectReport {
                project: project("a/success"),
                outcome: Ok(vec![pull_request(1)]),
            },
            ProjectReport {
                project: project("a/empty"),
                outcome: Ok(vec![]),
            },
            ProjectReport {
                project: project("a/failure"),
                outcome: Err(SourceError::NotFound),
            },
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
    let succeeded = ProjectReport {
        project: project("a/ok"),
        outcome: Ok(vec![]),
    };
    let failed = ProjectReport {
        project: project("a/ko"),
        outcome: Err(SourceError::RateLimited),
    };

    assert!(!has_failures(&[]));
    assert!(!has_failures(std::slice::from_ref(&succeeded)));
    assert!(has_failures(&[succeeded, failed]));
}
