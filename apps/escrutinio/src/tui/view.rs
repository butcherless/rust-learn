//! Dibuja un [`App`]: la barra del título, la tabla de circunscripciones con su barra de votos y
//! la fila del total, la leyenda y la ayuda (RN-08, RN-09). Con el servicio (RN-11), el título es
//! el de 2023, la ayuda no tiene pausa ni reinicio y, si falla, el pie muestra el error. Solo lee
//! el estado.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Row as TableRow, Table};

use crate::census::Election;
use crate::domain::{Pulse, Stations, Tally, percent, ratio, thousands};
use crate::mock::Row;
use crate::tui::app::{App, Scale, Source, Status};

pub const HELP: &str =
    "↑/↓ desplazar · RePág/AvPág página · espacio pausa · r reiniciar · e escala · q salir";
/// La ayuda con el servicio, que no tiene pausa ni reinicio.
pub const HELP_SERVICE: &str = "↑/↓ desplazar · RePág/AvPág página · e escala · q salir";

/// El título de `election`: `Elecciones generales España 2026`.
pub fn title(election: Election) -> String {
    format!("Elecciones generales España {}", election.year())
}
pub const TOTAL: &str = "Total España";

/// Las texturas y colores de cada zona de la barra (RN-08).
const CENSUS: (&str, Color) = ("░", Color::DarkGray);
/// Ocre apagado: el amarillo de la terminal es tan brillante que se confunde con el verde del
/// escrutado.
const OCHRE: Color = Color::Rgb(140, 110, 50);
const ESTIMATED: (&str, Color) = ("▒", OCHRE);
const COUNTED: (&str, Color) = ("█", Color::Green);
const ABOVE_ESTIMATE: (&str, Color) = ("█", Color::Cyan);
const TITLE_BAR: Style = Style::new().fg(Color::White).bg(Color::Blue);
/// El piloto (RN-12), como el de pr-checker.
const PILOT: &str = "●";
const ORANGE: Color = Color::Rgb(255, 165, 0);
/// Azul claro: el `Color::Blue` de la terminal es tan oscuro que apenas se lee sobre el fondo.
const LIGHT_BLUE: Color = Color::Rgb(100, 170, 255);

const NAME_WIDTH: u16 = 22;
const NUMBER_WIDTH: u16 = 10;
const PERCENT_WIDTH: u16 = 7;
/// «60.310/60.310», lo más ancho que tiene la columna de las mesas.
const STATIONS_WIDTH: u16 = 13;
const PILOT_WIDTH: u16 = 1;
const SPACING: u16 = 1;
/// Los títulos antes y después de la columna de las mesas, que solo está al seguir al servicio.
const HEADERS_BEFORE: [&str; 4] = ["Provincia", "Censo", "Estimado", "Escrutado"];
const HEADERS_AFTER: [&str; 3] = ["Partic.", "", ""];
const STATIONS_HEADER: &str = "Mesas";

/// Dibuja `app` en todo el marco.
pub fn draw(app: &App, frame: &mut Frame) {
    let [title, body, legend, help] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    draw_title(app, frame, title);
    draw_table(app, frame, body);
    frame.render_widget(legend_line(app), legend);
    frame.render_widget(help_line(app.source()), help);
}

/// La ayuda de las teclas o, si el servicio falló, su error.
fn help_line(source: &Source) -> Line<'static> {
    match source {
        Source::Mock => Line::from(format!(" {HELP}")).dark_gray(),
        Source::Service { error: None, .. } => Line::from(format!(" {HELP_SERVICE}")).dark_gray(),
        Source::Service {
            error: Some(error), ..
        } => Line::from(format!(" {error}")).red(),
    }
}

/// El título a la izquierda y el estado a la derecha, sobre fondo azul.
fn draw_title(app: &App, frame: &mut Frame, area: Rect) {
    frame.render_widget(Block::new().style(TITLE_BAR), area);
    frame.render_widget(
        Line::from(format!(" {}", title(app.election()))).bold(),
        area,
    );
    frame.render_widget(
        Line::from(format!("{} ", status_text(app.status()))).right_aligned(),
        area,
    );
}

pub fn status_text(status: Status) -> &'static str {
    match status {
        Status::Running => "▶ Escrutando",
        Status::Paused => "⏸ En pausa",
        Status::Finished => "✓ Escrutinio completo",
        Status::Following => "● Siguiendo el servicio",
        Status::Unreachable => "✗ Sin conexión con el servicio",
    }
}

