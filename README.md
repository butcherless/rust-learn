# rust-learn

> Learn and have fun learning a new language to solve problems in a different way

A command-line tool that lists the open pull requests of a set of GitHub repositories as a colored
table.

```
╭────────────────┬───────┬──────────────────────────────┬───────╮
│ Project        ┆ PR #  ┆ Title                        ┆ State │
╞════════════════╪═══════╪══════════════════════════════╪═══════╡
│ rust-lang/rust ┆ 12345 ┆ Fix borrowck ICE in closures ┆ open  │  orange
│ rust-lang/rust ┆ 12350 ┆ Stabilize foo                ┆ draft │  orange
│ tokio-rs/tokio ┆ —     ┆ no open pull requests        ┆ —     │  green
│ acme/missing   ┆ —     ┆ error: repository not found  ┆ —     │  red
╰────────────────┴───────┴──────────────────────────────┴───────╯
```

## Running

Requires [Rust](https://www.rust-lang.org/tools/install). The stable toolchain is selected by
`rust-toolchain.toml`. Run the commands from the project folder.

**1. Set the token.** Export a GitHub token:

```bash
export GITHUB_TOKEN=<your token>
```

If you use the GitHub CLI, you can take it from
[`gh auth token`](https://cli.github.com/manual/gh_auth_token) instead:

```bash
export GITHUB_TOKEN=$(gh auth token)
```

**2. Choose how to see the results.**

| Mode | Command | What you get |
|---|---|---|
| Table (default) | `cargo run` | Prints the colored table once and exits |
| Interactive UI | `cargo run -- --tui` | A full-screen view you can browse, refresh, and open PRs from |

The `--` passes the flags that follow to the program instead of to `cargo`. Flags can be
combined, for example `cargo run -- --tui --config path/to/projects.toml`.

To run the compiled binary directly, without `cargo run`:

```bash
cargo build --release
./target/release/rust-learn          # table
./target/release/rust-learn --tui    # interactive UI
```

### Table mode

Prints the table to stdout, so it also works in scripts and pipes. Colors only appear when stdout
is a terminal.

### Interactive UI mode

`--tui` shows the same rows and colors in a full-screen view. It needs a real terminal: if
output is piped or redirected, it stops with exit code `2`.

| Key | Action |
|---|---|
| `↑` `↓` / `j` `k` | Move the selection (`g` `G` / `Home` `End` jump to the first or last row) |
| `Enter` / `o` | Open the selected pull request in your browser |
| `r` | Fetch the pull requests again |
| `q` / `Esc` / `Ctrl+C` | Quit |

When you quit, the exit code reflects the last fetch, as in the table below.

## Configuration

**Repositories:** `./projects.toml` is read by default. Use `--config` to read another file:

```bash
cargo run -- --config path/to/projects.toml
```

```toml
projects = [
  "rust-lang/rust",
  "tokio-rs/tokio",
]
```

Each entry must be `owner/repo`. An invalid entry stops the program with a message naming it. An
empty list prints an empty table.

**Token:** the `GITHUB_TOKEN` environment variable is required. For private repositories, a
[personal access token](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens)
needs [read access to pull requests](https://docs.github.com/en/rest/authentication/permissions-required-for-fine-grained-personal-access-tokens)
if it is fine-grained, or the `repo` scope if it is classic.

## Behavior

- At most 2 repositories are fetched at the same time, and the table follows the order of the file.
- A failing repository is reported in its own row and doesn't stop the others.
- Colors only appear when stdout is a terminal.

| Exit code | Meaning |
|---|---|
| `0` | Every project succeeded |
| `1` | At least one project failed (the table is still printed) |
| `2` | Startup error (configuration or token), or the terminal UI could not start |

### Limitations

- Only the first 100 open pull requests of each repository are listed.
- A [secondary rate limit](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)
  returned as 403 without `x-ratelimit-remaining: 0` is reported as `unauthorized`.

## Development

```bash
cargo build                                # or: cargo build --release
cargo test                                 # no token or real network calls needed
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

See [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) for requirements and design, and
[CLAUDE.md](CLAUDE.md) for architecture and code rules.
