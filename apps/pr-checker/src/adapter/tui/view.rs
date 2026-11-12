//! Draws an [`App`]: the table of rows, the status line and the help line. It only reads the
//! state.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Row, Table, TableState};

use crate::adapter::tui::app::App;
use crate::adapter::view::{DisplayRow, HEADER, ORANGE, RowKind};

const TITLE: &str = " Open pull requests ";
const HELP: &str = "↑/↓ move · Enter open in browser · r refresh · q quit";
const HIGHLIGHT_SYMBOL: &str = "▶ ";
const NUMBER_WIDTH: u16 = 6;
const STATE_WIDTH: u16 = 6;

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
    frame.render_widget(Line::from(app.status()), status_area);
    frame.render_widget(Line::from(HELP).dim(), help_area);
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

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::adapter::tui::app::fixtures::loaded;

    fn screen(app: &App) -> String {
        let mut terminal = Terminal::new(TestBackend::new(70, 10)).unwrap();
        terminal.draw(|frame| draw(app, frame)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
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
