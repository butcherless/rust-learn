//! The terminal UI's model, following The Elm Architecture: [`message`] turns a key into a
//! [`Message`], [`App::update`] turns the current state and a message into the next state and an
//! [`Effect`], and [`App::view`] draws a state. All three are free of I/O, so they are tested
//! directly; the event loop in the parent module performs the effects.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Row, Table, TableState};

use crate::adapter::view::{DisplayRow, HEADER, ORANGE, RowKind, rows};
use crate::domain::model::PrUrl;
use crate::service::ProjectReport;

const TITLE: &str = " Open pull requests ";
const HELP: &str = "↑/↓ move · Enter open in browser · r refresh · q quit";
const LOADING: &str = "Loading…";
const NOT_A_PULL_REQUEST: &str = "This row is not a pull request";
const SAFE_SCHEME: &str = "https://";
const HIGHLIGHT_SYMBOL: &str = "▶ ";
const NUMBER_WIDTH: u16 = 6;
const STATE_WIDTH: u16 = 6;

/// A change of the selected row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    /// One row up.
    Up,
    /// One row down.
    Down,
    /// The first row.
    First,
    /// The last row.
    Last,
}

/// Something that happened: a key press, or the result of an [`Effect`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// Change the selected row.
    Move(Move),
    /// Open the selected pull request in the browser.
    OpenSelected,
    /// Fetch the pull requests again.
    Refresh,
    /// Leave the UI.
    Quit,
    /// A fetch finished.
    Loaded(Vec<ProjectReport>),
    /// The browser was launched for this address, or could not be.
    Opened(PrUrl, Result<(), String>),
}

/// Work with side effects that the event loop must do after an update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Nothing: draw and wait for the next key.
    None,
    /// Fetch every project, then send [`Message::Loaded`].
    Fetch,
    /// Launch the browser, then send [`Message::Opened`].
    Open(PrUrl),
    /// Restore the terminal and leave.
    Quit,
}

/// What the status line reports.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum Status {
    #[default]
    Loading,
    Summary,
    Opened(PrUrl),
    OpenFailed(String),
    NotAPullRequest,
    Unsafe(PrUrl),
}

/// The terminal UI's state. Each update consumes it and returns the next one.
///
/// The default state has no reports, no selection, and shows that loading is in progress.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct App {
    reports: Vec<ProjectReport>,
    selected: Option<usize>,
    status: Status,
}

/// Maps a key press to a message. Unbound keys give `None`.
pub fn message(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        KeyCode::Char('q') | KeyCode::Esc => Some(Message::Quit),
        KeyCode::Char('r') => Some(Message::Refresh),
        KeyCode::Enter | KeyCode::Char('o') => Some(Message::OpenSelected),
        KeyCode::Up | KeyCode::Char('k') => Some(Message::Move(Move::Up)),
        KeyCode::Down | KeyCode::Char('j') => Some(Message::Move(Move::Down)),
        KeyCode::Home | KeyCode::Char('g') => Some(Message::Move(Move::First)),
        KeyCode::End | KeyCode::Char('G') => Some(Message::Move(Move::Last)),
        _ => None,
    }
}

impl App {
    /// The index of the selected row.
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// The status line's text.
    pub fn status(&self) -> String {
        match &self.status {
            Status::Loading => LOADING.to_owned(),
            Status::Summary => summary(&self.reports),
            Status::Opened(url) => format!("Opened {url}"),
            Status::OpenFailed(error) => format!("Cannot open the browser: {error}"),
            Status::NotAPullRequest => NOT_A_PULL_REQUEST.to_owned(),
            Status::Unsafe(url) => format!("Not opening an address that is not https: {url}"),
        }
    }

    /// The latest reports, which decide the exit code.
    pub fn into_reports(self) -> Vec<ProjectReport> {
        self.reports
    }

    /// Computes the next state for `message`, and the effect the event loop must perform.
    pub fn update(self, message: Message) -> (Self, Effect) {
        match message {
            Message::Quit => (self, Effect::Quit),
            Message::Refresh => (self.with_status(Status::Loading), Effect::Fetch),
            Message::Move(step) => (self.moved(step), Effect::None),
            Message::OpenSelected => self.open_selected(),
            Message::Loaded(reports) => (self.loaded(reports), Effect::None),
            Message::Opened(url, Ok(())) => (self.with_status(Status::Opened(url)), Effect::None),
            Message::Opened(_, Err(error)) => {
                (self.with_status(Status::OpenFailed(error)), Effect::None)
            }
        }
    }

    fn with_status(self, status: Status) -> Self {
        Self { status, ..self }
    }

    fn loaded(self, reports: Vec<ProjectReport>) -> Self {
        // Keeps the selection on the same line when it still exists.
        let selected = clamp(self.selected.unwrap_or(0), rows(&reports).len());
        Self {
            reports,
            selected,
            status: Status::Summary,
        }
    }

