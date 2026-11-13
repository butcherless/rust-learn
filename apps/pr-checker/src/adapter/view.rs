//! Presentation model shared by the table and the terminal UI: which rows to show, their text,
//! their kind and their pull request address. Pure, and free of any rendering library.

use crate::domain::model::{PrUrl, Project, PullRequest};
use crate::service::ProjectReport;

/// The column titles.
pub const HEADER: [&str; 5] = ["Project", "PR #", "Title", "State", "Mergeable"];
/// The orange of open pull request rows, as RGB.
pub const ORANGE: (u8, u8, u8) = (255, 165, 0);
const NO_VALUE: &str = "—";
/// The message of a project without open pull requests (FR9).
pub const NO_PULL_REQUESTS: &str = "no open pull requests";
/// The message of a project whose first fetch has not finished yet (FR14).
pub const LOADING_PROJECT: &str = "loading…";

/// What a row represents, which decides its color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// A project without open pull requests.
    NoPullRequests,
    /// An open pull request.
    OpenPullRequest,
    /// A project whose fetch failed.
    Failed,
    /// A project whose first fetch has not finished yet.
    Loading,
}

/// One row: its text, what it represents and, for a pull request, its address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayRow {
    /// What the row represents.
    pub kind: RowKind,
    /// The text of each column, in [`HEADER`] order.
    pub cells: [String; 5],
    /// The pull request's web page. Only set for [`RowKind::OpenPullRequest`].
    pub url: Option<PrUrl>,
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
            .map(|pull_request| pull_request_row(project.clone(), pull_request))
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
        url: None,
    }
}

fn pull_request_row(project: String, pull_request: &PullRequest) -> DisplayRow {
    DisplayRow {
        kind: RowKind::OpenPullRequest,
        cells: [
            project,
            pull_request.number.to_string(),
            pull_request.title.to_string(),
            pull_request.state.to_string(),
            pull_request.mergeable.to_string(),
        ],
        url: Some(pull_request.url.clone()),
    }
}

/// Builders of reports shared by the unit tests of the presentation adapters.
#[cfg(test)]
pub(crate) mod fixtures {
    use crate::domain::error::SourceError;
    use crate::domain::model::{Mergeable, PrNumber, PrState, PrTitle, PrUrl, PullRequest};
    use crate::service::ProjectReport;

    /// The address of the first sample pull request, `a/b` #1.
    pub const FIRST_URL: &str = "https://github.com/a/b/pull/1";

    /// A report for `project` with the given outcome.
    pub fn report(project: &str, outcome: Result<Vec<PullRequest>, SourceError>) -> ProjectReport {
        ProjectReport {
            project: project.parse().unwrap(),
            outcome,
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

    fn row(kind: RowKind, cells: [&str; 5], url: Option<&str>) -> DisplayRow {
        DisplayRow {
            kind,
            cells: cells.map(String::from),
            url: url.map(|url| PrUrl::from(url.to_owned())),
        }
    }

    /// The only place that spells out the texts of FR8 and FR9; other tests use the constants.
    #[test]
    fn texts_are_the_ones_of_the_requirements() {
        assert_eq!(HEADER, ["Project", "PR #", "Title", "State", "Mergeable"]);
        assert_eq!(NO_PULL_REQUESTS, "no open pull requests");
        assert_eq!(LOADING_PROJECT, "loading…");
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
    fn empty_reports_give_no_rows() {
        assert!(rows(&[]).is_empty());
    }
}
