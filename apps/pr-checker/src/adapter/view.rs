//! Presentation model shared by the table and the terminal UI: which rows to show, their text,
//! their kind and their pull request address. Pure, and free of any rendering library.

use std::fmt;

use crate::domain::model::{PrNumber, PrUrl, Project, PullRequest, RunStatus};
use crate::service::{ProjectReport, RunsReport};

/// The column titles.
pub const HEADER: [&str; 5] = ["Project", "PR #", "Title", "State", "Mergeable"];
/// The orange of open pull request rows, as RGB.
pub const ORANGE: (u8, u8, u8) = (255, 165, 0);
const NO_VALUE: &str = "—";
/// The message of a project without open pull requests (FR9).
pub const NO_PULL_REQUESTS: &str = "no open pull requests";
/// The message of a project whose first fetch has not finished yet (FR14).
pub const LOADING_PROJECT: &str = "loading…";
/// The state of a pull request merged from the terminal UI (FR15).
pub const MERGED: &str = "merged";
/// The state of a pull request closed from the terminal UI (FR16).
pub const CLOSED: &str = "closed";

/// The column titles of the workflow runs panel.
pub const RUN_HEADER: [&str; 5] = ["Project", "Workflow", "Branch", "Event", "Status"];

/// What a workflow run row represents, which decides its color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunRowKind {
    /// A run with this status.
    Run(RunStatus),
    /// A project whose runs could not be fetched.
    Failed,
}

/// One row of the workflow runs panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRow {
    /// What the row represents.
    pub kind: RunRowKind,
    /// The text of each column, in [`RUN_HEADER`] order.
    pub cells: [String; 5],
}

/// The rows of the workflow runs panel, in report order: one per unfinished run, and one per
/// project whose runs failed. Projects without unfinished runs have none.
pub fn run_rows<'a>(reports: impl IntoIterator<Item = &'a RunsReport>) -> Vec<RunRow> {
    reports
        .into_iter()
        .flat_map(|report| {
            let project = report.project.to_string();
            match &report.outcome {
                Ok(runs) => runs
                    .iter()
                    .map(|run| RunRow {
                        kind: RunRowKind::Run(run.status),
                        cells: [
                            project.clone(),
                            run.name.clone(),
                            run.branch.clone(),
                            run.event.clone(),
                            run.status.to_string(),
                        ],
                    })
                    .collect(),
                Err(error) => vec![RunRow {
                    kind: RunRowKind::Failed,
                    cells: [
                        project,
                        format!("error: {error}"),
                        NO_VALUE.to_owned(),
                        NO_VALUE.to_owned(),
                        NO_VALUE.to_owned(),
                    ],
                }],
            }
        })
        .collect()
}

/// What a row represents, which decides its color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// A project without open pull requests.
    NoPullRequests,
    /// An open pull request.
    OpenPullRequest,
    /// A pull request merged in this session, until a fetch no longer lists it.
    MergedPullRequest,
    /// A pull request closed in this session, until a fetch no longer lists it.
    ClosedPullRequest,
    /// A project whose fetch failed.
    Failed,
    /// A project whose first fetch has not finished yet.
    Loading,
}

/// The pull request a row shows: its project, its number and its web page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrRef {
    /// The repository of the pull request.
    pub project: Project,
    /// The pull request number.
    pub number: PrNumber,
    /// The pull request's web page.
    pub url: PrUrl,
}

impl fmt::Display for PrRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", self.project, self.number)
    }
}

/// One row: its text, what it represents and, for a pull request, which one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayRow {
    /// What the row represents.
    pub kind: RowKind,
    /// The text of each column, in [`HEADER`] order.
    pub cells: [String; 5],
    /// The pull request shown. Only set for [`RowKind::OpenPullRequest`],
    /// [`RowKind::MergedPullRequest`] and [`RowKind::ClosedPullRequest`].
    pub pull_request: Option<PrRef>,
}

