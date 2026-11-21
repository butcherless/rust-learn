//! Draws an [`App`]: the table of rows with a merge and a close checkbox per pull request, the
//! panel of running workflows below it, the status line with its light, and the help line. It
//! only reads the state.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Cell, Padding, Row, Table, TableState};

use crate::adapter::tui::app::{Action, App, Checkbox, Light};
use crate::adapter::view::{
    DisplayRow, HEADER, LOADING_PROJECT, ORANGE, RUN_HEADER, RowKind, RunRow, RunRowKind, run_rows,
};
use crate::domain::model::RunStatus;

const TITLE: &str = " Open pull requests ";
const HELP: &str = "↑/↓ move · Enter open in browser · m m merge · c c close · r refresh · q quit";
const HIGHLIGHT_SYMBOL: &str = "▶ ";
/// The columns [`HIGHLIGHT_SYMBOL`] takes.
const HIGHLIGHT_WIDTH: u16 = 2;
const NUMBER_WIDTH: u16 = 6;
const STATE_WIDTH: u16 = 6;
/// The light while lit, and when idle (FR13).
const LIGHT_ON: &str = "●";
/// The light in the dark half of the blink.
const LIGHT_OFF: &str = "○";
const MERGEABLE_WIDTH: u16 = 9;
/// The titles of the columns after [`HEADER`], only in the terminal UI.
const MERGE_TITLE: &str = "Merge";
const CLOSE_TITLE: &str = "Close";
/// A checkbox while open, confirming, in progress and done, padded to [`CHECKBOX_WIDTH`].
const CHECKBOX_OFF: &str = " [ ] ";
const CHECKBOX_CONFIRMING: &str = " [?] ";
const CHECKBOX_MOVING: &str = " [~] ";
const CHECKBOX_ON: &str = " [✓] ";
const CHECKBOX_WIDTH: u16 = 5;
/// The actions, in the order of their columns.
const ACTIONS: [Action; 2] = [Action::Merge, Action::Close];

const RUNS_TITLE: &str = "Running workflows";
/// The runs panel's note when no project has an unfinished run.
const NO_RUNS: &str = "No workflows running";
/// The runs panel can always take this fraction of the screen's height, and more when the pull
/// requests leave it free.
const RUNS_SHARE: u16 = 3;
/// The runs panel's borders and header, around its rows. The pull request table has the same.
const RUNS_FRAME: u16 = 3;
/// The status and help lines.
const FOOTER: u16 = 2;
const EVENT_WIDTH: u16 = 12;
const RUN_STATUS_WIDTH: u16 = 11;

/// Draws the whole screen for `app`.
pub fn draw(app: &App, frame: &mut Frame) {
    let runs = run_rows(app.runs());
    let runs_height = runs_height(runs.len(), app.rows().len(), frame.area().height);
    let [table_area, runs_area, status_area, help_area] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(runs_height),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    // The selection is part of the model; ratatui derives the scroll offset from it, so the
    // selected pull request stays visible in whatever height the runs panel leaves.
    let mut table_state = TableState::default().with_selected(app.selected());

    frame.render_stateful_widget(table(app), table_area, &mut table_state);
    frame.render_widget(runs_panel(app, &runs, runs_height), runs_area);
    frame.render_widget(status_line(app), status_area);
    frame.render_widget(Line::from(HELP).dim(), help_area);
}

/// The light, then the status text.
fn status_line(app: &App) -> Line<'static> {
    let (symbol, color) = match app.light() {
        Light::On => (LIGHT_ON, orange()),
        Light::Off => (LIGHT_OFF, Color::DarkGray),
        Light::Done => (LIGHT_ON, Color::Green),
        Light::Failed => (LIGHT_ON, Color::Red),
    };
    Line::from(vec![
        Span::styled(symbol, Style::new().fg(color)),
        Span::raw(" "),
        Span::raw(app.status()),
    ])
}

fn orange() -> Color {
    let (r, g, b) = ORANGE;
    Color::Rgb(r, g, b)
}