/// La cabecera, las filas que caben desde `offset` y, abajo, el total.
fn draw_table(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().blue())
        .title(" Votos por provincia ")
        .title_bottom(Line::from(format!(" Escala {} ", app.scale().label())).right_aligned());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [header, rows_area, rule, total_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    // La columna de las mesas N'/N' solo la hay con el servicio (RN-13): la maqueta no tiene mesas.
    let with_stations = matches!(app.source(), Source::Service { .. });
    let columns = 7 + u16::from(with_stations);
    let bar_width = inner.width.saturating_sub(
        NAME_WIDTH
            + 3 * NUMBER_WIDTH
            + PERCENT_WIDTH
            + PILOT_WIDTH
            + if with_stations { STATIONS_WIDTH } else { 0 }
            + (columns - 1) * SPACING,
    );
    let rows = app.scrutiny().rows();
    let visible = usize::from(rows_area.height);
    let offset = app.offset().min(rows.len().saturating_sub(visible));
    let largest = rows.iter().map(|row| row.tally.census).max().unwrap_or(0);
    let base = |tally: &Tally| match app.scale() {
        Scale::Own => tally.census,
        Scale::Shared => largest,
    };

    let titles: Vec<&str> = HEADERS_BEFORE
        .into_iter()
        .chain(with_stations.then_some(STATIONS_HEADER))
        .chain(HEADERS_AFTER)
        .collect();
    let numeric_columns = 1..=3 + usize::from(with_stations) + 1;
    let headers = titles.iter().enumerate().map(|(column, &text)| {
        let line = Line::from(text);
        // Las cifras van alineadas a la derecha, como sus títulos.
        if numeric_columns.contains(&column) {
            line.right_aligned()
        } else {
            line
        }
    });
    frame.render_widget(
        table([TableRow::new(headers).bold().dark_gray()], with_stations),
        header,
    );
    frame.render_widget(
        table(
            rows.iter().skip(offset).take(visible).map(|row: &Row| {
                tally_row(
                    row.district.name,
                    &row.tally,
                    with_stations.then(|| stations_text(row.stations)),
                    row.pulse,
                    base(&row.tally),
                    bar_width,
                )
            }),
            with_stations,
        ),
        rows_area,
    );
    frame.render_widget(
        Line::from("─".repeat(usize::from(rule.width))).dark_gray(),
        rule,
    );
    let total = app.scrutiny().total();
    // El total se dibuja siempre contra su censo: con la escala absoluta no cabría.
    frame.render_widget(
        table(
            [tally_row(
                TOTAL,
                &total,
                with_stations.then(|| stations_text(app.scrutiny().stations())),
                app.scrutiny().pulse(),
                total.census,
                bar_width,
            )
            .bold()],
            with_stations,
        ),
        total_area,
    );
}

fn table<'a>(rows: impl IntoIterator<Item = TableRow<'a>>, with_stations: bool) -> Table<'a> {
    let widths = [
        Constraint::Length(NAME_WIDTH),
        Constraint::Length(NUMBER_WIDTH),
        Constraint::Length(NUMBER_WIDTH),
        Constraint::Length(NUMBER_WIDTH),
    ]
    .into_iter()
    .chain(with_stations.then_some(Constraint::Length(STATIONS_WIDTH)))
    .chain([
        Constraint::Length(PERCENT_WIDTH),
        Constraint::Length(PILOT_WIDTH),
        Constraint::Fill(1),
    ]);
    Table::new(rows, widths).column_spacing(SPACING)
}

/// Las mesas N'/N' tal como llegan del servicio (RN-13), o nada si aún no ha dicho nada de la fila.
fn stations_text(stations: Option<Stations>) -> String {
    stations.map_or_else(String::new, |stations| {
        format!(
            "{}/{}",
            thousands(u64::from(stations.counted)),
            thousands(u64::from(stations.possible))
        )
    })
}

/// Una fila de la tabla; `stations` es el texto de las mesas, si esa columna se muestra.
fn tally_row(
    name: &str,
    tally: &Tally,
    stations: Option<String>,
    pulse: Pulse,
    base: u64,
    bar_width: u16,
) -> TableRow<'static> {
    let right = |text: String| Line::from(text).right_aligned();
    let cells = [
        Line::from(name.to_owned()),
        right(thousands(tally.census)),
        right(thousands(tally.estimated)).fg(OCHRE),
        right(thousands(tally.counted)).green(),
    ]
    .into_iter()
    .chain(stations.map(right))
    .chain([
        right(percent(tally.turnout())),
        Line::from(Span::styled(PILOT, pulse_color(pulse))),
        bar(tally, base, bar_width),
    ]);
    TableRow::new(cells)
}

