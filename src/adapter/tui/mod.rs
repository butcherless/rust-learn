//! Inbound and presentation adapter: an interactive terminal UI.

pub mod app;

use std::io;

use crossterm::event::EventStream;
use futures::StreamExt;
use ratatui::DefaultTerminal;

use crate::adapter::tui::app::{Action, App};
use crate::domain::model::Project;
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
    let reports = result?;
    restored.map(|()| reports)
}

async fn event_loop<S: PullRequestSource>(
    terminal: &mut DefaultTerminal,
    source: &S,
    projects: &[Project],
) -> io::Result<Vec<ProjectReport>> {
    let mut app = App::new();
    let mut events = EventStream::new();
    fetch(terminal, &mut app, source, projects).await?;
    loop {
        terminal.draw(|frame| app.draw(frame))?;
        let Some(event) = events.next().await.transpose()? else {
            break;
        };
        let Some(key) = event.as_key_press_event() else {
            continue;
        };
        match app.handle_key(key) {
            Action::None => {}
            Action::Quit => break,
            Action::Refresh => fetch(terminal, &mut app, source, projects).await?,
            Action::Open(url) => {
                let result = open::that_detached(url.as_str());
                app.opened(&url, result);
            }
        }
    }
    Ok(app.into_reports())
}

/// Shows the loading state, then fetches every project.
async fn fetch<S: PullRequestSource>(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    source: &S,
    projects: &[Project],
) -> io::Result<()> {
    app.start_loading();
    terminal.draw(|frame| app.draw(frame))?;
    let reports = list_open_prs(source, projects.to_vec(), MAX_CONCURRENCY).await;
    app.set_reports(reports);
    Ok(())
}
