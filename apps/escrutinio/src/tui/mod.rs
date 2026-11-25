//! Adaptador de entrada y presentación: la interfaz de terminal.
//!
//! [`app`] guarda el estado y [`view`] lo dibuja; este módulo es el bucle de eventos, que cada
//! `interval` envía a la app el mensaje de `on_tick`: avanzar la maqueta o los snapshots del
//! servicio. Mientras dura el destello de una consulta (RN-14), también envía `Message::Flash` cada
//! `FLASH_STEP`.

pub mod app;
pub mod view;

use std::io;
use std::time::{Duration, Instant};

use ratatui::DefaultTerminal;
use ratatui::crossterm::event;

use crate::tui::app::{App, Effect, FLASH_STEPS, Message, message};

/// Lo que dura cada paso del destello: con `FLASH_STEPS` pasos, un segundo (RN-14).
pub const FLASH_STEP: Duration = Duration::from_millis(1000 / FLASH_STEPS as u64);

/// Qué toca cuando se acaba la espera: el siguiente paso del destello o el aviso periódico.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Due {
    Flash,
    Tick,
}

/// Lo que vence antes, si hay un destello en marcha; sin destello, siempre el aviso periódico.
fn due(flashing: bool, next_flash: Instant, next_tick: Instant) -> Due {
    if flashing && next_flash <= next_tick {
        Due::Flash
    } else {
        Due::Tick
    }
}

/// Muestra `app` hasta que el usuario sale. La terminal se restaura también si el bucle falla.
pub fn run(app: App, interval: Duration, on_tick: impl FnMut() -> Message) -> io::Result<()> {
    let mut terminal = ratatui::try_init()?;
    let result = event_loop(&mut terminal, app, interval, on_tick);
    let restored = ratatui::try_restore();
    result.and(restored)
}

fn event_loop(
    terminal: &mut DefaultTerminal,
    initial: App,
    interval: Duration,
    mut on_tick: impl FnMut() -> Message,
) -> io::Result<()> {
    let mut app = initial;
    let mut next_tick = Instant::now() + interval;
    let mut next_flash = Instant::now() + FLASH_STEP;
    loop {
        terminal.draw(|frame| view::draw(&app, frame))?;
        let which = due(app.flashing(), next_flash, next_tick);
        let deadline = match which {
            Due::Flash => next_flash,
            Due::Tick => next_tick,
        };
        let message = if event::poll(deadline.saturating_duration_since(Instant::now()))? {
            event::read()?.as_key_press_event().and_then(message)
        } else {
            let now = Instant::now();
            match which {
                Due::Flash => {
                    next_flash = now + FLASH_STEP;
                    Some(Message::Flash)
                }
                Due::Tick => {
                    next_tick = now + interval;
                    Some(on_tick())
                }
            }
        };
        if let Some(message) = message {
            let was_flashing = app.flashing();
            let (next, effect) = app.update(message);
            match effect {
                Effect::Quit => return Ok(()),
                Effect::None => {
                    // Un destello nuevo empieza con su paso completo.
                    if next.flashing() && !was_flashing {
                        next_flash = Instant::now() + FLASH_STEP;
                    }
                    app = next;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flash_takes_one_second_in_four_steps() {
        assert_eq!(FLASH_STEP, Duration::from_millis(250));
        assert_eq!(FLASH_STEP * u32::from(FLASH_STEPS), Duration::from_secs(1));
    }

    #[test]
    fn the_earliest_deadline_decides_and_a_flash_needs_a_flash_going() {
        let now = Instant::now();
        let soon = now + Duration::from_millis(100);
        let later = now + Duration::from_secs(5);
        [
            ("flash first", true, soon, later, Due::Flash),
            ("tick first", true, later, soon, Due::Tick),
            (
                "both at once, the flash goes first",
                true,
                soon,
                soon,
                Due::Flash,
            ),
            ("no flash going, so no flash", false, soon, later, Due::Tick),
        ]
        .into_iter()
        .for_each(|(case, flashing, next_flash, next_tick, expected)| {
            assert_eq!(due(flashing, next_flash, next_tick), expected, "{case}");
        });
    }
}