fn table(app: &App) -> Table<'static> {
    let body = app.rows().iter().map(|row| {
        let cells = row.cells.iter().cloned().map(Cell::from);
        let checkboxes = ACTIONS.map(|action| {
            app.checkbox(row, action)
                // An empty text, not `Cell::default()`: with a default cell before it, the
                // Close checkbox of a closed row was drawn under Merge.
                .map_or_else(|| Cell::from(""), |state| checkbox(state, action))
        });
        Row::new(cells.chain(checkboxes)).style(Style::new().fg(color(row.kind)))
    });
    let header = HEADER.into_iter().chain([MERGE_TITLE, CLOSE_TITLE]);
    Table::new(body, column_widths(app.rows()))
        .header(Row::new(header).style(Style::new().bold()))
        .block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .title(TITLE),
        )
        .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(HIGHLIGHT_SYMBOL)
}

/// A checkbox: empty and gray while open, `?` in orange while it waits for the second key, `~`
/// in orange while GitHub does it, and a tick once done: green once merged, red once closed. Its
/// text tells the states apart even without color.
fn checkbox(state: Checkbox, action: Action) -> Cell<'static> {
    let done = match action {
        Action::Merge => Color::Green,
        Action::Close => Color::Red,
    };
    let (text, style) = match state {
        Checkbox::Off => (CHECKBOX_OFF, Style::new().fg(Color::DarkGray)),
        Checkbox::Confirming => (CHECKBOX_CONFIRMING, Style::new().fg(orange()).bold()),
        Checkbox::Moving => (CHECKBOX_MOVING, Style::new().fg(orange()).bold()),
        Checkbox::On => (CHECKBOX_ON, Style::new().fg(done).bold()),
    };
    Cell::from(Span::styled(text, style))
}

/// The runs panel's height: its frame and one line per run, or one for its note. It gets what
/// the pull requests leave free, and at least 1/[`RUNS_SHARE`] of `screen` when they don't fit
/// and scroll, but never less than its frame and one line.
fn runs_height(runs: usize, pull_requests: usize, screen: u16) -> u16 {
    let lines = |count: usize| RUNS_FRAME.saturating_add(u16::try_from(count).unwrap_or(u16::MAX));
    let free = screen.saturating_sub(FOOTER + lines(pull_requests));
    let room = free.max(screen / RUNS_SHARE).max(lines(1));
    lines(runs.max(1)).min(room)
}

/// The running workflows: one row per unfinished run, in orange while in progress and gray
/// while queued or waiting, and red for a project whose runs failed. Rows that don't fit in
/// `height` are counted in a last `… N more` row. Without rows, a note says whether they are
/// still loading or none is running.
fn runs_panel(app: &App, runs: &[RunRow], height: u16) -> Table<'static> {
    let running = runs
        .iter()
        .filter(|row| matches!(row.kind, RunRowKind::Run(_)))
        .count();
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(format!(" {RUNS_TITLE} · {running} "))
        // Lined up with the pull requests, which leave room for the selection marker.
        .padding(Padding::left(HIGHLIGHT_WIDTH));
    let widths = run_widths(runs);
    if runs.is_empty() {
        let note = if app.runs_pending() {
            LOADING_PROJECT
        } else {
            NO_RUNS
        };
        return Table::new([Row::new([note]).dark_gray()], [Constraint::Fill(1)]).block(block);
    }
    let fits = usize::from(height.saturating_sub(RUNS_FRAME));
    let (shown, more) = if runs.len() > fits {
        let shown = fits.saturating_sub(1);
        (shown, Some(runs.len() - shown))
    } else {
        (runs.len(), None)
    };
    let body = runs[..shown]
        .iter()
        .map(|row| {
            let color = match row.kind {
                RunRowKind::Run(RunStatus::InProgress) => orange(),
                RunRowKind::Run(RunStatus::Queued | RunStatus::Waiting) => Color::Gray,
                RunRowKind::Failed => Color::Red,
            };
            Row::new(row.cells.clone()).style(Style::new().fg(color))
        })
        // In the Workflow column, which is wide enough for it.
        .chain(more.map(|more| Row::new(["…".to_owned(), format!("{more} more")]).dark_gray()));
    Table::new(body, widths)
        .header(Row::new(RUN_HEADER).style(Style::new().bold()))
        .block(block)
}

