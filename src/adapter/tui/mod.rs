//! Inbound and presentation adapter: an interactive terminal UI.
//!
//! [`app`] holds the pure model; this module is the event loop that performs its effects.

pub mod app;

use std::io;

use crossterm::event::EventStream;
use futures::StreamExt;
use ratatui::DefaultTerminal;

use crate::adapter::tui::app::{App, Effect, Message, message};
use crate::domain::model::{PrUrl, Project};
use crate::domain::port::PullRequestSource;
use crate::service::{MAX_CONCURRENCY, ProjectReport, list_open_prs};

/// Runs the UI until the user quits, and returns the last reports shown.
///
/// The terminal is restored before returning, also when the loop fails.
pub async fn run<S: PullRequestSource>(
    source: &S,
    projects: Vec<Project>,
) -> io::Result<Vec<ProjectReport>> {
    let mut terminal = ratatui::try_init()?;
    let result = event_loop(&mut terminal, source, &projects).await;
    let restored = ratatui::try_restore();
    result.and_then(|reports| restored.map(|()| reports))
}

async fn event_loop<S: PullRequestSource>(
    terminal: &mut DefaultTerminal,
    source: &S,
    projects: &[Project],
) -> io::Result<Vec<ProjectReport>> {
    let mut events = EventStream::new();
    let (mut app, mut effect) = (App::default(), Effect::Fetch);
    loop {
        let next = match effect {
            Effect::Quit => return Ok(app.into_reports()),
            Effect::Fetch => Some(fetch(terminal, &app, source, projects).await?),
            Effect::Open(url) => Some(open_in_browser(url)),
            Effect::None => next_message(terminal, &app, &mut events).await?,
        };
        (app, effect) = match next {
            Some(message) => app.update(message),
            None => (app, Effect::None),
        };
    }
}

/// Draws the loading state, then fetches every project.
async fn fetch<S: PullRequestSource>(
    terminal: &mut DefaultTerminal,
    app: &App,
    source: &S,
    projects: &[Project],
) -> io::Result<Message> {
    terminal.draw(|frame| app.view(frame))?;
    let reports = list_open_prs(source, projects.to_vec(), MAX_CONCURRENCY).await;
    Ok(Message::Loaded(reports))
}

fn open_in_browser(url: PrUrl) -> Message {
    let result = open::that_detached(url.as_str()).map_err(|error| error.to_string());
    Message::Opened(url, result)
}

/// Draws `app` and waits for the next event. Events without a message, such as a resize or an
/// unbound key, give `None` so the screen is redrawn. A closed event stream means quit.
async fn next_message(
    terminal: &mut DefaultTerminal,
    app: &App,
    events: &mut EventStream,
) -> io::Result<Option<Message>> {
    terminal.draw(|frame| app.view(frame))?;
    Ok(match events.next().await.transpose()? {
        Some(event) => event.as_key_press_event().and_then(message),
        None => Some(Message::Quit),
    })
}
