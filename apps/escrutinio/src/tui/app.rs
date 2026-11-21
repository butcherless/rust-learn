//! El estado de la interfaz, con The Elm Architecture: `message(key)` da un `Message` y
//! `App::update(self, Message) -> (App, Effect)` da el estado siguiente.

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::mock::Scrutiny;

/// Las filas que salta `PageUp`/`PageDown`.
pub const PAGE: usize = 10;

/// Contra qué se dibuja cada barra (RN-09).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    /// Cada barra, contra el censo de su circunscripción: todas miden lo mismo.
    Own,
    /// Todas, contra el censo mayor: se comparan los tamaños.
    Shared,
}

impl Scale {
    pub fn label(self) -> &'static str {
        match self {
            Self::Own => "proporcional",
            Self::Shared => "absoluta",
        }
    }

    fn toggled(self) -> Self {
        match self {
            Self::Own => Self::Shared,
            Self::Shared => Self::Own,
        }
    }
}

/// En qué punto está el escrutinio simulado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Running,
    Paused,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// Toca avanzar la simulación.
    Tick,
    TogglePause,
    Restart,
    ToggleScale,
    Up,
    Down,
    PageUp,
    PageDown,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    None,
    Quit,
}

/// El mensaje de una tecla, si tiene.
pub fn message(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Some(Message::Quit),
        KeyCode::Char(' ') | KeyCode::Char('p') => Some(Message::TogglePause),
        KeyCode::Char('r') => Some(Message::Restart),
        KeyCode::Char('e') => Some(Message::ToggleScale),
        KeyCode::Up | KeyCode::Char('k') => Some(Message::Up),
        KeyCode::Down | KeyCode::Char('j') => Some(Message::Down),
        KeyCode::PageUp => Some(Message::PageUp),
        KeyCode::PageDown => Some(Message::PageDown),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct App {
    scrutiny: Scrutiny,
    /// El escrutinio del principio, para `Restart`.
    initial: Scrutiny,
    paused: bool,
    scale: Scale,
    /// La primera fila que se ve. La vista la limita a lo que cabe.
    offset: usize,
}

impl App {
    pub fn new(scrutiny: Scrutiny) -> Self {
        Self {
            initial: scrutiny.clone(),
            scrutiny,
            paused: false,
            scale: Scale::Own,
            offset: 0,
        }
    }

    pub fn update(self, message: Message) -> (Self, Effect) {
        let last = self.scrutiny.rows().len().saturating_sub(1);
        let offset = self.offset;
        let next = match message {
            Message::Quit => return (self, Effect::Quit),
            Message::Tick if self.status() == Status::Running => Self {
                scrutiny: self.scrutiny.advance(),
                ..self
            },
            Message::Tick => self,
            Message::TogglePause => Self {
                paused: !self.paused,
                ..self
            },
            Message::Restart => Self {
                scrutiny: self.initial.clone(),
                paused: false,
                ..self
            },
            Message::ToggleScale => Self {
                scale: self.scale.toggled(),
                ..self
            },
            Message::Up => self.scrolled_to(offset.saturating_sub(1)),
            Message::Down => self.scrolled_to((offset + 1).min(last)),
            Message::PageUp => self.scrolled_to(offset.saturating_sub(PAGE)),
            Message::PageDown => self.scrolled_to((offset + PAGE).min(last)),
        };
        (next, Effect::None)
    }

    fn scrolled_to(self, offset: usize) -> Self {
        Self { offset, ..self }
    }

    pub fn scrutiny(&self) -> &Scrutiny {
        &self.scrutiny
    }

    pub fn scale(&self) -> Scale {
        self.scale
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn status(&self) -> Status {
        if self.scrutiny.finished() {
            Status::Finished
        } else if self.paused {
            Status::Paused
        } else {
            Status::Running
        }
    }
}

#[cfg(test)]
pub mod fixtures {
    use super::*;
    use crate::mock::fixtures::scrutiny;

    pub fn app() -> App {
        App::new(scrutiny())
    }

    /// `app` tras cada mensaje, en orden.
    pub fn apply(app: App, messages: &[Message]) -> App {
        messages
            .iter()
            .fold(app, |app, &message| app.update(message).0)
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{app, apply};
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    #[test]
    fn keys_map_to_messages() {
        [
            (KeyCode::Char('q'), Some(Message::Quit)),
            (KeyCode::Esc, Some(Message::Quit)),
            (KeyCode::Char(' '), Some(Message::TogglePause)),
            (KeyCode::Char('p'), Some(Message::TogglePause)),
            (KeyCode::Char('r'), Some(Message::Restart)),
            (KeyCode::Char('e'), Some(Message::ToggleScale)),
            (KeyCode::Up, Some(Message::Up)),
            (KeyCode::Char('k'), Some(Message::Up)),
            (KeyCode::Down, Some(Message::Down)),
            (KeyCode::Char('j'), Some(Message::Down)),
            (KeyCode::PageUp, Some(Message::PageUp)),
            (KeyCode::PageDown, Some(Message::PageDown)),
            (KeyCode::Char('x'), None),
        ]
        .into_iter()
        .for_each(|(code, expected)| {
            assert_eq!(
                message(KeyEvent::new(code, KeyModifiers::NONE)),
                expected,
                "{code:?}"
            );
        });
    }

    #[test]
    fn quit_ends_without_changing_the_state() {
        let (next, effect) = app().update(Message::Quit);
        assert_eq!(effect, Effect::Quit);
        assert_eq!(next, app());
    }

    #[test]
    fn ticks_advance_only_while_running() {
        let counted = |app: &App| app.scrutiny().total().counted;
        let running = apply(app(), &[Message::Tick]);
        assert!(counted(&running) > 0);
        assert_eq!(running.status(), Status::Running);

        let paused = apply(running.clone(), &[Message::TogglePause, Message::Tick]);
        assert_eq!(paused.status(), Status::Paused);
        assert_eq!(counted(&paused), counted(&running));

        let resumed = apply(paused, &[Message::TogglePause, Message::Tick]);
        assert!(counted(&resumed) > counted(&running));
    }

    #[test]
    fn finishes_and_then_ignores_ticks() {
        let finished = apply(app(), &[Message::Tick; 200]);
        assert_eq!(finished.status(), Status::Finished);
        assert_eq!(apply(finished.clone(), &[Message::Tick]), finished);
    }

    #[test]
    fn restart_goes_back_to_no_votes_and_running() {
        let restarted = apply(
            app(),
            &[Message::Tick, Message::TogglePause, Message::Restart],
        );
        assert_eq!(restarted.scrutiny().total().counted, 0);
        assert_eq!(restarted.status(), Status::Running);
    }

    #[test]
    fn the_scale_toggles() {
        assert_eq!(app().scale(), Scale::Own);
        assert_eq!(apply(app(), &[Message::ToggleScale]).scale(), Scale::Shared);
        assert_eq!(
            apply(app(), &[Message::ToggleScale, Message::ToggleScale]).scale(),
            Scale::Own
        );
        assert_eq!(Scale::Own.label(), "proporcional");
        assert_eq!(Scale::Shared.label(), "absoluta");
    }

    #[test]
    fn scrolling_stays_within_the_rows() {
        [
            (vec![Message::Up], 0),
            (vec![Message::Down, Message::Down], 2),
            (vec![Message::PageDown], PAGE),
            (vec![Message::PageDown, Message::Up], PAGE - 1),
            (vec![Message::PageDown, Message::PageUp], 0),
            (vec![Message::PageDown; 10], 51),
            (vec![Message::Down; 60], 51),
        ]
        .into_iter()
        .for_each(|(messages, offset)| {
            assert_eq!(apply(app(), &messages).offset(), offset, "{messages:?}");
        });
    }
}