impl DisplayRow {
    /// The web page of the row's pull request, if it shows one.
    pub fn url(&self) -> Option<&PrUrl> {
        self.pull_request
            .as_ref()
            .map(|pull_request| &pull_request.url)
    }

    /// The same pull request once merged: its state is `merged`, and whether it could be merged
    /// no longer applies. Other rows stay as they are.
    pub fn merged(self) -> Self {
        self.finished(RowKind::MergedPullRequest, MERGED)
    }

    /// The same pull request once closed: its state is `closed`, and whether it could be merged
    /// no longer applies. Other rows stay as they are.
    pub fn closed(self) -> Self {
        self.finished(RowKind::ClosedPullRequest, CLOSED)
    }

    /// An open pull request turned into `kind`, with `state` and no mergeability.
    fn finished(self, kind: RowKind, state: &str) -> Self {
        match self.kind {
            RowKind::OpenPullRequest => {
                let [project, number, title, _state, _mergeable] = self.cells;
                Self {
                    kind,
                    cells: [
                        project,
                        number,
                        title,
                        state.to_owned(),
                        NO_VALUE.to_owned(),
                    ],
                    ..self
                }
            }
            RowKind::MergedPullRequest
            | RowKind::ClosedPullRequest
            | RowKind::NoPullRequests
            | RowKind::Failed
            | RowKind::Loading => self,
        }
    }
}

/// Computes every row, in report order.
pub fn rows(reports: &[ProjectReport]) -> Vec<DisplayRow> {
    reports.iter().flat_map(report_rows).collect()
}

/// The row of a project that is still loading.
pub fn loading_row(project: &Project) -> DisplayRow {
    message_row(
        RowKind::Loading,
        project.to_string(),
        LOADING_PROJECT.to_owned(),
    )
}

/// The rows of one project's report.
pub fn report_rows(report: &ProjectReport) -> Vec<DisplayRow> {
    let project = report.project.to_string();
    match &report.outcome {
        Ok(pull_requests) if pull_requests.is_empty() => vec![message_row(
            RowKind::NoPullRequests,
            project,
            NO_PULL_REQUESTS.to_owned(),
        )],
        Ok(pull_requests) => pull_requests
            .iter()
            .map(|pull_request| pull_request_row(&report.project, pull_request))
            .collect(),
        Err(error) => vec![message_row(
            RowKind::Failed,
            project,
            format!("error: {error}"),
        )],
    }
}

fn message_row(kind: RowKind, project: String, message: String) -> DisplayRow {
    DisplayRow {
        kind,
        cells: [
            project,
            NO_VALUE.to_owned(),
            message,
            NO_VALUE.to_owned(),
            NO_VALUE.to_owned(),
        ],
        pull_request: None,
    }
}

fn pull_request_row(project: &Project, pull_request: &PullRequest) -> DisplayRow {
    DisplayRow {
        kind: RowKind::OpenPullRequest,
        cells: [
            project.to_string(),
            pull_request.number.to_string(),
            pull_request.title.to_string(),
            pull_request.state.to_string(),
            pull_request.mergeable.to_string(),
        ],
        pull_request: Some(PrRef {
            project: project.clone(),
            number: pull_request.number,
            url: pull_request.url.clone(),
        }),
    }
}

/// Builders of reports shared by the unit tests of the presentation adapters.
#[cfg(test)]
pub(crate) mod fixtures {
    use crate::domain::error::SourceError;
    use crate::domain::model::{
        Mergeable, PrNumber, PrState, PrTitle, PrUrl, PullRequest, RunStatus, WorkflowRun,
    };
    use crate::service::{ProjectReport, RunsReport};

    /// The address of the first sample pull request, `a/b` #1.
    pub const FIRST_URL: &str = "https://github.com/a/b/pull/1";

    /// A report for `project` with the given outcome.
    pub fn report(project: &str, outcome: Result<Vec<PullRequest>, SourceError>) -> ProjectReport {
        ProjectReport {
            project: project.parse().unwrap(),
            outcome,
        }
    }

