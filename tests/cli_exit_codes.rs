//! The compiled binary's startup errors: exit code 2 and a message on stderr (FR2, FR7).
//!
//! Every case fails before any HTTP call, so no network or real token is used.

use std::process::{Command, Output};

const BINARY: &str = env!("CARGO_BIN_EXE_rust-learn");
const STARTUP_ERROR: i32 = 2;
const VALID_CONFIG: &str = "tests/fixtures/projects.toml";

/// Runs the binary from the crate root with `args`, and `GITHUB_TOKEN` set to `token` or unset.
fn run(args: &[&str], token: Option<&str>) -> Output {
    Command::new(BINARY)
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_remove("GITHUB_TOKEN")
        .envs(token.map(|value| ("GITHUB_TOKEN", value)))
        .output()
        .unwrap()
}

#[test]
fn startup_errors_exit_with_2_and_explain_why() {
    let missing_token = "the GITHUB_TOKEN environment variable is missing or empty";
    [
        (vec!["--config", VALID_CONFIG], None, missing_token),
        (vec!["--config", VALID_CONFIG], Some(""), missing_token),
        (
            vec!["--config", "tests/fixtures/does-not-exist.toml"],
            Some("token"),
            "cannot load the project list: cannot read tests/fixtures/does-not-exist.toml",
        ),
        (
            vec!["--config", "tests/fixtures/invalid_projects.toml"],
            Some("token"),
            "invalid project \"not-a-project\": expected the format owner/repo",
        ),
        (vec!["--tui", "--config", VALID_CONFIG], None, missing_token),
    ]
    .into_iter()
    .for_each(|(args, token, expected)| {
        let output = run(&args, token);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(STARTUP_ERROR),
            "args {args:?}, stderr: {stderr}"
        );
        assert!(
            stderr.contains(expected),
            "args {args:?}: expected {expected:?} in {stderr:?}"
        );
        assert!(output.stdout.is_empty(), "args {args:?}: nothing on stdout");
    });
}
