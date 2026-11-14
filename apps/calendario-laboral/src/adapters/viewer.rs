//! Adaptador de salida: la ventana a pantalla completa de `month`, `year` y `summary` (RN-22).
//!
//! Sigue The Elm Architecture: [`message`] convierte una tecla en un [`Message`], y
//! [`Viewer::update`] devuelve el estado siguiente y un [`Effect`]. El estado no sabe nada del
//! terminal; [`run`] es el único código con efectos secundarios. La tecla `r` vuelve a leer el
//! fichero y a dibujar la vista (RN-28).
//!
//! En las vistas de mes y de año, las respuestas de los embalses llegan de otro hilo mientras la
//! ventana está abierta (RN-31). Cada vez solo se escriben las líneas de la pantalla que han
//! cambiado: [`Screen`] es lo que hay escrito, y [`draw`] lo compara con lo nuevo.

use std::io::{self, Write};
use std::iter;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use anstyle::{AnsiColor, Style};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::{Attribute, Print, SetAttribute};
use crossterm::terminal::{
    self, Clear, ClearType, DisableLineWrap, EnableLineWrap, EnterAlternateScreen,
    LeaveAlternateScreen,
};
use crossterm::{execute, queue};

use crate::adapters::reservoirs::{Fetched, ReservoirTable};
use crate::application::Report;
use crate::domain::MADRID_RESERVOIRS;

/// Cada cuánto cambia la luz de la fila que se consulta (RN-31).
const BLINK: Duration = Duration::from_millis(500);
/// Cada cuánto se miran las respuestas mientras llegan.
const POLL: Duration = Duration::from_millis(100);
/// Cuánto se espera una tecla cuando ya no llegan respuestas; al pasar, solo se redibuja.
const IDLE: Duration = Duration::from_secs(60);

/// Dónde pueden mostrarse los informes: en la ventana, o solo en la consola si la salida no es un
/// terminal o se ha pedido `--no-tui` (RN-22).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Window,
    Console,
}

/// Si el resultado se muestra en la ventana: solo `month`, `year` y `summary`, y solo si `output`
/// lo permite.
pub const fn opens(report: &Report, output: Output) -> bool {
    let fills_screen = match report {
        Report::Month(..) | Report::Year(..) | Report::Summary(..) => true,
        Report::Marks(..) | Report::Day(..) | Report::Config(..) => false,
    };
    match output {
        Output::Window => fills_screen,
        Output::Console => false,
    }
}

/// Si la vista lleva los embalses de Madrid: solo las de mes y de año (RN-31).
pub const fn shows_reservoirs(report: &Report) -> bool {
    match report {
        Report::Month(..) | Report::Year(..) => true,
        Report::Summary(..) | Report::Marks(..) | Report::Day(..) | Report::Config(..) => false,
    }
}

/// Lo que pide una tecla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// Una línea hacia arriba.
    Up,
    /// Una línea hacia abajo.
    Down,
    /// Una pantalla hacia arriba.
    PageUp,
    /// Una pantalla hacia abajo.
    PageDown,
    /// Al principio.
    Home,
    /// Al final.
    End,
    /// Volver a leer el fichero y a dibujar la vista (RN-28).
    Reload,
    /// Cerrar la ventana.
    Quit,
}

/// Lo que hay que hacer después de [`Viewer::update`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Volver a dibujar y esperar otra tecla.
    None,
    /// Volver a leer el fichero y pasar el texto nuevo a [`Viewer::reloaded`] (RN-28).
    Reload,
    /// Cerrar la ventana.
    Quit,
}