/// The project column fits its longest entry; workflow and branch share the rest.
fn run_widths(runs: &[RunRow]) -> [Constraint; 5] {
    [
        fit(RUN_HEADER[0], runs.iter().map(|row| row.cells[0].as_str())),
        Constraint::Fill(2),
        Constraint::Fill(1),
        Constraint::Length(EVENT_WIDTH),
        Constraint::Length(RUN_STATUS_WIDTH),
    ]
}

/// The project column fits its longest entry; the title takes the remaining space.
fn column_widths(display_rows: &[DisplayRow]) -> [Constraint; 7] {
    [
        fit(
            HEADER[0],
            display_rows.iter().map(|row| row.cells[0].as_str()),
        ),
        Constraint::Length(NUMBER_WIDTH),
        Constraint::Fill(1),
        Constraint::Length(STATE_WIDTH),
        Constraint::Length(MERGEABLE_WIDTH),
        Constraint::Length(CHECKBOX_WIDTH),
        Constraint::Length(CHECKBOX_WIDTH),
    ]
}

/// A column as wide as the longest of its `title` and its `cells`.
fn fit<'a>(title: &'a str, cells: impl Iterator<Item = &'a str>) -> Constraint {
    let width = cells
        .chain([title])
        .map(|text| text.chars().count())
        .max()
        .and_then(|width| u16::try_from(width).ok())
        .unwrap_or(u16::MAX);
    Constraint::Length(width)
}

