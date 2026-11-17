//! Draws an [`App`]: the table of rows with a merge switch per pull request, the status line with
//! its light, and the help line. It only reads the state.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Cell, Row, Table, TableState};

use crate::adapter::tui::app::{App, Light, MergeSwitch};
use crate::adapter::view::{DisplayRow, HEADER, ORANGE, RowKind};

const TITLE: &str = " Open pull requests ";
const HELP: &str = "↑/↓ move · Enter open in browser · m m merge · r refresh · q quit";
const HIGHLIGHT_SYMBOL: &str = "▶ ";
const NUMBER_WIDTH: u16 = 6;
const STATE_WIDTH: u16 = 6;
/// The light while lit, and when idle (FR13).
const LIGHT_ON: &str = "●";
/// The light in the dark half of the blink.
const LIGHT_OFF: &str = "○";
const MERGEABLE_WIDTH: u16 = 9;
/// The title of the column after [`HEADER`], only in the terminal UI.
const MERGE_TITLE: &str = "Merge";
/// The switch's ends, which round off its track, and its knob.
const SWITCH_LEFT: &str = "▐";
const SWITCH_RIGHT: &str = "▌";
const SWITCH_KNOB: char = '●';
/// The track between the ends: three cells, one of them the knob.
const SWITCH_TRACK: usize = 3;
const MERGE_WIDTH: u16 = 5;

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
        let switch = app.merge_switch(row).map_or_else(Cell::default, switch);
        Row::new(cells.chain([switch])).style(Style::new().fg(color(row.kind)))
    });
    let header = HEADER.into_iter().chain([MERGE_TITLE]);
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

/// A switch like a phone's: a white knob on an orange track while open, sliding to the right on
/// green once merged.
fn switch(state: MergeSwitch) -> Cell<'static> {
    let (knob_at, color) = match state {
        MergeSwitch::Off => (0, orange()),
        MergeSwitch::Moving => (1, orange()),
        MergeSwitch::On => (SWITCH_TRACK - 1, Color::Green),
    };
    let track: String = (0..SWITCH_TRACK)
        .map(|position| {
            if position == knob_at {
                SWITCH_KNOB
            } else {
                ' '
            }
        })
        .collect();
    Cell::from(Line::from(vec![
        Span::styled(SWITCH_LEFT, Style::new().fg(color)),
        Span::styled(track, Style::new().fg(Color::White).bg(color)),
        Span::styled(SWITCH_RIGHT, Style::new().fg(color)),
    ]))
}

/// The project column fits its longest entry; the title takes the remaining space.
fn column_widths(display_rows: &[DisplayRow]) -> [Constraint; 6] {
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
        Constraint::Length(MERGE_WIDTH),
    ]
}

fn color(kind: RowKind) -> Color {
    match kind {
        RowKind::NoPullRequests | RowKind::MergedPullRequest => Color::Green,
        RowKind::OpenPullRequest => orange(),
        RowKind::Failed => Color::Red,
        RowKind::Loading => Color::DarkGray,
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    use super::*;
    use crate::adapter::tui::app::fixtures::{loaded, loaded_with, loading, merged, run};
    use crate::adapter::tui::app::{Message, Move};
    use crate::adapter::view::fixtures::report;
    use crate::adapter::view::{LOADING_PROJECT, MERGED, NO_PULL_REQUESTS};

    /// The status line's row on the 70×10 test screen.
    const STATUS_ROW: u16 = 8;

    fn buffer(app: &App) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(70, 10)).unwrap();
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

    /// The first table row on the 70×10 test screen, below the border and the header.
    const FIRST_ROW: u16 = 2;

    /// The switch of the first row: its text and the color of its track.
    fn first_switch(app: &App) -> (String, Color) {
        let buffer = buffer(app);
        let start = (0..buffer.area.width)
            .find(|&x| buffer[(x, FIRST_ROW)].symbol() == SWITCH_LEFT)
            .unwrap();
        let text = (start..start + MERGE_WIDTH)
            .map(|x| buffer[(x, FIRST_ROW)].symbol())
            .collect();
        (text, buffer[(start + 1, FIRST_ROW)].bg)
    }

    /// The switch's colors are the check, so this test reads them. The selection moves off the
    /// first row, whose highlight would reverse them.
    #[test]
    fn the_merge_switch_slides_and_turns_green() {
        [
            ("open", Message::Tick, "▐●  ▌", orange()),
            ("merging", Message::MergeSelected, "▐ ● ▌", orange()),
        ]
        .into_iter()
        .for_each(|(case, second_key, text, color)| {
            let app = run(
                loaded(),
                [
                    Message::MergeSelected,
                    second_key,
                    Message::Move(Move::Last),
                ],
            )
            .0;
            assert_eq!(first_switch(&app), (text.to_owned(), color), "{case}");
        });
        let app = run(merged(), [Message::Move(Move::Last)]).0;
        assert_eq!(
            first_switch(&app),
            ("▐  ●▌".to_owned(), Color::Green),
            "merged"
        );
    }

    /// The selected row is drawn in inverse video of its color, so it turns from orange to
    /// green with the merge, like its text.
    #[test]
    fn the_selection_takes_the_color_of_its_row() {
        [
            ("open", loaded(), orange()),
            ("merged", merged(), Color::Green),
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
    fn a_merged_row_shows_its_new_state() {
        let screen = screen(&merged());
        assert!(screen.contains(MERGED), "missing {MERGED:?} in {screen}");
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
            "title 1",
            NO_PULL_REQUESTS,
            "error: repository not found",
            "2 open pull requests",
            "q quit",
        ]
        .into_iter()
        .for_each(|text| assert!(screen.contains(text), "missing {text:?} in {screen}"));
    }
}
