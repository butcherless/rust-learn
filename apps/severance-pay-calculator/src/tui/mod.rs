//! Inbound and presentation adapter: an interactive terminal UI.
//!
//! [`app`] holds the state and [`view`] draws it; this module is the event loop that performs
//! their effects.

pub mod app;
pub mod view;

use std::io;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event;

use crate::domain::Input;
use crate::tui::app::{App, Effect, message};

/// Runs the UI, starting from `input`'s values, until the user quits. The terminal is restored
/// also when the loop fails.
pub fn run(input: Input) -> io::Result<()> {
    let mut terminal = ratatui::try_init()?;
    let result = event_loop(&mut terminal, App::from(input));
    let restored = ratatui::try_restore();
    result.and(restored)
}

fn event_loop(terminal: &mut DefaultTerminal, initial: App) -> io::Result<()> {
    let mut app = initial;
    loop {
        terminal.draw(|frame| view::draw(&app, frame))?;
        if let Some(message) = event::read()?.as_key_press_event().and_then(message) {
            let (next, effect) = app.update(message);
            match effect {
                Effect::Quit => return Ok(()),
                Effect::None => app = next,
            }
        }
    }
}
