//! Command-line arguments.

use std::path::PathBuf;

use clap::Parser;

/// Lists the open pull requests of the configured GitHub repositories.
#[derive(Debug, Parser)]
pub struct Args {
    /// Path to the TOML file listing the repositories.
    #[arg(long, default_value = "projects.toml")]
    pub config: PathBuf,
}