/// Naranja si cambió en la última actualización, azul claro si no, gris sin datos y verde si está
/// completa, sin nada más que esperar (RN-12).
pub fn pulse_color(pulse: Pulse) -> Color {
    match pulse {
        Pulse::NoData => Color::DarkGray,
        Pulse::Updated => ORANGE,
        Pulse::Steady => LIGHT_BLUE,
        Pulse::Complete => Color::Green,
    }
}

/// La barra de un recuento en `width` celdas, a escala de `base` votos (RN-08): escrutado hasta
/// el estimado, escrutado por encima del estimado, estimado aún sin escrutar y resto del censo.
///
/// Las celdas se redondean primero a centésimas: el estimado redondeado hacia abajo (RN-04) queda
/// unas milésimas por debajo del 70 %, y sin eso, cuando el 70 % del ancho cae justo en media celda
/// (73,5 de 105), las provincias con el 70 % exacto tendrían una celda más de estimado que el resto.
pub fn bar(tally: &Tally, base: u64, width: u16) -> Line<'static> {
    let cells = |votes: u64| {
        let exact = ratio(votes, base) * f64::from(width);
        let cells = ((exact * 100.0).round() / 100.0).round() as usize;
        cells.min(usize::from(width))
    };
    let (census, estimated, counted) = (
        cells(tally.census),
        cells(tally.estimated),
        cells(tally.counted),
    );
    let segments = [
        (COUNTED, counted.min(estimated)),
        (ABOVE_ESTIMATE, counted.saturating_sub(estimated)),
        (ESTIMATED, estimated.saturating_sub(counted)),
        (CENSUS, census.saturating_sub(counted.max(estimated))),
    ];
    Line::from(
        segments
            .into_iter()
            .filter(|&(_, length)| length > 0)
            .map(|((texture, color), length)| Span::styled(texture.repeat(length), color))
            .collect::<Vec<_>>(),
    )
}

