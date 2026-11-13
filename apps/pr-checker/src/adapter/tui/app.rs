//! The terminal UI's model, following The Elm Architecture: [`message`] turns a key into a
//! [`Message`], [`App::update`] turns the current state and a message into the next state and an
//! [`Effect`], and [`super::view::draw`] draws a state. All three are free of I/O, so they are
//! tested directly; the event loop in the parent module performs the effects.

use std::iter;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::adapter::view::{DisplayRow, loading_row, report_rows};
use crate::domain::model::{PrUrl, Project};
use crate::service::ProjectReport;

/// The status line while fetching, before the progress.
pub const LOADING: &str = "Loading…";
/// The status line when the selected row has no pull request to open.
pub const NOT_A_PULL_REQUEST: &str = "This row is not a pull request";
const SAFE_SCHEME: &str = "https://";

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

/// Something that happened: a key press, the clock, or the result of an [`Effect`].
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
    /// Half a second passed while fetching: the light blinks (FR13).
    Tick,
    /// The project at this position of the list finished fetching (FR14).
    Fetched(usize, ProjectReport),
    /// Every project finished fetching.
    Finished,
    /// The browser was launched for this address, or could not be.
    Opened(PrUrl, Result<(), String>),
}

/// Work with side effects that the event loop must do after an update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Nothing: draw and wait for the next message.
    None,
    /// Fetch every project, sending [`Message::Fetched`] for each one and then
    /// [`Message::Finished`], and [`Message::Tick`] every half second meanwhile.
    Fetch,
    /// Launch the browser, then send [`Message::Opened`].
    Open(PrUrl),
    /// Restore the terminal and leave.
    Quit,
}

/// The light at the start of the status line (FR13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Light {
    /// Fetching, in the lit half of the blink.
    On,
    /// Fetching, in the dark half of the blink.
    Off,
    /// Idle, and every project succeeded.
    Done,
    /// Idle, and at least one project failed.
    Failed,
}

/// What the status line reports.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Status {
    Loading,
    Summary,
    Opened(PrUrl),
    OpenFailed(String),
    NotAPullRequest,
    Unsafe(PrUrl),
}

/// One project of the list: still waiting for its first report, or with its latest one.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Slot {
    Pending(Project),
    Done(ProjectReport),
}

/// A fetch in progress: how many projects have arrived, and the blink's half.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Progress {
    received: usize,
    lit: bool,
}

