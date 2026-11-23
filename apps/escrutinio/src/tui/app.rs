//! El estado de la interfaz, con The Elm Architecture: `message(key)` da un `Message` y
//! `App::update(self, Message) -> (App, Effect)` da el estado siguiente.
//!
//! El escrutado viene de la maqueta (`Source::Mock`, RN-06), que avanza con cada `Tick`, o del
//! servicio (`Source::Service`, RN-11), que llega con `Snapshots` cada 10 s. Con el servicio no hay
//! pausa ni reinicio, y el censo es el de 2023 (RN-10).

use escrutinio_contrato::ProvinceSnapshot;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::census::Election;
use crate::feed::counted;
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

/// En qué punto está el escrutinio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Running,
    Paused,
    Finished,
    /// Siguiendo al servicio, que respondió la última vez.
    Following,
    /// La última petición al servicio falló: se ve lo último que llegó.
    Unreachable,
}

/// De dónde viene el escrutado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Mock,
    /// El servicio en `url`, con el error de la última petición si falló.
    Service {
        url: String,
        error: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// Toca avanzar la simulación.
    Tick,
    /// La respuesta del servicio, o el texto de su error.
    Snapshots(Result<Vec<ProvinceSnapshot>, String>),
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
    source: Source,
    election: Election,
    /// El escrutinio del principio, para `Restart`.
    initial: Scrutiny,
    paused: bool,
    scale: Scale,
    /// La primera fila que se ve. La vista la limita a lo que cabe.
    offset: usize,
}

impl App {
    /// La maqueta de 2026, con el escrutado simulado.
    pub fn new(scrutiny: Scrutiny) -> Self {
        Self {
            initial: scrutiny.clone(),
            scrutiny,
            source: Source::Mock,
            election: Election::Generales2026,
            paused: false,
            scale: Scale::Own,
            offset: 0,
        }
    }

    /// Sigue al servicio en `url`; `scrutiny` debe tener el censo de 2023.
    pub fn following(scrutiny: Scrutiny, url: &str) -> Self {
        Self {
            source: Source::Service {
                url: url.to_owned(),
                error: None,
            },
            election: Election::Generales2023,
            ..Self::new(scrutiny)
        }
    }

    pub fn update(self, message: Message) -> (Self, Effect) {
        let last = self.scrutiny.rows().len().saturating_sub(1);
        let offset = self.offset;
        let mock = self.source == Source::Mock;
        let next = match message {
            Message::Quit => return (self, Effect::Quit),
            // Ya terminada, la maqueta sigue avanzando sin votos, para que sus pilotos pasen a verde.
            Message::Tick if matches!(self.status(), Status::Running | Status::Finished) => Self {
                scrutiny: self.scrutiny.advance(),
                ..self
            },
            Message::Tick => self,
            Message::Snapshots(result) => self.received(result),
            Message::TogglePause if mock => Self {
                paused: !self.paused,
                ..self
            },
            Message::Restart if mock => Self {
                scrutiny: self.initial.clone(),
                paused: false,
                ..self
            },
            Message::TogglePause | Message::Restart => self,
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

    /// Con los snapshots, el escrutado de cada fila; con un error, lo mismo que había.
    fn received(self, result: Result<Vec<ProvinceSnapshot>, String>) -> Self {
        match (self.source.clone(), result) {
            (Source::Mock, _) => self,
            (Source::Service { url, .. }, Ok(snapshots)) => Self {
                scrutiny: self.scrutiny.with_counted(&counted(&snapshots)),
                source: Source::Service { url, error: None },
                ..self
            },
            (Source::Service { url, .. }, Err(error)) => Self {
                source: Source::Service {
                    url,
                    error: Some(error),
                },
                ..self
            },
        }
    }

    pub fn source(&self) -> &Source {
        &self.source
    }

    pub fn election(&self) -> Election {
        self.election
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
        match &self.source {
            Source::Service { error: None, .. } => return Status::Following,
            Source::Service { error: Some(_), .. } => return Status::Unreachable,
            Source::Mock => {}
        }
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

    pub const SERVICE: &str = "http://localhost:8080";

    /// Siguiendo al servicio, con el censo de 2023 y sin votos.
    pub fn following() -> App {
        let scrutiny = Scrutiny::start(
            &crate::census::ordered(Election::Generales2023),
            crate::domain::DEFAULT_TURNOUT,
            1,
        );
        App::following(scrutiny, SERVICE)
    }

    /// `app` tras cada mensaje, en orden.
    pub fn apply(app: App, messages: &[Message]) -> App {
        messages
            .iter()
            .fold(app, |app, message| app.update(message.clone()).0)
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{SERVICE, app, apply, following};
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
    fn finishes_and_then_ticks_change_no_row() {
        let finished = apply(app(), &vec![Message::Tick; 200]);
        assert_eq!(finished.status(), Status::Finished);
        let ticked = apply(finished.clone(), &[Message::Tick]);
        assert_eq!(ticked.status(), Status::Finished);
        assert_eq!(ticked.scrutiny().rows(), finished.scrutiny().rows());
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
    fn follows_the_snapshots_of_the_service() {
        use crate::feed::fixtures::snapshot;
        let app = following();
        assert_eq!(
            (app.status(), app.election()),
            (Status::Following, Election::Generales2023)
        );
        let counted = |app: &App| app.scrutiny().total().counted;
        let received = apply(
            app,
            &[Message::Snapshots(Ok(vec![
                snapshot("28", 1_000),
                snapshot("51", 7),
            ]))],
        );
        assert_eq!(counted(&received), 1_007);
        // Los ticks, la pausa y el reinicio no cambian nada con el servicio.
        let keys = apply(
            received.clone(),
            &[Message::Tick, Message::TogglePause, Message::Restart],
        );
        assert_eq!(keys, received);
    }

    #[test]
    fn keeps_the_last_data_when_the_service_fails_and_recovers() {
        use crate::feed::fixtures::snapshot;
        let ok = Message::Snapshots(Ok(vec![snapshot("28", 1_000)]));
        let failed = apply(
            following(),
            &[ok.clone(), Message::Snapshots(Err("caído".to_owned()))],
        );
        assert_eq!(failed.status(), Status::Unreachable);
        assert_eq!(failed.scrutiny().total().counted, 1_000);
        assert_eq!(
            failed.source(),
            &Source::Service {
                url: SERVICE.to_owned(),
                error: Some("caído".to_owned())
            }
        );
        assert_eq!(apply(failed, &[ok]).status(), Status::Following);
    }

    #[test]
    fn the_mock_ignores_snapshots() {
        let ignored = apply(app(), &[Message::Snapshots(Ok(vec![]))]);
        assert_eq!(ignored, app());
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
