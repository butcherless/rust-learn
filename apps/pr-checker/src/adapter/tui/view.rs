//! Draws an [`App`]: the table of rows, the status line with its light, and the help line. It
//! only reads the state.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Row, Table, TableState};

use crate::adapter::tui::app::{App, Light};
use crate::adapter::view::{DisplayRow, HEADER, ORANGE, RowKind};

const TITLE: &str = " Open pull requests ";
const HELP: &str = "↑/↓ move · Enter open in browser · r refresh · q quit";
const HIGHLIGHT_SYMBOL: &str = "▶ ";
const NUMBER_WIDTH: u16 = 6;
const STATE_WIDTH: u16 = 6;
/// The light while lit, and when idle (FR13).
const LIGHT_ON: &str = "●";
/// The light in the dark half of the blink.
const LIGHT_OFF: &str = "○";
const MERGEABLE_WIDTH: u16 = 9;

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

    frame.render_stateful_widget(table(app.rows()), table_area, &mut table_state);
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
fn column_widths(display_rows: &[DisplayRow]) -> [Constraint; 5] {
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
    ]
}

fn color(kind: RowKind) -> Color {
    match kind {
        RowKind::NoPullRequests => Color::Green,
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
    use crate::adapter::tui::app::Message;
    use crate::adapter::tui::app::fixtures::{loaded, loaded_with, loading, run};
    use crate::adapter::view::fixtures::report;
    use crate::adapter::view::{LOADING_PROJECT, NO_PULL_REQUESTS};

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