/// The terminal UI's state. Each update consumes it and returns the next one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    /// One slot per project, in the order of the configuration.
    slots: Vec<Slot>,
    /// The rows of `slots`, computed when a slot changes instead of on every key and draw.
    rows: Vec<DisplayRow>,
    selected: Option<usize>,
    status: Status,
    fetch: Option<Progress>,
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
    /// The state before the first fetch, with one loading row per project, and the fetch to
    /// start.
    pub fn new(projects: Vec<Project>) -> (Self, Effect) {
        let app = Self {
            slots: Vec::new(),
            rows: Vec::new(),
            selected: None,
            status: Status::Loading,
            fetch: None,
        };
        let slots = projects.into_iter().map(Slot::Pending).collect();
        (app.with_slots(slots).fetching(), Effect::Fetch)
    }

    /// The rows shown: one per pull request, or one per project without any, failed or loading.
    pub fn rows(&self) -> &[DisplayRow] {
        &self.rows
    }

    /// The index of the selected row.
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// The light at the start of the status line.
    pub fn light(&self) -> Light {
        match self.fetch {
            Some(Progress { lit: true, .. }) => Light::On,
            Some(Progress { lit: false, .. }) => Light::Off,
            None if self.reports().any(|report| report.outcome.is_err()) => Light::Failed,
            None => Light::Done,
        }
    }

    /// The status line's text.
    pub fn status(&self) -> String {
        match &self.status {
            Status::Loading => format!(
                "{LOADING} {}/{}",
                self.fetch.map_or(0, |progress| progress.received),
                self.slots.len()
            ),
            Status::Summary => summary(&self.reports().collect::<Vec<_>>()),
            Status::Opened(url) => format!("Opened {url}"),
            Status::OpenFailed(error) => format!("Cannot open the browser: {error}"),
            Status::NotAPullRequest => NOT_A_PULL_REQUEST.to_owned(),
            Status::Unsafe(url) => format!("Not opening an address that is not https: {url}"),
        }
    }

    /// The latest report of each project that has one, which decide the exit code.
    pub fn into_reports(self) -> Vec<ProjectReport> {
        self.slots
            .into_iter()
            .filter_map(|slot| match slot {
                Slot::Pending(_) => None,
                Slot::Done(report) => Some(report),
            })
            .collect()
    }

    /// Computes the next state for `message`, and the effect the event loop must perform.
    pub fn update(self, message: Message) -> (Self, Effect) {
        match message {
            Message::Quit => (self, Effect::Quit),
            // A fetch in progress is not restarted.
            Message::Refresh if self.fetch.is_some() => (self, Effect::None),
            Message::Refresh => (self.fetching(), Effect::Fetch),
            Message::Move(step) => (self.moved(step), Effect::None),
            Message::OpenSelected => self.open_selected(),
            Message::Tick => (self.ticked(), Effect::None),
            Message::Fetched(index, report) => (self.fetched(index, report), Effect::None),
            Message::Finished => (self.finished(), Effect::None),
            Message::Opened(url, Ok(())) => (self.with_status(Status::Opened(url)), Effect::None),
            Message::Opened(_, Err(error)) => {
                (self.with_status(Status::OpenFailed(error)), Effect::None)
            }
        }
    }

    fn reports(&self) -> impl Iterator<Item = &ProjectReport> {
        self.slots.iter().filter_map(|slot| match slot {
            Slot::Pending(_) => None,
            Slot::Done(report) => Some(report),
        })
    }

    fn with_status(self, status: Status) -> Self {
        Self { status, ..self }
    }

    fn fetching(self) -> Self {
        Self {
            fetch: Some(Progress {
                received: 0,
                lit: true,
            }),
            status: Status::Loading,
            ..self
        }
    }

    fn ticked(self) -> Self {
        let fetch = self.fetch.map(|progress| Progress {
            lit: !progress.lit,
            ..progress
        });
        Self { fetch, ..self }
    }

    fn fetched(self, index: usize, report: ProjectReport) -> Self {
        let fetch = self.fetch.map(|progress| Progress {
            received: progress.received + 1,
            ..progress
        });
        // The report goes to the slot at `index`, moved rather than cloned: it is the only `Some`
        // in the sequence zipped with the slots.
        let replacements = iter::repeat_with(|| None)
            .take(index)
            .chain(iter::once(Some(report)))
            .chain(iter::repeat_with(|| None));
        let Self {
            slots,
            rows,
            selected,
            status,
            ..
        } = self;
        let slots = slots
            .into_iter()
            .zip(replacements)
            .map(|(slot, replacement)| replacement.map_or(slot, Slot::Done))
            .collect();
        // `with_slots` reads the old rows and selection to keep the selection in place.
        Self {
            slots: Vec::new(),
            rows,
            selected,
            status,
            fetch,
        }
        .with_slots(slots)
    }

    fn finished(self) -> Self {
        Self {
            fetch: None,
            status: Status::Summary,
            ..self
        }
    }

    /// Replaces the slots and their rows. The selection stays on the same pull request when it
    /// is still there, and otherwise on the same line when it still exists.
    fn with_slots(self, slots: Vec<Slot>) -> Self {
        let rows: Vec<DisplayRow> = slots
            .iter()
            .flat_map(|slot| match slot {
                Slot::Pending(project) => vec![loading_row(project)],
                Slot::Done(report) => report_rows(report),
            })
            .collect();
        let same_pull_request = self
            .selected
            .and_then(|index| self.rows.get(index))
            .and_then(|row| row.url.as_ref())
            .and_then(|url| rows.iter().position(|row| row.url.as_ref() == Some(url)));
        let selected = same_pull_request.or_else(|| clamp(self.selected.unwrap_or(0), rows.len()));
        Self {
            slots,
            rows,
            selected,
            ..self
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
        let selected = clamp(target, self.rows.len());
        Self { selected, ..self }
    }

    fn open_selected(self) -> (Self, Effect) {
        let url = self
            .selected
            .and_then(|index| self.rows.get(index))
            .and_then(|row| row.url.clone());
        match url {
            // The address comes from the API, so only web pages are handed to the system opener.
            Some(url) if url.as_str().starts_with(SAFE_SCHEME) => (self, Effect::Open(url)),
            Some(url) => (self.with_status(Status::Unsafe(url)), Effect::None),
            None => (self.with_status(Status::NotAPullRequest), Effect::None),
        }
    }
}

