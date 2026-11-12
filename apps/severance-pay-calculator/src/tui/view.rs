//! Draws an [`App`]: the input panel and the grouped report side by side (stacked on narrow
//! terminals), the forecast below them when the terminal is tall enough, then the disclaimer and
//! the help line. It only reads the state.

use std::iter::once;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Cell, Padding, Paragraph, Row, Table, Wrap};

use crate::domain::{Dismissal, Report};
use crate::report::{Concept, DISCLAIMER, FORECAST_YEARS, GOOD_LUCK, GROUPS, horizon};
use crate::sequence::{Cycle, separated};
use crate::tui::app::{App, Field, FieldValue};

const ORANGE: Color = Color::Rgb(255, 165, 0);
const PADDING: Padding = Padding::new(2, 2, 1, 1);
/// Below this width the panels are stacked.
const SIDE_BY_SIDE_MIN_WIDTH: u16 = 76;
const VALUE_WIDTH: u16 = 12;
const FOCUS_MARKER: &str = "▶ ";
const INDENT: &str = "  ";
const CURSOR: &str = "▏";
const RULE: &str =
    "────────────────────────────────────────────────────────────────────────────────";
const EARNED: &str = "█";
const GROWTH: &str = "░";
const GAP: &str = "  ";
const DATE_WIDTH: usize = 10;
const HORIZON_WIDTH: usize = 3;
const HELP: &str = "Tab/↓ next · Shift+Tab/↑ previous · ←/→ change type · Del clear · Esc quit";

/// Draws `app` on the whole frame.
pub fn draw(app: &App, frame: &mut Frame) {
    let area = frame.area();
    let side_by_side = area.width >= SIDE_BY_SIDE_MIN_WIDTH;
    let main_height = if side_by_side {
        Constraint::Length(report_height())
    } else {
        Constraint::Fill(1)
    };
    let main_needed = if side_by_side {
        report_height()
    } else {
        input_height() + report_height()
    };
    // Shown only when it fits with the panels, the footer and the help line.
    let forecast_rows = if area.height >= main_needed + forecast_height() + 2 {
        forecast_height()
    } else {
        0
    };
    let [main, forecast_area, footer, help, _] = Layout::vertical([
        main_height,
        Constraint::Length(forecast_rows),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(0),
    ])
    .areas(area);
    let [input_area, report_area] = if side_by_side {
        Layout::horizontal([Constraint::Length(input_width()), Constraint::Fill(1)])
            .spacing(1)
            .areas(main)
    } else {
        Layout::vertical([Constraint::Length(input_height()), Constraint::Fill(1)]).areas(main)
    };

    frame.render_widget(
        Paragraph::new(input_lines(app)).block(panel(" Input ", Color::Cyan)),
        input_area,
    );
    let report_block = panel(" Report ", Color::DarkGray)
        .title_bottom(Line::from(format!(" {GOOD_LUCK} ")).green().right_aligned());
    match app.report() {
        Ok(report) => frame.render_widget(report_table(&report).block(report_block), report_area),
        Err(error) => frame.render_widget(
            Paragraph::new(Line::from(format!("⚠ {error}")).red().bold())
                .wrap(Wrap { trim: true })
                .block(report_block),
            report_area,
        ),
    }
    if forecast_rows > 0 {
        draw_forecast(app, frame, forecast_area);
    }
    [(DISCLAIMER, footer), (HELP, help)]
        .into_iter()
        .for_each(|(text, area)| {
            frame.render_widget(Line::from(format!(" {text}")).dark_gray(), area);
        });
}

/// Every field's lines, with a blank line between fields.
fn input_lines(app: &App) -> Vec<Line<'static>> {
    let fields = Field::ALL
        .iter()
        .map(|&field| field_lines(field, app.value(field), field == app.focus()));
    separated(fields, Line::default).collect()
}

/// A field's label line and value line. The focused field is yellow, with a marker and a cursor.
fn field_lines(field: Field, value: FieldValue, focused: bool) -> [Line<'static>; 2] {
    let value = match value {
        FieldValue::Text(text) if focused => format!("{text}{CURSOR}"),
        FieldValue::Text(text) => text.to_owned(),
        FieldValue::Choice(dismissal) => choice_text(dismissal),
    };
    let marker = if focused { FOCUS_MARKER } else { INDENT };
    let label = Line::from(format!("{marker}{}", field.label()));
    let value = Line::from(format!("{INDENT}{value}"));
    if focused {
        [label.yellow().bold(), value.yellow().bold()]
    } else {
        [label.gray(), value]
    }
}

fn choice_text(dismissal: Dismissal) -> String {
    format!("◀ {dismissal} ▶")
}

