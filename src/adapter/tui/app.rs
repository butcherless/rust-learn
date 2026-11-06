//! State, key handling and drawing of the terminal UI. Free of I/O, so it is tested directly.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Row, Table, TableState};

use crate::adapter::cli::render::{DisplayRow, HEADER, RowKind, rows};
use crate::domain::model::PrUrl;
use crate::service::ProjectReport;

const TITLE: &str = " Open pull requests ";
const HELP: &str = "↑/↓ move · Enter open in browser · r refresh · q quit";
const LOADING: &str = "Loading…";
const NOT_A_PULL_REQUEST: &str = "This row is not a pull request";
const SAFE_SCHEME: &str = "https://";
const ORANGE: Color = Color::Rgb(255, 165, 0);
const NUMBER_WIDTH: u16 = 6;
const STATE_WIDTH: u16 = 6;

/// What the event loop must do after a key press.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Nothing beyond redrawing.
    None,
    /// Leave the UI.
    Quit,
    /// Fetch the pull requests again.
    Refresh,
    /// Open this pull request in the browser.
    Open(PrUrl),
}

/// The terminal UI's state.
#[derive(Debug, Default)]
pub struct App {
    reports: Vec<ProjectReport>,
    rows: Vec<DisplayRow>,
    table: TableState,
    status: String,
}

impl App {
    /// An empty UI that is waiting for its first reports.
    pub fn new() -> Self {
        Self {
            status: LOADING.to_owned(),
            ..Self::default()
        }
    }

    /// Shows that a fetch is in progress.
    pub fn start_loading(&mut self) {
        LOADING.clone_into(&mut self.status);
    }

    /// Replaces the shown reports, keeping the selection on the same line when it still exists.
    pub fn set_reports(&mut self, reports: Vec<ProjectReport>) {
        self.rows = rows(&reports);
        self.reports = reports;
        let selected = match (self.table.selected(), self.rows.len()) {
            (_, 0) => None,
            (Some(index), len) => Some(index.min(len - 1)),
            (None, _) => Some(0),
        };
        self.table.select(selected);
        self.status = summary(&self.reports);
    }

    /// Records whether the browser could be launched for `url`.
    pub fn opened(&mut self, url: &PrUrl, result: std::io::Result<()>) {
        self.status = match result {
            Ok(()) => format!("Opened {url}"),
            Err(error) => format!("Cannot open the browser: {error}"),
        };
    }

    /// The index of the selected row.
    pub fn selected(&self) -> Option<usize> {
        self.table.selected()
    }

    /// The status line's text.
    pub fn status(&self) -> &str {
        &self.status
    }

    /// The latest reports, which decide the exit code.
    pub fn into_reports(self) -> Vec<ProjectReport> {
        self.reports
    }

    /// Updates the state for `key` and tells the event loop what to do next.
    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::Quit,
            KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
            KeyCode::Char('r') => Action::Refresh,
            KeyCode::Down | KeyCode::Char('j') => self.move_by(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_by(-1),
            KeyCode::Home | KeyCode::Char('g') => self.select(0),
            KeyCode::End | KeyCode::Char('G') => self.select(self.rows.len().saturating_sub(1)),
            KeyCode::Enter | KeyCode::Char('o') => self.open_selected(),
            _ => Action::None,
        }
    }

    fn move_by(&mut self, step: isize) -> Action {
        let current = self.table.selected().unwrap_or(0);
        self.select(current.saturating_add_signed(step))
    }

    fn select(&mut self, index: usize) -> Action {
        if let Some(last) = self.rows.len().checked_sub(1) {
            self.table.select(Some(index.min(last)));
        }
        Action::None
    }

    fn open_selected(&mut self) -> Action {
        let url = self
            .table
            .selected()
            .and_then(|index| self.rows.get(index))
            .and_then(|row| row.url.clone());
        match url {
            // The address comes from the API, so only web pages are handed to the system opener.
            Some(url) if url.as_str().starts_with(SAFE_SCHEME) => Action::Open(url),
            Some(url) => {
                self.status = format!("Not opening an address that is not https: {url}");
                Action::None
            }
            None => {
                NOT_A_PULL_REQUEST.clone_into(&mut self.status);
                Action::None
            }
        }
    }

    /// Draws the whole screen.
    pub fn draw(&mut self, frame: &mut Frame) {
        let layout = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ]);
        let [table_area, status_area, help_area] = layout.areas(frame.area());

        let project_width = self
            .rows
            .iter()
            .map(|row| row.cells[0].chars().count())
            .chain([HEADER[0].len()])
            .max()
            .and_then(|width| u16::try_from(width).ok())
            .unwrap_or(u16::MAX);
        let widths = [
            Constraint::Length(project_width),
            Constraint::Length(NUMBER_WIDTH),
            Constraint::Fill(1),
            Constraint::Length(STATE_WIDTH),
        ];
        let header = Row::new(HEADER).style(Style::new().bold());
        let body = self
            .rows
            .iter()
            .map(|row| Row::new(row.cells.clone()).style(Style::new().fg(color(row.kind))));
        let table = Table::new(body, widths)
            .header(header)
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .title(TITLE),
            )
            .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(table, table_area, &mut self.table);
        frame.render_widget(Line::from(self.status.as_str()), status_area);
        frame.render_widget(Line::from(HELP).dim(), help_area);
    }
}

