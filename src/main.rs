//! Composition root.

use std::env;
use std::process::ExitCode;

use anyhow::{Context, bail};
use clap::Parser;
use rust_learn::adapter::cli::args::Args;
use rust_learn::adapter::cli::render::render;
use rust_learn::adapter::config::load_projects;
use rust_learn::adapter::github::{DEFAULT_BASE_URL, GithubClient, GithubToken};
use rust_learn::adapter::tui;
use rust_learn::domain::model::Project;
use rust_learn::service::{MAX_CONCURRENCY, ProjectReport, has_failures, list_open_prs};

const EXIT_FAILURES: u8 = 1;
const EXIT_STARTUP: u8 = 2;
const TOKEN_VARIABLE: &str = "GITHUB_TOKEN";

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    let reports = match startup(&args) {
        Ok((client, projects)) if args.tui => tui::run(&client, projects)
            .await
            .context("cannot run the terminal UI"),
        Ok((client, projects)) => {
            let reports = list_open_prs(&client, projects, MAX_CONCURRENCY).await;
            println!("{}", render(&reports));
            Ok(reports)
        }
        Err(error) => Err(error),
    };
    match reports {
        Ok(reports) => exit_code(&reports),
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(EXIT_STARTUP)
        }
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
    let token = read_token()?;
    let client = GithubClient::new(DEFAULT_BASE_URL, token)?;
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
