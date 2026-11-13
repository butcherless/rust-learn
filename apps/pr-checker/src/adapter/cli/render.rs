//! Renders project reports as a colored comfy-table.

use comfy_table::presets::UTF8_FULL_CONDENSED;
use comfy_table::{Cell, Color, Row, Table};

use crate::adapter::view::{DisplayRow, HEADER, ORANGE, RowKind, rows};
use crate::service::ProjectReport;

/// Builds the table. Styling is dropped when stdout is not a TTY.
pub fn render(reports: &[ProjectReport]) -> Table {
    let mut table = Table::new();
    table
        .load_style(UTF8_FULL_CONDENSED.with_rounded_corners())
        .set_header(HEADER)
        .add_rows(rows(reports).into_iter().map(table_row));
    table
}

fn table_row(row: DisplayRow) -> Row {
    let color = color(row.kind);
    Row::from(row.cells.map(|text| Cell::new(text).fg(color)))
}

fn color(kind: RowKind) -> Color {
    match kind {
        RowKind::NoPullRequests => Color::Green,
        RowKind::OpenPullRequest => {
            let (r, g, b) = ORANGE;
            Color::Rgb { r, g, b }
        }
        RowKind::Failed => Color::Red,
        RowKind::Loading => Color::DarkGrey,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::view::fixtures::report;
    use crate::domain::error::SourceError;

    #[test]
    fn empty_reports_render_only_the_header() {
        let output = render(&[]).to_string();
        assert!(HEADER.iter().all(|title| output.contains(title)));
    }

    #[test]
    fn each_kind_has_its_color() {
        [
            (RowKind::NoPullRequests, Color::Green),
            (
                RowKind::OpenPullRequest,
                Color::Rgb {
                    r: 255,
                    g: 165,
                    b: 0,
                },
            ),
            (RowKind::Failed, Color::Red),
            (RowKind::Loading, Color::DarkGrey),
        ]
        .into_iter()
        .for_each(|(kind, expected)| assert_eq!(color(kind), expected, "{kind:?}"));
    }

    #[test]
    fn rows_are_rendered_with_their_text() {
        let output = render(&[report("a/b", Err(SourceError::NotFound))]).to_string();
        assert!(output.contains("error: repository not found"), "{output}");
    }
}
