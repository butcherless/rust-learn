//! Adaptador de salida: la ventana a pantalla completa de `month`, `year` y `summary` (RN-22).
//!
//! Sigue The Elm Architecture: [`message`] convierte una tecla en un [`Message`], y
//! [`Viewer::update`] devuelve el estado siguiente y un [`Effect`]. El estado no sabe nada del
//! terminal; [`run`] es el único código con efectos secundarios.

use std::io::{self, Write};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{
    self, Clear, ClearType, DisableLineWrap, EnableLineWrap, EnterAlternateScreen,
    LeaveAlternateScreen,
};
use crossterm::{execute, queue};

use crate::application::Report;

/// Si el resultado se muestra en la ventana: solo `month`, `year` y `summary`, solo si la salida
/// es un terminal y solo si no se ha pedido `--no-tui`.
pub fn opens(report: &Report, no_tui: bool, terminal: bool) -> bool {
    let fills_screen = match report {
        Report::Month(..) | Report::Year(..) | Report::Summary(..) => true,
        Report::Marks(..) | Report::Day(..) | Report::Config(..) => false,
    };
    fills_screen && terminal && !no_tui
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
    /// Cerrar la ventana.
    Quit,
}

/// Lo que hay que hacer después de [`Viewer::update`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Volver a dibujar y esperar otra tecla.
    None,
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
        _ => None,
    }
}

/// Las líneas de la vista y la primera que se ve.
///
/// `height` es el número de líneas que caben; llega en cada llamada porque el terminal puede
/// cambiar de tamaño.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Viewer {
    lines: Vec<String>,
    top: usize,
}

impl Viewer {
    /// La vista desde su primera línea. `text` es el mismo texto que se imprime en la consola.
    pub fn new(text: &str) -> Self {
        Self {
            lines: text.lines().map(str::to_owned).collect(),
            top: 0,
        }
    }

    /// El estado tras `message`.
    pub fn update(self, message: Message, height: usize) -> (Self, Effect) {
        let top = self.top(height);
        let page = height.max(1);
        let next = match message {
            Message::Quit => return (self, Effect::Quit),
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

    /// La línea de ayuda: las teclas y, si la vista no cabe, qué líneas se ven.
    pub fn help(&self, height: usize) -> String {
        let total = self.lines.len();
        if total <= height {
            "Esc o q: salir".to_owned()
        } else {
            let first = self.top(height) + 1;
            let last = (self.top(height) + height).min(total);
            format!(
                "Líneas {first}–{last} de {total} · ↑ ↓ RePág AvPág Inicio Fin: desplazar · Esc o q: salir"
            )
        }
    }

    /// La primera línea que se ve. Si el terminal ha crecido, sube para no dejar hueco abajo.
    fn top(&self, height: usize) -> usize {
        self.top.min(self.last_top(height))
    }

    fn last_top(&self, height: usize) -> usize {
        self.lines.len().saturating_sub(height)
    }
}

/// Muestra `text` en la pantalla alternativa hasta que se cierra con `Esc` o `q`. El terminal se
/// restaura también si el bucle falla.
pub fn run(text: &str) -> io::Result<()> {
    let mut out = io::stdout();
    terminal::enable_raw_mode()?;
    let result = execute!(out, EnterAlternateScreen, DisableLineWrap, Hide)
        .and_then(|()| event_loop(&mut out, Viewer::new(text)));
    let restored =
        execute!(out, Show, EnableLineWrap, LeaveAlternateScreen).and(terminal::disable_raw_mode());
    result.and(restored)
}

fn event_loop(out: &mut impl Write, initial: Viewer) -> io::Result<()> {
    let mut viewer = initial;
    loop {
        let (_, rows) = terminal::size()?;
        // La última fila es la de ayuda.
        let height = rows.saturating_sub(1).max(1);
        draw(out, &viewer, height)?;
        if let Some(message) = event::read()?.as_key_press_event().and_then(message) {
            let (next, effect) = viewer.update(message, usize::from(height));
            match effect {
                Effect::Quit => return Ok(()),
                Effect::None => viewer = next,
            }
        }
    }
}

fn draw(out: &mut impl Write, viewer: &Viewer, height: u16) -> io::Result<()> {
    queue!(out, Clear(ClearType::All))?;
    viewer
        .visible(usize::from(height))
        .zip(0..height)
        .try_for_each(|(line, row)| {
            queue!(
                out,
                MoveTo(0, row),
                Print(line),
                SetAttribute(Attribute::Reset)
            )
        })?;
    queue!(
        out,
        MoveTo(0, height),
        SetForegroundColor(Color::DarkGrey),
        Print(viewer.help(usize::from(height))),
        ResetColor
    )?;
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{Command, Outcome, execute};
    use crate::domain::Month;
    use crate::domain::fixtures::{calendar, date};

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
        });
        let year = report(Command::Year(2026));
        let summary = report(Command::Summary(2026));
        let day = report(Command::Day(date(2026, 6, 8)));
        let list = report(Command::List(None));
        [
            ("month", &month, false, true, true),
            ("year", &year, false, true, true),
            ("summary", &summary, false, true, true),
            ("day", &day, false, true, false),
            ("list", &list, false, true, false),
            ("month redirected", &month, false, false, false),
            ("month with --no-tui", &month, true, true, false),
        ]
        .into_iter()
        .for_each(|(case, report, no_tui, terminal, expected)| {
            assert_eq!(opens(report, no_tui, terminal), expected, "{case}");
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
            "Líneas 3–5 de 10 · ↑ ↓ RePág AvPág Inicio Fin: desplazar · Esc o q: salir"
        );
        assert_eq!(numbered(3).help(3), "Esc o q: salir");
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
        assert!(draw(&mut screen, &numbered(10).scrolled(2), 3).is_ok());
        let screen = String::from_utf8_lossy(&screen);
        let help = screen.find("Líneas 3–5 de 10");
        assert!(help.is_some(), "{screen}");
        ["línea 3", "línea 4", "línea 5"]
            .into_iter()
            .for_each(|line| assert!(screen.find(line) < help, "{line}: {screen}"));
        assert!(!screen.contains("línea 2\u{1b}"), "{screen}");
        assert!(!screen.contains("línea 6"), "{screen}");
    }

    impl Viewer {
        /// La misma vista con `top` como primera línea, sin pasar por `update`.
        fn scrolled(self, top: usize) -> Self {
            Self { top, ..self }
        }
    }
}