/// La tecla pulsada como [`Message`], o nada si no tiene función.
pub fn message(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        KeyCode::Esc | KeyCode::Char('q') => Some(Message::Quit),
        KeyCode::Up | KeyCode::Char('k') => Some(Message::Up),
        KeyCode::Down | KeyCode::Char('j') => Some(Message::Down),
        KeyCode::PageUp => Some(Message::PageUp),
        KeyCode::PageDown | KeyCode::Char(' ') => Some(Message::PageDown),
        KeyCode::Home | KeyCode::Char('g') => Some(Message::Home),
        KeyCode::End | KeyCode::Char('G') => Some(Message::End),
        KeyCode::Char('r' | 'R') => Some(Message::Reload),
        _ => None,
    }
}

/// Las líneas de la vista, la primera que se ve y el error de la última recarga, si falló.
///
/// `height` es el número de líneas que caben; llega en cada llamada porque el terminal puede
/// cambiar de tamaño.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Viewer {
    lines: Vec<String>,
    top: usize,
    error: Option<String>,
}

impl Viewer {
    /// La vista desde su primera línea. `text` es el mismo texto que se imprime en la consola.
    pub fn new(text: &str) -> Self {
        Self {
            lines: text.lines().map(str::to_owned).collect(),
            top: 0,
            error: None,
        }
    }

    /// La vista tras recargar (RN-28): con el texto nuevo desde la misma línea, o la de antes con
    /// el error en la línea de ayuda si no se ha podido leer.
    pub fn reloaded(self, text: Result<String, String>) -> Self {
        match text {
            Ok(text) => Self {
                top: self.top,
                ..Self::new(&text)
            },
            Err(error) => Self {
                error: Some(error),
                ..self
            },
        }
    }

    /// La vista con `text` en lugar del texto de antes, desde la misma línea y con el mismo error:
    /// la tabla de embalses ha cambiado (RN-31).
    pub fn retexted(self, text: &str) -> Self {
        Self {
            top: self.top,
            error: self.error,
            ..Self::new(text)
        }
    }

    /// Lo que debe verse en un terminal de `size` (columnas, filas): las líneas que caben,
    /// completadas con líneas vacías, y la de ayuda en gris en la última fila.
    pub fn screen(&self, size: (u16, u16)) -> Screen {
        let height = usize::from(text_height(size.1));
        let grey = Style::new().fg_color(Some(AnsiColor::BrightBlack.into()));
        let help = format!("{grey}{}{grey:#}", self.help(height));
        Screen {
            size,
            rows: self
                .visible(height)
                .map(str::to_owned)
                .chain(iter::repeat(String::new()))
                .take(height)
                .chain(iter::once(help))
                .collect(),
        }
    }

    /// El estado tras `message`.
    pub fn update(self, message: Message, height: usize) -> (Self, Effect) {
        let top = self.top(height);
        let page = height.max(1);
        let next = match message {
            Message::Quit => return (self, Effect::Quit),
            Message::Reload => return (self, Effect::Reload),
            Message::Up => top.saturating_sub(1),
            Message::Down => top.saturating_add(1),
            Message::PageUp => top.saturating_sub(page),
            Message::PageDown => top.saturating_add(page),
            Message::Home => 0,
            Message::End => usize::MAX,
        };
        let top = next.min(self.last_top(height));
        (Self { top, ..self }, Effect::None)
    }

    /// Las líneas que caben en `height`, desde la primera que se ve.
    pub fn visible(&self, height: usize) -> impl Iterator<Item = &str> {
        self.lines
            .iter()
            .skip(self.top(height))
            .take(height)
            .map(String::as_str)
    }