fn color(kind: RowKind) -> Color {
    match kind {
        RowKind::NoPullRequests => Color::Green,
        RowKind::OpenPullRequest => ORANGE,
        RowKind::Failed => Color::Red,
    }
}

fn summary(reports: &[ProjectReport]) -> String {
    let pull_requests: usize = reports
        .iter()
        .filter_map(|report| report.outcome.as_ref().ok())
        .map(Vec::len)
        .sum();
    let failures = reports
        .iter()
        .filter(|report| report.outcome.is_err())
        .count();
    let mut text = format!(
        "{pull_requests} open pull requests in {} projects",
        reports.len()
    );
    if failures > 0 {
        text.push_str(&format!(", {failures} failed"));
    }
    text
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::domain::error::SourceError;
    use crate::domain::model::{PrNumber, PrState, PrTitle, PullRequest};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn pull_request(number: u64, url: &str) -> PullRequest {
        PullRequest {
            number: PrNumber(number),
            title: PrTitle(format!("title {number}")),
            state: PrState::Open,
            url: PrUrl(url.to_owned()),
        }
    }

    /// Rows: 0 = a/b PR #1, 1 = a/b PR #2, 2 = c/d without PRs, 3 = e/f failed.
    fn app() -> App {
        let mut app = App::new();
        app.set_reports(vec![
            ProjectReport {
                project: "a/b".parse().unwrap(),
                outcome: Ok(vec![
                    pull_request(1, "https://github.com/a/b/pull/1"),
                    pull_request(2, "http://github.com/a/b/pull/2"),
                ]),
            },
            ProjectReport {
                project: "c/d".parse().unwrap(),
                outcome: Ok(vec![]),
            },
            ProjectReport {
                project: "e/f".parse().unwrap(),
                outcome: Err(SourceError::NotFound),
            },
        ]);
        app
    }

    fn screen(app: &mut App) -> String {
        let mut terminal = Terminal::new(TestBackend::new(70, 10)).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn starts_loading_without_selection() {
        let app = App::new();
        assert_eq!(app.status(), LOADING);
        assert_eq!(app.selected(), None);
    }

    #[test]
    fn reports_select_the_first_row_and_summarize() {
        let app = app();
        assert_eq!(app.selected(), Some(0));
        assert_eq!(app.status(), "2 open pull requests in 3 projects, 1 failed");
    }

    #[test]
    fn movement_stays_within_the_rows() {
        let mut app = app();
        assert_eq!(app.handle_key(key(KeyCode::Up)), Action::None);
        assert_eq!(app.selected(), Some(0));
        app.handle_key(key(KeyCode::Char('G')));
        assert_eq!(app.selected(), Some(3));
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.selected(), Some(3));
        app.handle_key(key(KeyCode::Char('k')));
        assert_eq!(app.selected(), Some(2));
        app.handle_key(key(KeyCode::Home));
        assert_eq!(app.selected(), Some(0));
    }

    #[test]
    fn enter_on_a_pull_request_opens_its_url() {
        let mut app = app();
        assert_eq!(
            app.handle_key(key(KeyCode::Enter)),
            Action::Open(PrUrl("https://github.com/a/b/pull/1".to_owned()))
        );
    }

    #[test]
    fn enter_refuses_addresses_that_are_not_https() {
        let mut app = app();
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.handle_key(key(KeyCode::Enter)), Action::None);
        assert!(app.status().starts_with("Not opening"));
    }

    #[test]
    fn enter_on_other_rows_does_nothing() {
        let mut app = app();
        app.handle_key(key(KeyCode::End));
        assert_eq!(app.handle_key(key(KeyCode::Enter)), Action::None);
        assert_eq!(app.status(), NOT_A_PULL_REQUEST);
    }

    #[test]
    fn quit_and_refresh_keys() {
        let mut app = app();
        assert_eq!(app.handle_key(key(KeyCode::Char('q'))), Action::Quit);
        assert_eq!(app.handle_key(key(KeyCode::Esc)), Action::Quit);
        assert_eq!(
            app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Action::Quit
        );
        assert_eq!(app.handle_key(key(KeyCode::Char('r'))), Action::Refresh);
    }

    #[test]
    fn refresh_keeps_the_selection_when_possible() {
        let mut app = app();
        app.handle_key(key(KeyCode::End));
        app.set_reports(vec![ProjectReport {
            project: "c/d".parse().unwrap(),
            outcome: Ok(vec![]),
        }]);
        assert_eq!(app.selected(), Some(0));
        app.set_reports(vec![]);
        assert_eq!(app.selected(), None);
        assert_eq!(app.handle_key(key(KeyCode::Enter)), Action::None);
    }

    #[test]
    fn open_result_is_shown() {
        let mut app = app();
        let url = PrUrl("https://github.com/a/b/pull/1".to_owned());
        app.opened(&url, Ok(()));
        assert_eq!(app.status(), "Opened https://github.com/a/b/pull/1");
        app.opened(&url, Err(std::io::Error::other("no browser")));
        assert_eq!(app.status(), "Cannot open the browser: no browser");
    }

    #[test]
    fn draws_rows_status_and_help() {
        let mut app = app();
        let screen = screen(&mut app);
        [
            "Open pull requests",
            "Project",
            "title 1",
            "no open pull requests",
        ]
        .into_iter()
        .chain([
            "error: repository not found",
            "2 open pull requests",
            "q quit",
        ])
        .for_each(|text| assert!(screen.contains(text), "missing {text:?} in {screen}"));
    }
}