    fn moved(self, step: Move) -> Self {
        let current = self.selected.unwrap_or(0);
        let target = match step {
            Move::Up => current.saturating_sub(1),
            Move::Down => current.saturating_add(1),
            Move::First => 0,
            Move::Last => usize::MAX,
        };
        let selected = clamp(target, rows(&self.reports).len());
        Self { selected, ..self }
    }

    fn open_selected(self) -> (Self, Effect) {
        let url = self
            .selected
            .and_then(|index| rows(&self.reports).into_iter().nth(index))
            .and_then(|row| row.url);
        match url {
            // The address comes from the API, so only web pages are handed to the system opener.
            Some(url) if url.as_str().starts_with(SAFE_SCHEME) => (self, Effect::Open(url)),
            Some(url) => (self.with_status(Status::Unsafe(url)), Effect::None),
            None => (self.with_status(Status::NotAPullRequest), Effect::None),
        }
    }

    /// Draws the whole screen.
    pub fn view(&self, frame: &mut Frame) {
        let [table_area, status_area, help_area] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        let display_rows = rows(&self.reports);
        // The selection is part of the model; ratatui derives the scroll offset from it.
        let mut table_state = TableState::default().with_selected(self.selected);

        frame.render_stateful_widget(table(&display_rows), table_area, &mut table_state);
        frame.render_widget(Line::from(self.status()), status_area);
        frame.render_widget(Line::from(HELP).dim(), help_area);
    }
}

/// The index `index` limited to the last of `len` rows, or `None` when there are no rows.
fn clamp(index: usize, len: usize) -> Option<usize> {
    len.checked_sub(1).map(|last| index.min(last))
}

fn table(display_rows: &[DisplayRow]) -> Table<'static> {
    let body = display_rows
        .iter()
        .map(|row| Row::new(row.cells.clone()).style(Style::new().fg(color(row.kind))));
    Table::new(body, column_widths(display_rows))
        .header(Row::new(HEADER).style(Style::new().bold()))
        .block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .title(TITLE),
        )
        .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(HIGHLIGHT_SYMBOL)
}

/// The project column fits its longest entry; the title takes the remaining space.
fn column_widths(display_rows: &[DisplayRow]) -> [Constraint; 4] {
    let project_width = display_rows
        .iter()
        .map(|row| row.cells[0].chars().count())
        .chain([HEADER[0].len()])
        .max()
        .and_then(|width| u16::try_from(width).ok())
        .unwrap_or(u16::MAX);
    [
        Constraint::Length(project_width),
        Constraint::Length(NUMBER_WIDTH),
        Constraint::Fill(1),
        Constraint::Length(STATE_WIDTH),
    ]
}

