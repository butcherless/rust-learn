//! Draws an [`App`]: the table of rows with a merge and a close checkbox per pull request, the
//! status line with
//! its light, and the help line. It only reads the state.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Cell, Row, Table, TableState};

use crate::adapter::tui::app::{Action, App, Checkbox, Light};
use crate::adapter::view::{DisplayRow, HEADER, ORANGE, RowKind};

const TITLE: &str = " Open pull requests ";
const HELP: &str = "↑/↓ move · Enter open in browser · m m merge · c c close · r refresh · q quit";
const HIGHLIGHT_SYMBOL: &str = "▶ ";
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

/// Draws the whole screen for `app`.
pub fn draw(app: &App, frame: &mut Frame) {
    let [table_area, status_area, help_area] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    // The selection is part of the model; ratatui derives the scroll offset from it.
    let mut table_state = TableState::default().with_selected(app.selected());

    frame.render_stateful_widget(table(app), table_area, &mut table_state);
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

/// The project column fits its longest entry; the title takes the remaining space.
fn column_widths(display_rows: &[DisplayRow]) -> [Constraint; 7] {
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
        Constraint::Length(MERGEABLE_WIDTH),
        Constraint::Length(CHECKBOX_WIDTH),
        Constraint::Length(CHECKBOX_WIDTH),
    ]
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
    use crate::adapter::view::fixtures::report;
    use crate::adapter::view::{CLOSED, LOADING_PROJECT, MERGED, NO_PULL_REQUESTS};

    /// The status line's row on the 80×10 test screen.
    const STATUS_ROW: u16 = 8;

    fn buffer(app: &App) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
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

    /// The first table row on the 80×10 test screen, below the border and the header.
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
}