/// Qué significa cada zona de la barra.
fn legend_line(app: &App) -> Line<'static> {
    let estimated = format!("Estimado ({} %)", app.scrutiny().turnout());
    let items = [
        (CENSUS, "Censo".to_owned()),
        (ESTIMATED, estimated),
        (COUNTED, "Escrutado".to_owned()),
        (ABOVE_ESTIMATE, "Escrutado sobre lo estimado".to_owned()),
    ];
    let pilots = [
        (Pulse::Updated, "Actualizada"),
        (Pulse::Steady, "Sin cambios"),
        (Pulse::NoData, "Sin datos"),
        (Pulse::Complete, "Completa"),
    ]
    .into_iter()
    .flat_map(|(pulse, label)| {
        [
            Span::styled(PILOT, pulse_color(pulse)),
            Span::raw(format!(" {label}  ")),
        ]
    });
    Line::from(
        std::iter::once(Span::raw(" "))
            .chain(items.into_iter().flat_map(|((texture, color), label)| {
                [
                    Span::styled(texture.repeat(2), color),
                    Span::raw(format!(" {label}   ")),
                ]
            }))
            .chain(pilots)
            .collect::<Vec<_>>(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::Message;
    use crate::tui::app::fixtures::{app, apply, following};
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

    fn text(line: &Line) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    #[test]
    fn texts_shown_to_the_user() {
        assert_eq!(
            title(Election::Generales2026),
            "Elecciones generales España 2026"
        );
        assert_eq!(
            title(Election::Generales2023),
            "Elecciones generales España 2023"
        );
        assert_eq!(status_text(Status::Following), "● Siguiendo el servicio");
        assert_eq!(
            status_text(Status::Unreachable),
            "✗ Sin conexión con el servicio"
        );
        assert_eq!(TOTAL, "Total España");
        assert_eq!(status_text(Status::Running), "▶ Escrutando");
        assert_eq!(status_text(Status::Paused), "⏸ En pausa");
        assert_eq!(status_text(Status::Finished), "✓ Escrutinio completo");
    }

    #[test]
    fn the_bar_shows_each_zone_in_order() {
        let tally = Tally::new(2_000_000, 70);
        [
            ("no votes", tally, "▒▒▒▒▒▒▒░░░"),
            ("some votes", tally.with_counted(400_000), "██▒▒▒▒▒░░░"),
            (
                "at the estimate",
                tally.with_counted(1_400_000),
                "███████░░░",
            ),
            ("above it", tally.with_counted(1_600_000), "████████░░"),
            ("everyone", tally.with_counted(2_000_000), "██████████"),
        ]
        .into_iter()
        .for_each(|(case, tally, expected)| {
            assert_eq!(text(&bar(&tally, tally.census, 10)), expected, "{case}");
        });
    }

    #[test]
    fn votes_above_the_estimate_have_their_own_color() {
        let tally = Tally::new(2_000_000, 70).with_counted(1_600_000);
        let colors: Vec<_> = bar(&tally, tally.census, 10)
            .spans
            .iter()
            .map(|span| span.style.fg)
            .collect();
        assert_eq!(
            colors,
            [Some(Color::Green), Some(Color::Cyan), Some(Color::DarkGray)]
        );
    }

    #[test]
    fn an_estimate_rounded_down_draws_as_long_as_an_exact_one() {
        // Alicante: el 70 % exacto. Albacete: 3 votos por debajo, por el redondeo de RN-04.
        let width = 105;
        let estimated_cells = |census| {
            let tally = Tally::new(census, 70);
            text(&bar(&tally, tally.census, width))
                .chars()
                .filter(|&c| c == '▒')
                .count()
        };
        assert_eq!(estimated_cells(1_261_010), 74);
        assert_eq!(estimated_cells(299_924), 74);
    }

    #[test]
    fn the_pilot_is_orange_green_or_gray() {
        assert_eq!(pulse_color(Pulse::Updated), Color::Rgb(255, 165, 0));
        assert_eq!(pulse_color(Pulse::Steady), Color::Rgb(100, 170, 255));
        assert_eq!(pulse_color(Pulse::NoData), Color::DarkGray);
        assert_eq!(pulse_color(Pulse::Complete), Color::Green);
    }

    #[test]
    fn the_estimate_is_a_dim_ochre_not_the_terminal_yellow() {
        let tally = Tally::new(2_000_000, 70).with_counted(400_000);
        let colors: Vec<_> = bar(&tally, tally.census, 10)
            .spans
            .iter()
            .map(|span| span.style.fg)
            .collect();
        assert_eq!(
            colors,
            [
                Some(Color::Green),
                Some(Color::Rgb(140, 110, 50)),
                Some(Color::DarkGray)
            ]
        );
    }

    #[test]
    fn a_shared_scale_shortens_the_smaller_bars() {
        let tally = Tally::new(1_000_000, 70);
        assert_eq!(text(&bar(&tally, 2_000_000, 10)), "▒▒▒▒░");
        assert_eq!(text(&bar(&tally, 0, 10)), "");
    }

    #[test]
    fn shows_the_title_the_table_and_the_total() {
        assert_shows(
            &screen(&app(), 120, 20),
            &[
                &title(Election::Generales2026),
                status_text(Status::Running),
                "Votos por provincia",
                "Provincia",
                "Censo",
                "Estimado",
                "Escrutado",
                "A Coruña",
                "928.060",
                "649.642",
                TOTAL,
                "35.953.542",
                "0,0 %",
                "Escala proporcional",
                "Estimado (70 %)",
                HELP,
            ],
        );
    }

    #[test]
    fn scrolling_to_the_end_shows_the_autonomous_cities_last() {
        let screen = screen(&apply(app(), &vec![Message::PageDown; 6]), 120, 20);
        assert_shows(&screen, &["Zaragoza", "Ceuta", "Melilla", TOTAL]);
        assert!(!screen.contains("A Coruña"));
        let lines: Vec<_> = screen.lines().collect();
        let line_of = |name| lines.iter().position(|line| line.contains(name));
        assert!(line_of("Zaragoza") < line_of("Ceuta"));
        assert!(line_of("Ceuta") < line_of("Melilla"));
    }

    /// El color del piloto de la fila que empieza por `name`, en una pantalla de 120 × 20.
    fn pilot_color(app: &App, name: &str) -> Option<Color> {
        let mut terminal = Terminal::new(TestBackend::new(120, 20)).expect("test backend");
        terminal.draw(|frame| draw(app, frame)).expect("draw");
        let buffer = terminal.backend().buffer();
        let width = usize::from(buffer.area.width);
        buffer
            .content()
            .chunks(width)
            .find(|line| {
                line.iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>()
                    .trim_start_matches(['│', ' '])
                    .starts_with(name)
            })
            .and_then(|line| line.iter().find(|cell| cell.symbol() == PILOT))
            .map(|cell| cell.fg)
    }

    #[test]
    fn the_pilots_show_which_districts_changed_in_the_last_update() {
        use crate::feed::fixtures::{snapshot, snapshots, with_stations};
        let coruna = |counted| Message::Snapshots(Ok(snapshots(vec![snapshot("15", counted)])));
        let failed = Message::Snapshots(Err("sin red".to_owned()));
        let gray = Some(Color::DarkGray);
        [
            ("start", vec![], gray),
            ("first data", vec![coruna(500)], Some(ORANGE)),
            (
                "no change",
                vec![coruna(500), coruna(500)],
                Some(LIGHT_BLUE),
            ),
            (
                "more votes",
                vec![coruna(500), coruna(500), coruna(900)],
                Some(ORANGE),
            ),
            (
                "a failed request changes nothing",
                vec![coruna(500), failed],
                Some(ORANGE),
            ),
        ]
        .into_iter()
        .for_each(|(case, messages, color)| {
            let app = apply(following(), &messages);
            assert_eq!(pilot_color(&app, "A Coruña"), color, "{case}");
        });
        // Completa lo dice el servicio: aquí, con todas sus mesas posibles.
        let complete = Message::Snapshots(Ok(snapshots(vec![with_stations("15", 900, 718, 718)])));
        let complete = apply(following(), &[complete]);
        assert_eq!(pilot_color(&complete, "A Coruña"), Some(Color::Green));
        // Y el total, completo solo si el servicio dice que lo está.
        assert_eq!(pilot_color(&complete, TOTAL), Some(Color::Green));
        let one = apply(following(), &[coruna(500)]);
        assert_eq!(pilot_color(&one, "Albacete"), gray);
        assert_eq!(pilot_color(&one, TOTAL), Some(ORANGE));
    }

    #[test]
    fn the_stations_column_shows_what_the_service_calculated() {
        use crate::feed::fixtures::{snapshots, with_stations};
        assert_eq!(STATIONS_HEADER, "Mesas");
        // La maqueta no tiene mesas: sin columna.
        assert!(!screen(&app(), 120, 20).contains(STATIONS_HEADER));
        // León (24) y A Coruña (15) con mesas N'/N' distintas, y la suma que hace el servicio.
        let update = Message::Snapshots(Ok(snapshots(vec![
            with_stations("15", 500_000, 700, 718),
            with_stations("24", 400_000, 718, 718),
        ])));
        let screen = screen(&apply(following(), &[update]), 150, 70);
        assert_shows(
            &screen,
            &[STATIONS_HEADER, "700/718", "718/718", "1.418/1.436"],
        );
        // Una fila de la que el servicio no ha dicho nada no inventa mesas.
        let lines: Vec<_> = screen.lines().collect();
        let albacete = lines.iter().find(|line| line.contains("Albacete"));
        assert!(
            albacete.is_some_and(|line| !line.contains('/')),
            "{albacete:?}"
        );
    }

    #[test]
    fn following_the_service_shows_2023_and_its_errors() {
        use crate::feed::fixtures::{snapshot, snapshots};
        let ok = Message::Snapshots(Ok(snapshots(vec![snapshot("15", 500_000)])));
        assert_shows(
            &screen(&apply(following(), std::slice::from_ref(&ok)), 120, 20),
            &[
                &title(Election::Generales2023),
                status_text(Status::Following),
                "A Coruña",
                "500.000",
                "35.141.198",
                HELP_SERVICE,
            ],
        );
        let failed = apply(
            following(),
            &[ok, Message::Snapshots(Err("sin red".to_owned()))],
        );
        let screen = screen(&failed, 120, 20);
        assert_shows(
            &screen,
            &[status_text(Status::Unreachable), "500.000", "sin red"],
        );
        assert!(!screen.contains(HELP_SERVICE));
    }

    #[test]
    fn shows_the_paused_and_finished_states_and_the_shared_scale() {
        let paused = apply(
            app(),
            &[Message::Tick, Message::TogglePause, Message::ToggleScale],
        );
        assert_shows(
            &screen(&paused, 120, 20),
            &[status_text(Status::Paused), "Escala absoluta"],
        );
        let finished = apply(app(), &vec![Message::Tick; 200]);
        assert_shows(
            &screen(&finished, 120, 20),
            &[status_text(Status::Finished)],
        );
    }
}
