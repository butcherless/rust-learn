//! Renders project reports as a colored table.

use comfy_table::presets::UTF8_FULL_CONDENSED;
use comfy_table::{Cell, Color, Table};

use crate::domain::model::{PrUrl, PullRequest};
use crate::service::ProjectReport;

/// The column titles.
pub const HEADER: [&str; 4] = ["Project", "PR #", "Title", "State"];
const NO_VALUE: &str = "—";
const NO_PULL_REQUESTS: &str = "no open pull requests";
const ORANGE: Color = Color::Rgb {
    r: 255,
    g: 165,
    b: 0,
};

/// What a table row represents, which decides its color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// A project without open pull requests.
    NoPullRequests,
    /// An open pull request.
    OpenPullRequest,
    /// A project whose fetch failed.
    Failed,
}

/// One table row: its text, what it represents and, for a pull request, its address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayRow {
    /// What the row represents.
    pub kind: RowKind,
    /// The text of each column, in [`HEADER`] order.
    pub cells: [String; 4],
    /// The pull request's web page. Only set for [`RowKind::OpenPullRequest`].
    pub url: Option<PrUrl>,
}

/// Computes every table row, in report order.
pub fn rows(reports: &[ProjectReport]) -> Vec<DisplayRow> {
    reports.iter().flat_map(report_rows).collect()
}

fn report_rows(report: &ProjectReport) -> Vec<DisplayRow> {
    let project = report.project.to_string();
    match &report.outcome {
        Ok(pull_requests) if pull_requests.is_empty() => vec![DisplayRow {
            kind: RowKind::NoPullRequests,
            cells: message_cells(project, NO_PULL_REQUESTS.to_owned()),
            url: None,
        }],
        Ok(pull_requests) => pull_requests
            .iter()
            .map(|pull_request| DisplayRow {
                kind: RowKind::OpenPullRequest,
                cells: pull_request_cells(project.clone(), pull_request),
                url: Some(pull_request.url.clone()),
            })
            .collect(),
        Err(error) => vec![DisplayRow {
            kind: RowKind::Failed,
            cells: message_cells(project, format!("error: {error}")),
            url: None,
        }],
    }
}

fn message_cells(project: String, message: String) -> [String; 4] {
    [project, NO_VALUE.to_owned(), message, NO_VALUE.to_owned()]
}

fn pull_request_cells(project: String, pull_request: &PullRequest) -> [String; 4] {
    [
        project,
        pull_request.number.to_string(),
        pull_request.title.to_string(),
        pull_request.state.to_string(),
    ]
}

fn color(kind: RowKind) -> Color {
    match kind {
        RowKind::NoPullRequests => Color::Green,
        RowKind::OpenPullRequest => ORANGE,
        RowKind::Failed => Color::Red,
    }
}

/// Builds the table. Styling is dropped when stdout is not a TTY.
pub fn render(reports: &[ProjectReport]) -> Table {
    let mut table = Table::new();
    table
        .load_style(UTF8_FULL_CONDENSED.with_rounded_corners())
        .set_header(HEADER);
    rows(reports).into_iter().for_each(|row| {
        let color = color(row.kind);
        table.add_row(row.cells.map(|text| Cell::new(text).fg(color)));
    });
    table
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::error::SourceError;
    use crate::domain::model::{PrNumber, PrState, PrTitle};

    fn row(kind: RowKind, cells: [&str; 4], url: Option<&str>) -> DisplayRow {
        DisplayRow {
            kind,
            cells: cells.map(String::from),
            url: url.map(|url| PrUrl(url.to_owned())),
        }
    }

    fn report(project: &str, outcome: Result<Vec<PullRequest>, SourceError>) -> ProjectReport {
        ProjectReport {
            project: project.parse().unwrap(),
            outcome,
        }
    }

    fn pull_request(number: u64, state: PrState) -> PullRequest {
        PullRequest {
            number: PrNumber(number),
            title: PrTitle(format!("title {number}")),
            state,
            url: PrUrl(format!("https://github.com/a/b/pull/{number}")),
        }
    }

    #[test]
    fn no_pull_requests_gives_one_row() {
        assert_eq!(
            rows(&[report("a/b", Ok(vec![]))]),
            [row(
                RowKind::NoPullRequests,
                ["a/b", "—", "no open pull requests", "—"],
                None
            )]
        );
    }

    #[test]
    fn each_pull_request_gives_one_row() {
        let reports = [report(
            "a/b",
            Ok(vec![
                pull_request(1, PrState::Open),
                pull_request(2, PrState::Draft),
            ]),
        )];
        assert_eq!(
            rows(&reports),
            [
                row(
                    RowKind::OpenPullRequest,
                    ["a/b", "1", "title 1", "open"],
                    Some("https://github.com/a/b/pull/1")
                ),
                row(
                    RowKind::OpenPullRequest,
                    ["a/b", "2", "title 2", "draft"],
                    Some("https://github.com/a/b/pull/2")
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
                ["a/b", "—", "error: repository not found", "—"],
                None
            )]
        );
    }

    #[test]
    fn order_is_preserved_across_projects() {
        let reports = [
            report("z/z", Ok(vec![])),
            report("a/a", Err(SourceError::Unauthorized)),
            report("m/m", Ok(vec![pull_request(7, PrState::Open)])),
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
    fn empty_reports_render_only_the_header() {
        let output = render(&[]).to_string();
        assert!(output.contains("Project"));
        assert!(rows(&[]).is_empty());
    }
}