fn color(kind: RowKind) -> Color {
    match kind {
        RowKind::NoPullRequests => Color::Green,
        RowKind::OpenPullRequest => {
            let (r, g, b) = ORANGE;
            Color::Rgb(r, g, b)
        }
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
    let counts = format!(
        "{pull_requests} open pull requests in {} projects",
        reports.len()
    );
    match failures {
        0 => counts,
        failures => format!("{counts}, {failures} failed"),
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::adapter::view::fixtures::{pull_request, report};
    use crate::domain::error::SourceError;
    use crate::domain::model::PrState;

    const FIRST_URL: &str = "https://github.com/a/b/pull/1";

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn url(text: &str) -> PrUrl {
        PrUrl(text.to_owned())
    }

    /// Rows: 0 = a/b PR #1, 1 = a/b PR #2 (http), 2 = c/d without PRs, 3 = e/f failed.
    fn reports() -> Vec<ProjectReport> {
        vec![
            report(
                "a/b",
                Ok(vec![
                    pull_request(1, PrState::Open, FIRST_URL),
                    pull_request(2, PrState::Open, "http://github.com/a/b/pull/2"),
                ]),
            ),
            report("c/d", Ok(vec![])),
            report("e/f", Err(SourceError::NotFound)),
        ]
    }

    /// Applies `messages` in order, returning the final state and the last effect.
    fn run(app: App, messages: impl IntoIterator<Item = Message>) -> (App, Effect) {
        messages
            .into_iter()
            .fold((app, Effect::None), |(app, _), message| app.update(message))
    }

    fn loaded() -> App {
        run(App::default(), [Message::Loaded(reports())]).0
    }

    fn moves(steps: impl IntoIterator<Item = Move>) -> App {
        run(loaded(), steps.into_iter().map(Message::Move)).0
    }

    fn screen(app: &App) -> String {
        let mut terminal = Terminal::new(TestBackend::new(70, 10)).unwrap();
        terminal.draw(|frame| app.view(frame)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn keys_map_to_messages() {
        [
            (key(KeyCode::Char('q')), Some(Message::Quit)),
            (key(KeyCode::Esc), Some(Message::Quit)),
            (
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                Some(Message::Quit),
            ),
            (key(KeyCode::Char('c')), None),
            (key(KeyCode::Char('r')), Some(Message::Refresh)),
            (key(KeyCode::Enter), Some(Message::OpenSelected)),
            (key(KeyCode::Char('o')), Some(Message::OpenSelected)),
            (key(KeyCode::Up), Some(Message::Move(Move::Up))),
            (key(KeyCode::Char('j')), Some(Message::Move(Move::Down))),
            (key(KeyCode::Home), Some(Message::Move(Move::First))),
            (key(KeyCode::Char('G')), Some(Message::Move(Move::Last))),
            (key(KeyCode::Tab), None),
        ]
        .into_iter()
        .for_each(|(key, expected)| assert_eq!(message(key), expected, "key: {key:?}"));
    }

    #[test]
    fn starts_loading_without_selection() {
        let app = App::default();
        assert_eq!(app.status(), LOADING);
        assert_eq!(app.selected(), None);
    }

    #[test]
    fn loading_selects_the_first_row_and_summarizes() {
        let app = loaded();
        assert_eq!(app.selected(), Some(0));
        assert_eq!(app.status(), "2 open pull requests in 3 projects, 1 failed");
    }

    #[test]
    fn summary_omits_failures_when_there_are_none() {
        let app = run(
            App::default(),
            [Message::Loaded(vec![report("c/d", Ok(vec![]))])],
        )
        .0;
        assert_eq!(app.status(), "0 open pull requests in 1 projects");
    }

    #[test]
    fn movement_stays_within_the_rows() {
        assert_eq!(moves([Move::Up]).selected(), Some(0));
        assert_eq!(moves([Move::Last]).selected(), Some(3));
        assert_eq!(moves([Move::Last, Move::Down]).selected(), Some(3));
        assert_eq!(moves([Move::Last, Move::Up]).selected(), Some(2));
        assert_eq!(moves([Move::Last, Move::First]).selected(), Some(0));
    }

    #[test]
    fn movement_without_rows_selects_nothing() {
        let (app, effect) = App::default().update(Message::Move(Move::Down));
        assert_eq!(app.selected(), None);
        assert_eq!(effect, Effect::None);
    }

    #[test]
    fn opening_a_pull_request_asks_for_the_browser() {
        let (_, effect) = loaded().update(Message::OpenSelected);
        assert_eq!(effect, Effect::Open(url(FIRST_URL)));
    }

    #[test]
    fn addresses_that_are_not_https_are_refused() {
        let (app, effect) = moves([Move::Down]).update(Message::OpenSelected);
        assert_eq!(effect, Effect::None);
        assert!(app.status().starts_with("Not opening"));
    }

    #[test]
    fn other_rows_cannot_be_opened() {
        let (app, effect) = moves([Move::Last]).update(Message::OpenSelected);
        assert_eq!(effect, Effect::None);
        assert_eq!(app.status(), NOT_A_PULL_REQUEST);
    }

    #[test]
    fn refresh_shows_loading_and_asks_for_a_fetch() {
        let (app, effect) = loaded().update(Message::Refresh);
        assert_eq!(effect, Effect::Fetch);
        assert_eq!(app.status(), LOADING);
    }

    #[test]
    fn into_reports_returns_the_last_loaded_reports() {
        let shorter = vec![report("c/d", Ok(vec![]))];
        let app = run(loaded(), [Message::Loaded(shorter.clone())]).0;
        assert_eq!(app.into_reports(), shorter);
    }

    #[test]
    fn quit_asks_to_leave() {
        assert_eq!(loaded().update(Message::Quit).1, Effect::Quit);
    }

    #[test]
    fn reloading_keeps_the_selection_when_possible() {
        let shorter = vec![report("c/d", Ok(vec![]))];
        let app = run(moves([Move::Last]), [Message::Loaded(shorter)]).0;
        assert_eq!(app.selected(), Some(0));
        let (app, effect) = run(app, [Message::Loaded(vec![]), Message::OpenSelected]);
        assert_eq!(app.selected(), None);
        assert_eq!(effect, Effect::None);
    }

    #[test]
    fn open_results_are_shown() {
        let opened = loaded().update(Message::Opened(url(FIRST_URL), Ok(()))).0;
        assert_eq!(opened.status(), format!("Opened {FIRST_URL}"));
        let failed = loaded()
            .update(Message::Opened(
                url(FIRST_URL),
                Err("no browser".to_owned()),
            ))
            .0;
        assert_eq!(failed.status(), "Cannot open the browser: no browser");
    }

    #[test]
    fn draws_rows_status_and_help() {
        let screen = screen(&loaded());
        [
            "Open pull requests",
            "Project",
            "title 1",
            "no open pull requests",
            "error: repository not found",
            "2 open pull requests",
            "q quit",
        ]
        .into_iter()
        .for_each(|text| assert!(screen.contains(text), "missing {text:?} in {screen}"));
    }
}