    /// La línea de ayuda: el error de la última recarga, si falló, qué líneas se ven, si la vista
    /// no cabe, y las teclas.
    pub fn help(&self, height: usize) -> String {
        let total = self.lines.len();
        let error = self
            .error
            .as_ref()
            .map(|error| format!("No se ha podido recargar: {error}"));
        let scroll = (total > height).then(|| {
            let first = self.top(height) + 1;
            let last = (self.top(height) + height).min(total);
            format!("Líneas {first}–{last} de {total} · ↑ ↓ RePág AvPág Inicio Fin: desplazar")
        });
        error
            .into_iter()
            .chain(scroll)
            .chain(iter::once("r: recargar · Esc o q: salir".to_owned()))
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// La primera línea que se ve. Si el terminal ha crecido, sube para no dejar hueco abajo.
    fn top(&self, height: usize) -> usize {
        self.top.min(self.last_top(height))
    }

    fn last_top(&self, height: usize) -> usize {
        self.lines.len().saturating_sub(height)
    }
}

/// Lo que hay escrito en la pantalla: su tamaño y cada fila, la de ayuda incluida.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Screen {
    size: (u16, u16),
    rows: Vec<String>,
}

/// Las filas del texto: todas menos la de ayuda, y al menos una.
fn text_height(rows: u16) -> u16 {
    rows.saturating_sub(1).max(1)
}

/// Muestra el texto que `text` da para `report` en la pantalla alternativa hasta que se cierra con
/// `Esc` o `q`; `r` cambia el informe por lo que devuelva `reload` (RN-28). Si hay `fetched`, la
/// vista lleva la tabla de embalses, que se rellena con cada respuesta (RN-31). El terminal se
/// restaura también si el bucle falla.
pub fn run(
    report: Report,
    text: impl Fn(&Report, Option<&ReservoirTable>) -> String,
    reload: impl FnMut() -> Result<Report, String>,
    fetched: Option<Receiver<Fetched>>,
) -> io::Result<()> {
    let mut out = io::stdout();
    terminal::enable_raw_mode()?;
    let result = execute!(out, EnterAlternateScreen, DisableLineWrap, Hide)
        .and_then(|()| event_loop(&mut out, report, text, reload, fetched));
    let restored =
        execute!(out, Show, EnableLineWrap, LeaveAlternateScreen).and(terminal::disable_raw_mode());
    result.and(restored)
}

fn event_loop(
    out: &mut impl Write,
    initial: Report,
    text: impl Fn(&Report, Option<&ReservoirTable>) -> String,
    mut reload: impl FnMut() -> Result<Report, String>,
    fetched: Option<Receiver<Fetched>>,
) -> io::Result<()> {
    let mut report = initial;
    let mut table = fetched
        .as_ref()
        .map(|_| ReservoirTable::new(&MADRID_RESERVOIRS));
    let mut viewer = Viewer::new(&text(&report, table.as_ref()));
    let mut screen = Screen::default();
    let mut blinked = Instant::now();
    loop {
        let next = viewer.screen(terminal::size()?);
        draw(out, &screen, &next)?;
        screen = next;
        let fetching = table.as_ref().is_some_and(ReservoirTable::is_fetching);
        let wait = if fetching { POLL } else { IDLE };
        let key = if event::poll(wait)? {
            event::read()?.as_key_press_event().and_then(message)
        } else {
            None
        };
        if let Some(message) = key {
            let (next, effect) = viewer.update(message, usize::from(text_height(screen.size.1)));
            match effect {
                Effect::Quit => return Ok(()),
                Effect::None => viewer = next,
                Effect::Reload => match reload() {
                    Ok(reloaded) => {
                        report = reloaded;
                        viewer = next.reloaded(Ok(text(&report, table.as_ref())));
                    }
                    Err(error) => viewer = next.reloaded(Err(error)),
                },
            }
        }
        // RN-31: las respuestas que hayan llegado y, cada medio segundo, el parpadeo.
        let arrived: Vec<Fetched> = fetched.iter().flat_map(Receiver::try_iter).collect();
        let blink = fetching && blinked.elapsed() >= BLINK;
        if blink {
            blinked = Instant::now();
        }
        if !arrived.is_empty() || blink {
            table = table.map(|table| {
                let table = arrived.into_iter().fold(table, ReservoirTable::fetched);
                if blink { table.ticked() } else { table }
            });
            viewer = viewer.retexted(&text(&report, table.as_ref()));
        }
    }
}

