//! Composition root.

use std::env;
use std::process::ExitCode;

use anyhow::{Context, bail};
use clap::Parser;
use pr_checker::adapter::cli::args::Args;
use pr_checker::adapter::cli::render::render;
use pr_checker::adapter::config::load_projects;
use pr_checker::adapter::github::{DEFAULT_BASE_URL, GithubClient, GithubToken};
use pr_checker::adapter::tui;
use pr_checker::domain::model::Project;
use pr_checker::service::{MAX_CONCURRENCY, ProjectReport, has_failures, list_open_prs};

const EXIT_FAILURES: u8 = 1;
const EXIT_STARTUP: u8 = 2;
const TOKEN_VARIABLE: &str = "GITHUB_TOKEN";

#[tokio::main]
async fn main() -> ExitCode {
    match run(&Args::parse()).await {
        Ok(reports) => exit_code(&reports),
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(EXIT_STARTUP)
        }
    }
}

/// Wires the adapters and shows the reports in the requested mode.
async fn run(args: &Args) -> anyhow::Result<Vec<ProjectReport>> {
    let (client, projects) = startup(args)?;
    if args.tui {
        tui::run(&client, projects)
            .await
            .context("cannot run the terminal UI")
    } else {
        let reports = list_open_prs(&client, projects, MAX_CONCURRENCY).await;
        println!("{}", render(&reports));
        Ok(reports)
    }
}

fn exit_code(reports: &[ProjectReport]) -> ExitCode {
    if has_failures(reports) {
        ExitCode::from(EXIT_FAILURES)
    } else {
        ExitCode::SUCCESS
    }
}

fn startup(args: &Args) -> anyhow::Result<(GithubClient, Vec<Project>)> {
    let projects = load_projects(&args.config).context("cannot load the project list")?;
    let client = GithubClient::new(DEFAULT_BASE_URL, read_token()?)?;
    Ok((client, projects))
}

fn read_token() -> anyhow::Result<GithubToken> {
    match env::var(TOKEN_VARIABLE) {
        Ok(value) if !value.is_empty() => Ok(GithubToken::new(value)),
        Ok(_) | Err(env::VarError::NotPresent) => {
            bail!("the {TOKEN_VARIABLE} environment variable is missing or empty")
        }
        Err(env::VarError::NotUnicode(_)) => {
            bail!("the {TOKEN_VARIABLE} environment variable is not valid UTF-8")
        }
    }
}
