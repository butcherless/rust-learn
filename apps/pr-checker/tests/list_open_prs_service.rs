//! The use cases against in-memory fakes of the ports.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use common::{project, pull_request, workflow_run};
use futures::StreamExt;
use pr_checker::domain::error::SourceError;
use pr_checker::domain::model::{Mergeable, PrState, Project, PullRequest, RunStatus, WorkflowRun};
use pr_checker::domain::port::{PullRequestSource, WorkflowRunSource};
use pr_checker::service::{
    ProjectReport, RunsReport, has_failures, list_open_prs, stream_open_prs,
    stream_running_workflows,
};

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
                Ok(vec![pull_request(1, PrState::Open, Mergeable::Yes)])
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
            report(
                "a/success",
                Ok(vec![pull_request(1, PrState::Open, Mergeable::Yes)])
            ),
            report("a/empty", Ok(vec![])),
            report("a/failure", Err(SourceError::NotFound)),
        ]
    );
}

#[tokio::test]
async fn the_stream_yields_each_project_when_ready_with_its_position() {
    let projects = ["a/success", "a/empty", "a/failure"].map(project).to_vec();

    let yielded: Vec<(usize, ProjectReport)> = stream_open_prs(&ScriptedSource, projects, 3)
        .collect()
        .await;

    assert_eq!(
        yielded,
        [
            (2, report("a/failure", Err(SourceError::NotFound))),
            (1, report("a/empty", Ok(vec![]))),
            (
                0,
                report(
                    "a/success",
                    Ok(vec![pull_request(1, PrState::Open, Mergeable::Yes)])
                )
            ),
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

/// Answers by repository name, slowest first, to expose any reordering.
struct ScriptedRuns;

impl WorkflowRunSource for ScriptedRuns {
    async fn running_workflows(&self, project: &Project) -> Result<Vec<WorkflowRun>, SourceError> {
        match project.repo().as_str() {
            "busy" => {
                tokio::time::sleep(Duration::from_millis(20)).await;
                Ok(vec![workflow_run("CI", RunStatus::InProgress)])
            }
            "idle" => Ok(vec![]),
            _ => Err(SourceError::Unauthorized),
        }
    }
}

#[tokio::test]
async fn yields_the_runs_of_each_project_with_its_position() {
    let projects = ["a/busy", "a/idle", "a/private"].map(project).to_vec();

    let mut reports: Vec<(usize, RunsReport)> =
        stream_running_workflows(&ScriptedRuns, projects, 3)
            .collect()
            .await;
    reports.sort_by_key(|(index, _)| *index);

    assert_eq!(
        reports,
        [
            (
                0,
                RunsReport {
                    project: project("a/busy"),
                    outcome: Ok(vec![workflow_run("CI", RunStatus::InProgress)]),
                }
            ),
            (
                1,
                RunsReport {
                    project: project("a/idle"),
                    outcome: Ok(vec![]),
                }
            ),
            (
                2,
                RunsReport {
                    project: project("a/private"),
                    outcome: Err(SourceError::Unauthorized),
                }
            ),
        ]
    );
}