/// The index `index` limited to the last of `len` rows, or `None` when there are no rows.
fn clamp(index: usize, len: usize) -> Option<usize> {
    len.checked_sub(1).map(|last| index.min(last))
}

fn summary(reports: &[&ProjectReport]) -> String {
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
pub mod fixtures {
    use super::*;
    use crate::adapter::view::fixtures::{pull_request, report};
    use crate::domain::error::SourceError;
    use crate::domain::model::PrState;

    pub use crate::adapter::view::fixtures::FIRST_URL;

    pub fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    pub fn url(text: &str) -> PrUrl {
        PrUrl(text.to_owned())
    }

    /// Rows: 0 = a/b PR #1, 1 = a/b PR #2 (http), 2 = c/d without PRs, 3 = e/f failed.
    pub fn reports() -> Vec<ProjectReport> {
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

    /// The messages of a fetch that brings `reports`, in order, and finishes.
    pub fn fetch_of(reports: Vec<ProjectReport>) -> Vec<Message> {
        reports
            .into_iter()
            .enumerate()
            .map(|(index, report)| Message::Fetched(index, report))
            .chain([Message::Finished])
            .collect()
    }

    /// Applies `messages` in order, returning the final state and the last effect.
    pub fn run(app: App, messages: impl IntoIterator<Item = Message>) -> (App, Effect) {
        messages
            .into_iter()
            .fold((app, Effect::None), |(app, _), message| app.update(message))
    }

    /// The first fetch of the projects of `reports`, before any of them arrives.
    pub fn loading_of(reports: &[ProjectReport]) -> App {
        App::new(
            reports
                .iter()
                .map(|report| report.project.clone())
                .collect(),
        )
        .0
    }

    /// The first fetch of the projects of [`reports`], before any of them arrives.
    pub fn loading() -> App {
        loading_of(&reports())
    }

    /// Every one of `reports` fetched.
    pub fn loaded_with(reports: Vec<ProjectReport>) -> App {
        run(loading_of(&reports), fetch_of(reports)).0
    }

    /// Every one of [`reports`] fetched.
    pub fn loaded() -> App {
        loaded_with(reports())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::fixtures::{
        FIRST_URL, fetch_of, key, loaded, loaded_with, loading, reports, run, url,
    };
    use crate::adapter::view::fixtures::{pull_request, report};
    use crate::adapter::view::{RowKind, rows};
    use crate::domain::model::PrState;

    fn moves(steps: impl IntoIterator<Item = Move>) -> App {
        run(loaded(), steps.into_iter().map(Message::Move)).0
    }

    fn kinds(app: &App) -> Vec<RowKind> {
        app.rows().iter().map(|row| row.kind).collect()
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

    /// The only place that spells out the status texts; other tests use the constants.
    #[test]
    fn status_texts_are_fixed() {
        assert_eq!(LOADING, "Loading…");
        assert_eq!(NOT_A_PULL_REQUEST, "This row is not a pull request");
    }

    #[test]
    fn starts_fetching_with_one_loading_row_per_project() {
        let projects = reports().into_iter().map(|report| report.project).collect();
        let (app, effect) = App::new(projects);
        assert_eq!(effect, Effect::Fetch);
        assert_eq!(kinds(&app), [RowKind::Loading; 3]);
        assert_eq!(app.selected(), Some(0));
        assert_eq!(app.status(), format!("{LOADING} 0/3"));
        assert_eq!(app.light(), Light::On);
    }

    #[test]
    fn each_project_replaces_its_loading_row_as_it_arrives() {
        let failed = reports().remove(2);
        let app = loading().update(Message::Fetched(2, failed)).0;
        assert_eq!(
            kinds(&app),
            [RowKind::Loading, RowKind::Loading, RowKind::Failed]
        );
        assert_eq!(app.status(), format!("{LOADING} 1/3"));
        assert_eq!(app.light(), Light::On);
    }

    #[test]
    fn a_finished_fetch_shows_every_row_and_summarizes() {
        let app = loaded();
        assert_eq!(app.rows(), rows(&reports()));
        assert_eq!(app.selected(), Some(0));
        assert_eq!(app.status(), "2 open pull requests in 3 projects, 1 failed");
    }

    #[test]
    fn summary_omits_failures_when_there_are_none() {
        let app = loaded_with(vec![report("c/d", Ok(vec![]))]);
        assert_eq!(app.status(), "0 open pull requests in 1 projects");
    }

    #[test]
    fn the_light_blinks_while_fetching_and_then_shows_the_result() {
        let blinking = run(loading(), [Message::Tick]).0;
        assert_eq!(blinking.light(), Light::Off);
        assert_eq!(run(blinking, [Message::Tick]).0.light(), Light::On);
        assert_eq!(loaded().light(), Light::Failed);
        let succeeded = loaded_with(vec![report("c/d", Ok(vec![]))]);
        assert_eq!(succeeded.light(), Light::Done);
        assert_eq!(run(succeeded, [Message::Tick]).0.light(), Light::Done);
    }

    #[test]
    fn refresh_keeps_the_rows_until_each_project_arrives_again() {
        let (app, effect) = loaded().update(Message::Refresh);
        assert_eq!(effect, Effect::Fetch);
        assert_eq!(app.rows(), rows(&reports()));
        assert_eq!(app.status(), format!("{LOADING} 0/3"));
        assert_eq!(app.light(), Light::On);
        let app = app.update(Message::Fetched(1, report("c/d", Ok(vec![])))).0;
        assert_eq!(app.status(), format!("{LOADING} 1/3"));
    }

    #[test]
    fn refresh_while_fetching_does_nothing() {
        let app = loading();
        assert_eq!(app.clone().update(Message::Refresh), (app, Effect::None));
    }

    #[test]
    fn into_reports_returns_the_projects_that_arrived() {
        let first = reports().remove(0);
        let app = loading().update(Message::Fetched(0, first.clone())).0;
        assert_eq!(app.into_reports(), [first]);
        assert_eq!(loaded().into_reports(), reports());
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
    fn without_projects_nothing_is_selected() {
        let (app, effect) = run(App::new(vec![]).0, fetch_of(vec![]));
        assert_eq!(effect, Effect::None);
        assert_eq!(app.selected(), None);
        assert_eq!(app.status(), "0 open pull requests in 0 projects");
        let (app, effect) = app.update(Message::Move(Move::Down));
        assert_eq!(app.selected(), None);
        assert_eq!(effect, Effect::None);
        assert_eq!(app.update(Message::OpenSelected).1, Effect::None);
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
    fn quit_asks_to_leave() {
        assert_eq!(loaded().update(Message::Quit).1, Effect::Quit);
        assert_eq!(loading().update(Message::Quit).1, Effect::Quit);
    }

    #[test]
    fn the_selection_follows_its_pull_request_or_keeps_its_line() {
        let second = "http://github.com/a/b/pull/2";
        let refreshed = |numbers: &[u64]| {
            Message::Fetched(
                0,
                report(
                    "a/b",
                    Ok(numbers
                        .iter()
                        .map(|&number| match number {
                            2 => pull_request(2, PrState::Open, second),
                            number => pull_request(
                                number,
                                PrState::Open,
                                &format!("https://github.com/a/b/pull/{number}"),
                            ),
                        })
                        .collect()),
                ),
            )
        };
        let on_second = run(moves([Move::Down]), [Message::Refresh]).0;

        let moved_down = run(on_second.clone(), [refreshed(&[3, 1, 2])]).0;
        assert_eq!(moved_down.selected(), Some(2));
        assert_eq!(moved_down.rows()[2].url, Some(url(second)));

        let gone = run(on_second, [refreshed(&[1])]).0;
        assert_eq!(gone.selected(), Some(1));
        assert_eq!(gone.rows()[1].kind, RowKind::NoPullRequests);
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
}