/// Escribe `next` sobre la pantalla, que tiene `previous`: solo las filas que han cambiado, o
/// todas tras borrarla si el terminal ha cambiado de tamaño.
fn draw(out: &mut impl Write, previous: &Screen, next: &Screen) -> io::Result<()> {
    let resized = previous.size != next.size;
    if resized {
        queue!(out, Clear(ClearType::All))?;
    }
    next.rows
        .iter()
        .zip(0..u16::MAX)
        .filter(|&(line, row)| resized || previous.rows.get(usize::from(row)) != Some(line))
        .try_for_each(|(line, row)| {
            queue!(
                out,
                MoveTo(0, row),
                Clear(ClearType::CurrentLine),
                Print(line),
                SetAttribute(Attribute::Reset)
            )
        })?;
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{Command, Outcome, execute};
    use crate::domain::Month;
    use crate::domain::fixtures::{calendar, date, today};

    fn report(command: Command) -> Report {
        match execute(calendar(), command) {
            Ok(Outcome::Report(report)) => report,
            other => panic!("expected a report, got {other:?}"),
        }
    }

    fn numbered(count: usize) -> Viewer {
        Viewer::new(
            &(1..=count)
                .map(|line| format!("línea {line}\n"))
                .collect::<String>(),
        )
    }

    #[test]
    fn rn_22_only_month_year_and_summary_open_the_window_on_a_terminal() {
        let month = report(Command::Month {
            year: 2026,
            month: Month::June,
            today: today(),
        });
        let year = report(Command::Year {
            year: 2026,
            today: today(),
        });
        let summary = report(Command::Summary(2026));
        let day = report(Command::Day(date(2026, 6, 8)));
        let list = report(Command::List(None));
        [
            ("month", &month, Output::Window, true),
            ("year", &year, Output::Window, true),
            ("summary", &summary, Output::Window, true),
            ("day", &day, Output::Window, false),
            ("list", &list, Output::Window, false),
            ("month on the console", &month, Output::Console, false),
        ]
        .into_iter()
        .for_each(|(case, report, output, expected)| {
            assert_eq!(opens(report, output), expected, "{case}");
        });
    }

    #[test]
    fn rn_22_keys_scroll_or_close_the_window() {
        [
            (KeyEvent::from(KeyCode::Esc), Some(Message::Quit)),
            (KeyEvent::from(KeyCode::Char('q')), Some(Message::Quit)),
            (
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                Some(Message::Quit),
            ),
            (KeyEvent::from(KeyCode::Up), Some(Message::Up)),
            (KeyEvent::from(KeyCode::Char('k')), Some(Message::Up)),
            (KeyEvent::from(KeyCode::Down), Some(Message::Down)),
            (KeyEvent::from(KeyCode::Char('j')), Some(Message::Down)),
            (KeyEvent::from(KeyCode::PageUp), Some(Message::PageUp)),
            (KeyEvent::from(KeyCode::PageDown), Some(Message::PageDown)),
            (KeyEvent::from(KeyCode::Char(' ')), Some(Message::PageDown)),
            (KeyEvent::from(KeyCode::Home), Some(Message::Home)),
            (KeyEvent::from(KeyCode::Char('g')), Some(Message::Home)),
            (KeyEvent::from(KeyCode::End), Some(Message::End)),
            (KeyEvent::from(KeyCode::Char('G')), Some(Message::End)),
            (KeyEvent::from(KeyCode::Char('r')), Some(Message::Reload)),
            (KeyEvent::from(KeyCode::Char('R')), Some(Message::Reload)),
            (KeyEvent::from(KeyCode::Char('c')), None),
            (KeyEvent::from(KeyCode::Enter), None),
        ]
        .into_iter()
        .for_each(|(key, expected)| assert_eq!(message(key), expected, "{key:?}"));
    }

    #[test]
    fn rn_22_scrolling_stays_within_the_view() {
        // 10 líneas en una ventana de 4: la primera visible va de 0 a 6.
        let at = |top: usize| numbered(10).scrolled(top);
        [
            ("down", at(0), Message::Down, 1),
            ("up at the start", at(0), Message::Up, 0),
            ("down at the end", at(6), Message::Down, 6),
            ("page down", at(1), Message::PageDown, 5),
            ("page down past the end", at(5), Message::PageDown, 6),
            ("page up", at(5), Message::PageUp, 1),
            ("page up past the start", at(2), Message::PageUp, 0),
            ("home", at(4), Message::Home, 0),
            ("end", at(1), Message::End, 6),
        ]
        .into_iter()
        .for_each(|(case, viewer, message, expected)| {
            let (next, effect) = viewer.update(message, 4);
            assert_eq!((next.top, effect), (expected, Effect::None), "{case}");
        });
    }

    #[test]
    fn rn_22_quit_closes_without_changing_the_view() {
        let viewer = numbered(10).scrolled(3);
        assert_eq!(
            viewer.clone().update(Message::Quit, 4),
            (viewer, Effect::Quit)
        );
    }

    #[test]
    fn rn_22_shows_the_lines_that_fit_and_the_help() {
        let viewer = numbered(10).scrolled(2);
        assert_eq!(
            viewer.visible(3).collect::<Vec<_>>(),
            ["línea 3", "línea 4", "línea 5"]
        );
        assert_eq!(
            viewer.help(3),
            "Líneas 3–5 de 10 · ↑ ↓ RePág AvPág Inicio Fin: desplazar · r: recargar · Esc o q: salir"
        );
        assert_eq!(numbered(3).help(3), "r: recargar · Esc o q: salir");
    }

    #[test]
    fn rn_22_a_taller_terminal_leaves_no_gap_below() {
        // Desplazada hasta el final con 4 filas, y luego el terminal pasa a 8.
        let viewer = numbered(10).update(Message::End, 4).0;
        assert_eq!(viewer.visible(8).next(), Some("línea 3"));
        assert_eq!(viewer.update(Message::Up, 8).0.top, 1);
    }

    #[test]
    fn rn_22_draws_the_visible_lines_and_the_help_below() {
        let mut screen = Vec::new();
        let next = numbered(10).scrolled(2).screen((80, 4));
        assert!(draw(&mut screen, &Screen::default(), &next).is_ok());
        let screen = String::from_utf8_lossy(&screen);
        let help = screen.find("Líneas 3–5 de 10");
        assert!(help.is_some(), "{screen}");
        ["línea 3", "línea 4", "línea 5"]
            .into_iter()
            .for_each(|line| assert!(screen.find(line) < help, "{line}: {screen}"));
        assert!(!screen.contains("línea 2\u{1b}"), "{screen}");
        assert!(!screen.contains("línea 6"), "{screen}");
    }

    #[test]
    fn rn_28_reload_asks_for_the_file_without_changing_the_view() {
        let viewer = numbered(10).scrolled(3);
        assert_eq!(
            viewer.clone().update(Message::Reload, 4),
            (viewer, Effect::Reload)
        );
    }

    #[test]
    fn rn_28_a_reload_shows_the_new_text_from_the_same_line() {
        let reloaded = numbered(10).scrolled(3).reloaded(Ok(
            "nueva 1\nnueva 2\nnueva 3\nnueva 4\nnueva 5\nnueva 6\nnueva 7\n".to_owned(),
        ));
        assert_eq!(
            reloaded.visible(2).collect::<Vec<_>>(),
            ["nueva 4", "nueva 5"]
        );
        // Si el texto nuevo es más corto, se ve su final.
        let shorter = numbered(10)
            .scrolled(8)
            .reloaded(Ok("nueva 1\nnueva 2\nnueva 3\n".to_owned()));
        assert_eq!(
            shorter.visible(2).collect::<Vec<_>>(),
            ["nueva 2", "nueva 3"]
        );
    }

    #[test]
    fn rn_28_a_failed_reload_keeps_the_view_and_shows_the_error_until_the_next_one() {
        let failed = numbered(3).reloaded(Err("JSON no válido".to_owned()));
        assert_eq!(
            failed.visible(3).collect::<Vec<_>>(),
            ["línea 1", "línea 2", "línea 3"]
        );
        assert_eq!(
            failed.help(3),
            "No se ha podido recargar: JSON no válido · r: recargar · Esc o q: salir"
        );
        assert_eq!(
            failed.reloaded(Ok("nueva\n".to_owned())).help(3),
            "r: recargar · Esc o q: salir"
        );
    }

    #[test]
    fn rn_31_only_month_and_year_show_the_reservoirs() {
        [
            (
                "month",
                report(Command::Month {
                    year: 2026,
                    month: Month::June,
                    today: today(),
                }),
                true,
            ),
            (
                "year",
                report(Command::Year {
                    year: 2026,
                    today: today(),
                }),
                true,
            ),
            ("summary", report(Command::Summary(2026)), false),
            ("day", report(Command::Day(date(2026, 6, 8))), false),
            ("list", report(Command::List(None)), false),
            ("config", report(Command::Config), false),
        ]
        .into_iter()
        .for_each(|(case, report, expected)| {
            assert_eq!(shows_reservoirs(&report), expected, "{case}");
        });
    }

    #[test]
    fn rn_31_only_the_rows_that_changed_are_written_again() {
        let before = numbered(3).screen((80, 4));
        let after = numbered(3)
            .retexted("línea 1\nnueva 2\nlínea 3\n")
            .screen((80, 4));
        let mut written = Vec::new();
        assert!(draw(&mut written, &before, &after).is_ok());
        let written = String::from_utf8_lossy(&written);
        assert!(written.contains("nueva 2"), "{written}");
        assert!(!written.contains("línea 1"), "{written}");
        assert!(!written.contains("línea 3"), "{written}");
        assert!(!written.contains("\u{1b}[2J"), "{written}");
    }

    #[test]
    fn rn_31_a_resized_terminal_is_cleared_and_written_again() {
        let before = numbered(3).screen((80, 4));
        let mut written = Vec::new();
        assert!(draw(&mut written, &before, &numbered(3).screen((100, 4))).is_ok());
        let written = String::from_utf8_lossy(&written);
        assert!(written.contains("\u{1b}[2J"), "{written}");
        ["línea 1", "línea 2", "línea 3", "r: recargar"]
            .into_iter()
            .for_each(|line| assert!(written.contains(line), "{line}: {written}"));
    }

    #[test]
    fn rn_31_new_reservoir_rows_keep_the_line_and_the_reload_error() {
        let viewer = numbered(10)
            .scrolled(3)
            .reloaded(Err("JSON no válido".to_owned()))
            .retexted(
                &(1..=10)
                    .map(|line| format!("nueva {line}\n"))
                    .collect::<String>(),
            );
        assert_eq!(
            viewer.visible(2).collect::<Vec<_>>(),
            ["nueva 4", "nueva 5"]
        );
        assert!(
            viewer
                .help(2)
                .starts_with("No se ha podido recargar: JSON no válido")
        );
    }

    #[test]
    fn rn_31_the_screen_fills_the_text_rows_and_puts_the_help_last() {
        let screen = numbered(2).screen((80, 4));
        assert_eq!(
            screen
                .rows
                .iter()
                .map(|row| anstream::adapter::strip_str(row).to_string())
                .collect::<Vec<_>>(),
            ["línea 1", "línea 2", "", "r: recargar · Esc o q: salir"]
        );
    }

    impl Viewer {
        /// La misma vista con `top` como primera línea, sin pasar por `update`.
        fn scrolled(self, top: usize) -> Self {
            Self { top, ..self }
        }
    }
}
