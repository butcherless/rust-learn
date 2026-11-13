//! Inbound and presentation adapter: an interactive terminal UI.
//!
//! [`app`] holds the pure model; this module is the event loop that performs its effects. It
//! waits for keys, the blink clock and the reports of a fetch at the same time, so the screen
//! keeps responding while fetching.

pub mod app;
pub mod view;

use std::io;
use std::time::Duration;

use crossterm::event::EventStream;
use futures::{Stream, StreamExt};
use ratatui::DefaultTerminal;
use tokio::time::{Interval, MissedTickBehavior};

use crate::adapter::tui::app::{App, Effect, Message, message};
use crate::domain::model::{PrUrl, Project};
use crate::domain::port::PullRequestSource;
use crate::service::{MAX_CONCURRENCY, ProjectReport, stream_open_prs};

/// Half of the light's blink: lit, then dark (FR13).
const BLINK: Duration = Duration::from_millis(500);

/// Runs the UI until the user quits, and returns the last reports shown.
///
/// The terminal is restored before returning, also when the loop fails.
pub async fn run<S: PullRequestSource>(
    source: &S,
    projects: Vec<Project>,
) -> io::Result<Vec<ProjectReport>> {
    let mut terminal = ratatui::try_init()?;
    let result = event_loop(&mut terminal, source, projects).await;
    let restored = ratatui::try_restore();
    result.and_then(|reports| restored.map(|()| reports))
}

async fn event_loop<S: PullRequestSource>(
    terminal: &mut DefaultTerminal,
    source: &S,
    projects: Vec<Project>,
) -> io::Result<Vec<ProjectReport>> {
    let mut events = EventStream::new();
    let mut blink = tokio::time::interval(BLINK);
    blink.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut fetch = None;
    let (mut app, mut effect) = App::new(projects.clone());
    loop {
        let next = match effect {
            Effect::Quit => return Ok(app.into_reports()),
            Effect::Open(url) => Some(open_in_browser(url)),
            Effect::Fetch => {
                fetch = Some(Box::pin(stream_open_prs(
                    source,
                    projects.clone(),
                    MAX_CONCURRENCY,
                )));
                blink.reset();
                next_message(terminal, &app, &mut events, &mut blink, &mut fetch).await?
            }
            Effect::None => {
                next_message(terminal, &app, &mut events, &mut blink, &mut fetch).await?
            }
        };
        (app, effect) = match next {
            Some(message) => app.update(message),
            None => (app, Effect::None),
        };
    }
}

fn open_in_browser(url: PrUrl) -> Message {
    let result = open::that_detached(url.as_str()).map_err(|error| error.to_string());
    Message::Opened(url, result)
}

/// Draws `app` and waits for whichever comes first: a key, the blink clock or the next report,
/// the last two only while fetching. Events without a message, such as a resize or an unbound
/// key, give `None` so the screen is redrawn. A closed event stream means quit.
async fn next_message<F: Stream<Item = (usize, ProjectReport)> + Unpin>(
    terminal: &mut DefaultTerminal,
    app: &App,
    events: &mut EventStream,
    blink: &mut Interval,
    fetch: &mut Option<F>,
) -> io::Result<Option<Message>> {
    terminal.draw(|frame| view::draw(app, frame))?;
    let fetching = fetch.is_some();
    tokio::select! {
        event = events.next() => Ok(match event.transpose()? {
            Some(event) => event.as_key_press_event().and_then(message),
            None => Some(Message::Quit),
        }),
        _ = blink.tick(), if fetching => Ok(Some(Message::Tick)),
        report = next_report(fetch), if fetching => Ok(Some(report)),
    }
}

/// The next report of the fetch, or [`Message::Finished`] when there are no more, which also
/// ends the fetch.
async fn next_report<F: Stream<Item = (usize, ProjectReport)> + Unpin>(
    fetch: &mut Option<F>,
) -> Message {
    match fetch.as_mut() {
        Some(reports) => match reports.next().await {
            Some((index, report)) => Message::Fetched(index, report),
            None => {
                *fetch = None;
                Message::Finished
            }
        },
        None => futures::future::pending().await,
    }
}