/// The grouped report: a header, each group under an orange heading, and the total.
fn report_table(report: &Report) -> Table<'static> {
    let rule = || Row::new([RULE, RULE]).dark_gray();
    let groups = GROUPS.iter().map(|group| {
        let heading = Row::new([group.title]).style(Style::new().fg(ORANGE).bold());
        let items = group.concepts.iter().map(move |concept| {
            Row::new([
                Cell::from(format!("{INDENT}{}", concept.label())),
                value_cell(concept.value(report)).cyan(),
            ])
        });
        once(heading).chain(items)
    });
    let total = Row::new([
        Cell::from(Concept::Total.label()).bold(),
        value_cell(Concept::Total.value(report)).green().bold(),
    ]);
    let rows: Vec<Row> = once(rule())
        .chain(separated(groups, || Row::new([""; 2])))
        .chain([rule(), total])
        .collect();
    Table::new(rows, [Constraint::Fill(1), Constraint::Length(VALUE_WIDTH)]).header(
        Row::new([Cell::from("Concept"), value_cell("Value".to_owned())])
            .gray()
            .bold(),
    )
}

fn value_cell(text: String) -> Cell<'static> {
    Cell::from(Line::from(text).right_aligned())
}

/// The forecast panel: one stacked bar per anniversary, or a hint when the input is invalid.
fn draw_forecast(app: &App, frame: &mut Frame, area: ratatui::layout::Rect) {
    let title = format!(" Forecast · next {FORECAST_YEARS} years ");
    let block = panel(&title, Color::DarkGray);
    let width = usize::from(block.inner(area).width);
    let lines = app.forecast().map_or_else(
        |_| vec![Line::from("Fix the input to see the forecast").dark_gray()],
        |reports| forecast_lines(&reports, width),
    );
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// A line per report with its date, horizon, bar and total, then the legend. Each bar is the
/// total scaled to the largest one: the part earned today in solid green, the growth shaded cyan.
fn forecast_lines(reports: &[Report], width: usize) -> Vec<Line<'static>> {
    let value_width = usize::from(VALUE_WIDTH);
    let bar_width = width.saturating_sub(DATE_WIDTH + HORIZON_WIDTH + value_width + 3 * GAP.len());
    let max = reports.iter().map(|r| r.total).fold(0.0, f64::max);
    let earned = reports
        .first()
        .map_or(0, |today| bar_cells(today.total, max, bar_width));
    let bars = reports.iter().enumerate().map(|(years, report)| {
        let cells = bar_cells(report.total, max, bar_width);
        let solid = earned.min(cells);
        // Today's row is what is earned now: its total is green and bold like the report's
        // total, and the forecast rows are cyan.
        let (horizon_style, value_style) = if years == 0 {
            (Style::new().fg(ORANGE).bold(), Style::new().green().bold())
        } else {
            (Style::new().gray(), Style::new().cyan())
        };
        Line::from(vec![
            Span::from(Concept::CurrentDate.value(report)).gray(),
            Span::raw(GAP),
            Span::styled(format!("{:<HORIZON_WIDTH$}", horizon(years)), horizon_style),
            Span::raw(GAP),
            Span::from(EARNED.repeat(solid)).green(),
            Span::from(GROWTH.repeat(cells - solid)).cyan(),
            Span::raw(" ".repeat(bar_width - cells)),
            Span::raw(GAP),
            Span::styled(
                format!("{:>value_width$}", Concept::Total.value(report)),
                value_style,
            ),
        ])
    });
    let legend = Line::from(vec![
        Span::raw(" ".repeat(DATE_WIDTH + HORIZON_WIDTH + 2 * GAP.len())),
        Span::from(EARNED.repeat(3)).green(),
        Span::from(" earned today   ").gray(),
        Span::from(GROWTH.repeat(3)).cyan(),
        Span::from(" forecast growth").gray(),
    ]);
    bars.chain(once(legend)).collect()
}

/// How many of `width` cells show `value` on a scale where `max` fills them all.
fn bar_cells(value: f64, max: f64, width: usize) -> usize {
    if max > 0.0 {
        ((value / max * width as f64).round() as usize).min(width)
    } else {
        0
    }
}

/// A rounded panel with a bold title, a colored border, and space between border and content.
fn panel(title: &str, border: Color) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border))
        .title(title.bold().white())
        .padding(PADDING)
}

/// A panel's size for content of `width` columns and `height` rows: borders and padding added.
fn panel_size(width: usize, height: usize) -> (u16, u16) {
    let to_u16 = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
    (
        to_u16(width + 2 + usize::from(PADDING.left + PADDING.right)),
        to_u16(height + 2 + usize::from(PADDING.top + PADDING.bottom)),
    )
}

/// Fits the widest dismissal choice, which is wider than every label and text field.
fn input_width() -> u16 {
    let widest = Dismissal::ALL
        .iter()
        .flat_map(|&dismissal| field_lines(Field::Dismissal, FieldValue::Choice(dismissal), true))
        .map(|line| line.width())
        .max()
        .unwrap_or(0);
    panel_size(widest, 0).0
}

/// Two lines per field and a blank line between fields.
fn input_height() -> u16 {
    panel_size(0, Field::ALL.len() * 3 - 1).1
}