    /// The runs report of `project`.
    pub fn runs(project: &str, outcome: Result<Vec<WorkflowRun>, SourceError>) -> RunsReport {
        RunsReport {
            project: project.parse().unwrap(),
            outcome,
        }
    }

    /// The workflow run `name` on `main`, triggered by a push.
    pub fn run(name: &str, status: RunStatus) -> WorkflowRun {
        WorkflowRun {
            name: name.to_owned(),
            branch: "main".to_owned(),
            event: "push".to_owned(),
            status,
        }
    }

    /// A mergeable pull request titled `title {number}` at `url`.
    pub fn pull_request(number: u64, state: PrState, url: &str) -> PullRequest {
        PullRequest {
            number: PrNumber::from(number),
            title: PrTitle::from(format!("title {number}")),
            state,
            mergeable: Mergeable::Yes,
            url: PrUrl::from(url.to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{FIRST_URL, pull_request, report};
    use super::*;
    use crate::domain::error::SourceError;
    use crate::domain::model::{Mergeable, PrState, PullRequest};

    /// A row whose pull request, when it has a `url`, is the one of its project and number cells.
    fn row(kind: RowKind, cells: [&str; 5], url: Option<&str>) -> DisplayRow {
        DisplayRow {
            kind,
            pull_request: url.map(|url| PrRef {
                project: cells[0].parse().unwrap(),
                number: PrNumber::from(cells[1].parse::<u64>().unwrap()),
                url: PrUrl::from(url.to_owned()),
            }),
            cells: cells.map(String::from),
        }
    }

    /// The only place that spells out the texts of FR8 and FR9; other tests use the constants.
    #[test]
    fn texts_are_the_ones_of_the_requirements() {
        assert_eq!(HEADER, ["Project", "PR #", "Title", "State", "Mergeable"]);
        assert_eq!(NO_PULL_REQUESTS, "no open pull requests");
        assert_eq!(LOADING_PROJECT, "loading…");
        assert_eq!(MERGED, "merged");
        assert_eq!(CLOSED, "closed");
    }

    #[test]
    fn a_loading_project_gives_one_row() {
        assert_eq!(
            loading_row(&"a/b".parse().unwrap()),
            row(
                RowKind::Loading,
                ["a/b", "—", LOADING_PROJECT, "—", "—"],
                None
            )
        );
    }

    #[test]
    fn no_pull_requests_gives_one_row() {
        assert_eq!(
            rows(&[report("a/b", Ok(vec![]))]),
            [row(
                RowKind::NoPullRequests,
                ["a/b", "—", NO_PULL_REQUESTS, "—", "—"],
                None
            )]
        );
    }

    #[test]
    fn each_pull_request_gives_one_row() {
        let reports = [report(
            "a/b",
            Ok(vec![
                pull_request(1, PrState::Open, FIRST_URL),
                PullRequest {
                    mergeable: Mergeable::No,
                    ..pull_request(2, PrState::Draft, "https://github.com/a/b/pull/2")
                },
                PullRequest {
                    mergeable: Mergeable::Unknown,
                    ..pull_request(3, PrState::Open, "https://github.com/a/b/pull/3")
                },
            ]),
        )];
        assert_eq!(
            rows(&reports),
            [
                row(
                    RowKind::OpenPullRequest,
                    ["a/b", "1", "title 1", "open", "yes"],
                    Some(FIRST_URL)
                ),
                row(
                    RowKind::OpenPullRequest,
                    ["a/b", "2", "title 2", "draft", "no"],
                    Some("https://github.com/a/b/pull/2")
                ),
                row(
                    RowKind::OpenPullRequest,
                    ["a/b", "3", "title 3", "open", "unknown"],
                    Some("https://github.com/a/b/pull/3")
                ),
            ]
        );
    }

    #[test]
    fn error_gives_one_failed_row_with_message() {
        assert_eq!(
            rows(&[report("a/b", Err(SourceError::NotFound))]),
            [row(
                RowKind::Failed,
                ["a/b", "—", "error: repository not found", "—", "—"],
                None
            )]
        );
    }

    #[test]
    fn order_is_preserved_across_projects() {
        let reports = [
            report("z/z", Ok(vec![])),
            report("a/a", Err(SourceError::Unauthorized)),
            report(
                "m/m",
                Ok(vec![pull_request(7, PrState::Open, "https://x/7")]),
            ),
        ];
        let projects: Vec<String> = rows(&reports)
            .into_iter()
            .map(
                |DisplayRow {
                     cells: [project, ..],
                     ..
                 }| project,
            )
            .collect();
        assert_eq!(projects, ["z/z", "a/a", "m/m"]);
    }

    #[test]
    fn a_pull_request_reads_as_project_and_number() {
        let rows = rows(&[report(
            "a/b",
            Ok(vec![pull_request(1, PrState::Open, FIRST_URL)]),
        )]);
        let pull_request = rows[0].pull_request.as_ref().unwrap();
        assert_eq!(pull_request.to_string(), "a/b#1");
        assert_eq!(rows[0].url(), Some(&pull_request.url));
    }

    #[test]
    fn a_merged_or_closed_pull_request_changes_its_kind_state_and_mergeable() {
        let reports = [
            report("a/b", Ok(vec![pull_request(1, PrState::Open, FIRST_URL)])),
            report("c/d", Ok(vec![])),
        ];
        type Finish = fn(DisplayRow) -> DisplayRow;
        let finish: [(Finish, RowKind, &str); 2] = [
            (DisplayRow::merged, RowKind::MergedPullRequest, MERGED),
            (DisplayRow::closed, RowKind::ClosedPullRequest, CLOSED),
        ];
        finish.into_iter().for_each(|(finish, kind, state)| {
            let finished: Vec<DisplayRow> = rows(&reports).into_iter().map(finish).collect();
            assert_eq!(
                finished,
                [
                    row(kind, ["a/b", "1", "title 1", state, "—"], Some(FIRST_URL)),
                    row(
                        RowKind::NoPullRequests,
                        ["c/d", "—", NO_PULL_REQUESTS, "—", "—"],
                        None
                    ),
                ],
                "{state}"
            );
            assert_eq!(
                [DisplayRow::merged, DisplayRow::closed].map(|again| again(finished[0].clone())),
                [finished[0].clone(), finished[0].clone()],
                "a {state} pull request stays {state}"
            );
        });
    }

    #[test]
    fn empty_reports_give_no_rows() {
        assert!(rows(&[]).is_empty());
    }

    #[test]
    fn run_rows_list_unfinished_runs_and_failures() {
        use super::fixtures::{run, runs};
        let reports = [
            runs(
                "a/b",
                Ok(vec![
                    run("CI", RunStatus::InProgress),
                    run("Deploy", RunStatus::Queued),
                ]),
            ),
            runs("c/d", Ok(vec![])),
            runs("e/f", Err(SourceError::Unauthorized)),
        ];
        assert_eq!(
            RUN_HEADER,
            ["Project", "Workflow", "Branch", "Event", "Status"]
        );
        assert_eq!(
            run_rows(&reports),
            [
                RunRow {
                    kind: RunRowKind::Run(RunStatus::InProgress),
                    cells: ["a/b", "CI", "main", "push", "in progress"].map(str::to_owned),
                },
                RunRow {
                    kind: RunRowKind::Run(RunStatus::Queued),
                    cells: ["a/b", "Deploy", "main", "push", "queued"].map(str::to_owned),
                },
                RunRow {
                    kind: RunRowKind::Failed,
                    cells: ["e/f", "error: unauthorized", "—", "—", "—"].map(str::to_owned),
                },
            ]
        );
    }
}
