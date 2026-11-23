//! Adaptador de entrada y presentación: la interfaz de terminal.
//!
//! [`app`] guarda el estado y [`view`] lo dibuja; este módulo es el bucle de eventos, que cada
//! `interval` envía a la app el mensaje de `on_tick`: avanzar la maqueta o los snapshots del
//! servicio.

pub mod app;
pub mod view;

use std::io;
use std::time::{Duration, Instant};

use ratatui::DefaultTerminal;
use ratatui::crossterm::event;

use crate::tui::app::{App, Effect, Message, message};

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
    loop {
        terminal.draw(|frame| view::draw(&app, frame))?;
        let timeout = next_tick.saturating_duration_since(Instant::now());
        let message = if event::poll(timeout)? {
            event::read()?.as_key_press_event().and_then(message)
        } else {
            next_tick = Instant::now() + interval;
            Some(on_tick())
        };
        if let Some(message) = message {
            let (next, effect) = app.update(message);
            match effect {
                Effect::Quit => return Ok(()),
                Effect::None => app = next,
            }
        }
    }
}