/// A line per forecast point and the legend.
fn forecast_height() -> u16 {
    panel_size(0, FORECAST_YEARS as usize + 2).1
}

/// The table: header and rule, each group's heading and rows, spacers between groups, then a
/// rule and the total.
fn report_height() -> u16 {
    let groups: usize = GROUPS.iter().map(|group| 1 + group.concepts.len()).sum();
    panel_size(0, 2 + groups + GROUPS.len() - 1 + 2).1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Input;
    use crate::domain::fixtures::python_input;
    use crate::tui::app::Message;
    use crate::tui::app::fixtures::{app, apply};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn screen(app: &App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test backend");
        terminal.draw(|frame| draw(app, frame)).expect("draw");
        let buffer = terminal.backend().buffer();
        buffer
            .content()
            .chunks(usize::from(buffer.area.width))
            .map(|line| line.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn assert_shows(text: &str, expected: &[&str]) {
        expected
            .iter()
            .for_each(|part| assert!(text.contains(part), "missing {part:?}\n{text}"));
    }

    #[test]
    fn shows_the_python_report_grouped() {
        assert_shows(
            &screen(&app(), 80, 24),
            &[
                "Concept",
                "SALARY",
                "53,000",
                "33",
                "13.128",
                "PERIOD",
                "2021-10-11",
                "2026-11-07",
                "Days worked",
                "1,854",
                "TOTAL SEVERANCE PAY",
                "24,340",
                GOOD_LUCK,
                DISCLAIMER,
            ],
        );
    }

    #[test]
    fn panels_are_side_by_side_when_wide_and_stacked_when_narrow() {
        let titles_on_one_row = |text: &str| {
            text.lines()
                .any(|line| line.contains(" Input ") && line.contains(" Report "))
        };
        assert!(titles_on_one_row(&screen(&app(), 80, 24)));
        let narrow = screen(&app(), 60, 40);
        assert!(!titles_on_one_row(&narrow), "{narrow}");
        assert_shows(&narrow, &["24,340"]);
    }

    #[test]
    fn marks_the_focused_field() {
        let text = screen(&apply(app(), &[Message::NextField]), 80, 24);
        assert_shows(&text, &["▶ Type of dismissal", "  Annual pay", "53000 "]);
    }

    #[test]
    fn every_dismissal_fits_the_input_panel() {
        Dismissal::ALL.iter().for_each(|&dismissal| {
            let app = App::from(Input {
                dismissal,
                ..python_input()
            });
            assert_shows(&screen(&app, 80, 24), &[&choice_text(dismissal)]);
        });
    }

    #[test]
    fn shows_errors_instead_of_the_report() {
        let text = screen(&apply(app(), &[Message::Clear]), 120, 24);
        assert_shows(&text, &["⚠ annual pay is not a number"]);
        assert!(!text.contains("TOTAL"), "{text}");
    }

    #[test]
    fn panel_sizes_follow_the_content() {
        assert_eq!(
            (
                input_width(),
                input_height(),
                report_height(),
                forecast_height()
            ),
            (38, 15, 17, 11)
        );
    }

    #[test]
    fn forecast_shows_when_the_terminal_is_tall_enough() {
        let tall = screen(&app(), 80, 30);
        assert_shows(
            &tall,
            &[
                "Forecast · next 5 years",
                "2026-11-07  now",
                "2031-10-11  +5y",
                "24,340",
                "28,777",
                "33,582",
                "38,374",
                "43,165",
                "47,957",
                "earned today",
                "forecast growth",
                DISCLAIMER,
            ],
        );
        assert!(!screen(&app(), 80, 29).contains("Forecast"));
        let stacked = screen(&app(), 60, 50);
        assert_shows(&stacked, &["Forecast", "47,957"]);
    }

    #[test]
    fn forecast_asks_to_fix_invalid_input() {
        let text = screen(&apply(app(), &[Message::Clear]), 120, 40);
        assert_shows(&text, &["Fix the input to see the forecast"]);
    }

    #[test]
    fn forecast_bars_stack_earned_and_growth() {
        let reports = app().forecast().expect("valid input");
        let counts: Vec<_> = forecast_lines(&reports, 74)
            .iter()
            .take(reports.len())
            .map(|line| {
                let text = line.to_string();
                (text.matches(EARNED).count(), text.matches(GROWTH).count())
            })
            .collect();
        // 74 columns leave 43 for the bars; today's 24,340 of 47,957 is 22 of them.
        assert_eq!(
            counts,
            [(22, 0), (22, 4), (22, 8), (22, 12), (22, 17), (22, 21)]
        );
    }

    #[test]
    fn scales_bars() {
        [
            (0.0, 10.0, 10, 0),
            (5.0, 10.0, 10, 5),
            (10.0, 10.0, 10, 10),
            (11.0, 10.0, 10, 10),
            (5.0, 0.0, 10, 0),
        ]
        .into_iter()
        .for_each(|(value, max, width, cells)| {
            assert_eq!(bar_cells(value, max, width), cells, "{value}/{max}");
        });
    }
}