fn color(kind: RowKind) -> Color {
    match kind {
        RowKind::NoPullRequests | RowKind::MergedPullRequest => Color::Green,
        RowKind::OpenPullRequest => orange(),
        RowKind::Failed => Color::Red,
        RowKind::Loading => Color::DarkGray,
        RowKind::ClosedPullRequest => Color::Gray,
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    use super::*;
    use crate::adapter::tui::app::fixtures::{closed, loaded, loaded_with, loading, merged, run};
    use crate::adapter::tui::app::{Message, Move};
    use crate::adapter::view::fixtures::{pull_request, report, run as workflow, runs};
    use crate::adapter::view::{CLOSED, LOADING_PROJECT, MERGED, NO_PULL_REQUESTS};
    use crate::domain::error::SourceError;
    use crate::domain::model::PrState;
    use crate::service::RunsReport;

    /// The status line's row on the 80×16 test screen.
    const STATUS_ROW: u16 = 14;

    fn buffer(app: &App) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(80, 16)).unwrap();
        terminal.draw(|frame| draw(app, frame)).unwrap();
        terminal.backend().buffer().clone()
    }

    fn screen(app: &App) -> String {
        buffer(app)
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    /// The light is the one place where the color is the check, so this test reads it.
    #[test]
    fn the_light_starts_the_status_line() {
        [
            ("fetching, lit", loading(), LIGHT_ON, orange()),
            (
                "fetching, dark",
                run(loading(), [Message::Tick]).0,
                LIGHT_OFF,
                Color::DarkGray,
            ),
            (
                "done",
                loaded_with(vec![report("c/d", Ok(vec![]))]),
                LIGHT_ON,
                Color::Green,
            ),
            ("failed", loaded(), LIGHT_ON, Color::Red),
        ]
        .into_iter()
        .for_each(|(case, app, symbol, color)| {
            let cell = &buffer(&app)[(0, STATUS_ROW)];
            assert_eq!((cell.symbol(), cell.fg), (symbol, color), "{case}");
        });
    }

    /// The first table row on the 80×16 test screen, below the border and the header.
    const FIRST_ROW: u16 = 2;

    /// The checkbox of the first row in the column titled `title`: its text and its color.
    fn first_checkbox(app: &App, title: &str) -> (String, Color) {
        let buffer = buffer(app);
        let header: Vec<&str> = (0..buffer.area.width)
            .map(|x| buffer[(x, FIRST_ROW - 1)].symbol())
            .collect();
        // The whole title, so `Merge` does not match the start of `Mergeable`.
        let len = title.chars().count();
        let start = (0..header.len().saturating_sub(len))
            .find(|&x| {
                header[x..x + len].concat() == title
                    && !header[x + len].chars().all(char::is_alphabetic)
            })
            .and_then(|x| u16::try_from(x).ok())
            .unwrap();
        let text = (start..start + CHECKBOX_WIDTH)
            .map(|x| buffer[(x, FIRST_ROW)].symbol())
            .collect();
        (text, buffer[(start + 1, FIRST_ROW)].fg)
    }

    /// The checkboxes' colors are part of the check, so this test reads them. The selection
    /// moves off the first row, whose highlight would reverse them, except while confirming,
    /// which moving would cancel: there the highlight only adds `REVERSED` to the same color.
    #[test]
    fn the_checkboxes_fill_and_turn_green_or_red() {
        assert_eq!(
            [
                CHECKBOX_OFF,
                CHECKBOX_CONFIRMING,
                CHECKBOX_MOVING,
                CHECKBOX_ON
            ],
            [" [ ] ", " [?] ", " [~] ", " [✓] "]
        );
        let away = Message::Move(Move::Last);
        let twice = |action| vec![Message::Act(action), Message::Act(action), away.clone()];
        let off = (CHECKBOX_OFF.to_owned(), Color::DarkGray);
        let empty = (" ".repeat(usize::from(CHECKBOX_WIDTH)), Color::Gray);
        [
            (
                "open",
                loaded(),
                vec![away.clone()],
                [off.clone(), off.clone()],
            ),
            (
                "confirming a merge",
                loaded(),
                vec![Message::Act(Action::Merge)],
                [(CHECKBOX_CONFIRMING.to_owned(), orange()), off.clone()],
            ),
            (
                "confirming a close",
                loaded(),
                vec![Message::Act(Action::Close)],
                [off.clone(), (CHECKBOX_CONFIRMING.to_owned(), orange())],
            ),
            (
                "merging",
                loaded(),
                twice(Action::Merge),
                [(CHECKBOX_MOVING.to_owned(), orange()), off.clone()],
            ),
            (
                "closing",
                loaded(),
                twice(Action::Close),
                [off.clone(), (CHECKBOX_MOVING.to_owned(), orange())],
            ),
            (
                "merged",
                merged(),
                vec![away.clone()],
                [
                    (CHECKBOX_ON.to_owned(), Color::Green),
                    (empty.0.clone(), Color::Green),
                ],
            ),
            (
                "closed",
                closed(),
                vec![away],
                [empty, (CHECKBOX_ON.to_owned(), Color::Red)],
            ),
        ]
        .into_iter()
        .for_each(|(case, app, messages, expected)| {
            let app = run(app, messages).0;
            assert_eq!(
                [MERGE_TITLE, CLOSE_TITLE].map(|title| first_checkbox(&app, title)),
                expected,
                "{case}"
            );
        });
    }

    /// The selected row is drawn in inverse video of its color, so it turns from orange to
    /// green with the merge, like its text.
    #[test]
    fn the_selection_takes_the_color_of_its_row() {
        [
            ("open", loaded(), orange()),
            ("merged", merged(), Color::Green),
            ("closed", closed(), Color::Gray),
        ]
        .into_iter()
        .for_each(|(case, app, color)| {
            assert_eq!(app.selected(), Some(0), "{case}");
            let cell = &buffer(&app)[(1, FIRST_ROW)];
            assert_eq!(cell.fg, color, "{case}");
            assert!(cell.modifier.contains(Modifier::REVERSED), "{case}");
        });
    }

    #[test]
    fn a_merged_or_closed_row_shows_its_new_state() {
        [(merged(), MERGED), (closed(), CLOSED)]
            .into_iter()
            .for_each(|(app, state)| {
                let screen = screen(&app);
                assert!(screen.contains(state), "missing {state:?} in {screen}");
            });
    }

    #[test]
    fn draws_loading_rows_and_progress() {
        let screen = screen(&loading());
        [LOADING_PROJECT, "Loading… 0/3"]
            .into_iter()
            .for_each(|text| assert!(screen.contains(text), "missing {text:?} in {screen}"));
    }

    #[test]
    fn draws_rows_status_and_help() {
        let screen = screen(&loaded());
        [
            "Open pull requests",
            "Project",
            "Mergeable",
            MERGE_TITLE,
            CLOSE_TITLE,
            "title 1",
            NO_PULL_REQUESTS,
            "error: repository not found",
            "2 open pull requests",
            "c c close",
            "q quit",
        ]
        .into_iter()
        .for_each(|text| assert!(screen.contains(text), "missing {text:?} in {screen}"));
    }

    /// [`loaded`] with the workflow runs of its three projects.
    fn with_runs(runs: [RunsReport; 3]) -> App {
        let messages = runs
            .into_iter()
            .enumerate()
            .map(|(index, report)| Message::RunsFetched(index, report));
        run(loaded(), messages).0
    }

    #[test]
    fn the_runs_panel_keeps_to_a_third_of_the_screen() {
        [
            (0, 4, 16, 4, "a note"),
            (2, 4, 16, 5, "two runs"),
            (10, 4, 16, 7, "what four pull requests leave free"),
            (
                10,
                20,
                16,
                5,
                "a third of 16 while the pull requests scroll",
            ),
            (10, 4, 60, 13, "ten runs"),
            (
                60,
                50,
                60,
                20,
                "a third of 60 while the pull requests scroll",
            ),
            (60, 10, 60, 45, "what ten pull requests leave free"),
            (3, 0, 6, 4, "never less than the frame and a line"),
        ]
        .into_iter()
        .for_each(|(runs, pull_requests, screen, height, case)| {
            assert_eq!(runs_height(runs, pull_requests, screen), height, "{case}");
        });
    }

    #[test]
    fn the_runs_panel_lists_unfinished_runs_and_failures() {
        let app = with_runs([
            runs(
                "a/b",
                Ok(vec![
                    workflow("CI", RunStatus::InProgress),
                    workflow("Deploy", RunStatus::Queued),
                ]),
            ),
            runs("c/d", Ok(vec![])),
            runs("e/f", Err(SourceError::Unauthorized)),
        ]);
        let screen = screen(&app);
        [
            "Running workflows · 2",
            "Workflow",
            "Branch",
            "CI",
            "in progress",
            "Deploy",
            "queued",
            "error: unauthorized",
        ]
        .into_iter()
        .for_each(|text| assert!(screen.contains(text), "missing {text:?} in {screen}"));
    }

    #[test]
    fn the_runs_panel_says_when_they_load_or_none_runs() {
        let loading_runs = screen(&loaded());
        assert!(loading_runs.contains(LOADING_PROJECT), "{loading_runs}");
        assert_eq!(NO_RUNS, "No workflows running");
        let idle = screen(&with_runs(
            ["a/b", "c/d", "e/f"].map(|project| runs(project, Ok(vec![]))),
        ));
        [NO_RUNS, "Running workflows · 0"]
            .into_iter()
            .for_each(|text| assert!(idle.contains(text), "missing {text:?} in {idle}"));
    }

    #[test]
    fn runs_that_do_not_fit_are_counted() {
        let many = (1..=10)
            .map(|n| workflow(&format!("job {n}"), RunStatus::InProgress))
            .collect();
        let app = with_runs([
            runs("a/b", Ok(many)),
            runs("c/d", Ok(vec![])),
            runs("e/f", Ok(vec![])),
        ]);
        // 16 rows, minus the footer and the four pull requests with their frame, leave the panel
        // 7: its frame and four lines, three runs and the count.
        let screen = screen(&app);
        ["Running workflows · 10", "job 3", "7 more"]
            .into_iter()
            .for_each(|text| assert!(screen.contains(text), "missing {text:?} in {screen}"));
        assert!(!screen.contains("job 4"), "{screen}");
    }

    #[test]
    fn the_pull_requests_scroll_to_the_selection_above_the_runs_panel() {
        let pull_requests = (1..=20)
            .map(|n| {
                pull_request(
                    n,
                    PrState::Open,
                    &format!("https://github.com/a/b/pull/{n}"),
                )
            })
            .collect();
        let app = run(
            loaded_with(vec![report("a/b", Ok(pull_requests))]),
            [Message::Move(Move::Last)],
        )
        .0;
        let screen = screen(&app);
        assert!(screen.contains("title 20"), "{screen}");
        assert!(!screen.contains("title 1 "), "{screen}");
    }
}
