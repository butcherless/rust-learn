# rust-learn

> Learn and have fun learning a new language to solve problems in a different way

A command-line tool that lists the open pull requests of a set of GitHub repositories, as a colored
table or in an interactive terminal UI.

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

## Quick start

Requires [Rust](https://www.rust-lang.org/tools/install); `rust-toolchain.toml` selects the stable
toolchain. From the project folder:

```bash
export GITHUB_TOKEN=$(gh auth token)   # or: export GITHUB_TOKEN=<your token>
cargo run                              # print the table
cargo run -- --tui                     # open the interactive UI
```

[`gh auth token`](https://cli.github.com/manual/gh_auth_token) prints the GitHub CLI's token. The
`--` passes the flags after it to the program instead of to `cargo`.

To run the compiled binary instead:

```bash
cargo build --release
./target/release/rust-learn            # table
./target/release/rust-learn --tui      # interactive UI
```

## Usage

| Flag | Effect |
|---|---|
| *(none)* | Print the table once and exit. Works in scripts and pipes; colors only appear on a terminal. |
| `--tui` | Show the same rows in a full-screen view. Needs a real terminal. |
| `--config <path>` | Read the repositories from `<path>` instead of `./projects.toml`. |

Flags can be combined, for example `cargo run -- --tui --config path/to/projects.toml`.

### Interactive UI keys

| Key | Action |
|---|---|
| `↑` `↓` / `j` `k` | Move the selection |
| `g` `G` / `Home` `End` | Jump to the first or last row |
| `Enter` / `o` | Open the selected pull request in your browser |
| `r` | Fetch the pull requests again |
| `q` / `Esc` / `Ctrl+C` | Quit |

## Configuration

**Repositories:** a TOML file with one `owner/repo` entry per repository, in the order to show them.

```toml
projects = [
  "rust-lang/rust",
  "tokio-rs/tokio",
]
```

An invalid entry stops the program with a message naming it. An empty list shows an empty table.

**Token:** the `GITHUB_TOKEN` environment variable is required. For private repositories, a
[personal access token](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens)
needs [read access to pull requests](https://docs.github.com/en/rest/authentication/permissions-required-for-fine-grained-personal-access-tokens)
if it is fine-grained, or the `repo` scope if it is classic.

## Behavior

- At most 2 repositories are fetched at the same time, and rows follow the order of the file.
- A failing repository is reported in its own red row and doesn't stop the others.

| Exit code | Meaning |
|---|---|
| `0` | Every repository succeeded |
| `1` | At least one repository failed (the results are still shown) |
| `2` | Startup error: configuration, token, or a terminal UI that could not start |

In the interactive UI, the exit code reflects the last fetch before quitting.

**Limitations:**

- Only the first 100 open pull requests of each repository are listed.
- A [secondary rate limit](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)
  returned as 403 without `x-ratelimit-remaining: 0` is reported as `unauthorized`.

## Development

```bash
cargo test                                 # offline: no token or network needed
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo llvm-cov --open                      # run the tests and open an HTML coverage report
```

Coverage uses [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov). Install it once with
`brew install cargo-llvm-cov` or `cargo install cargo-llvm-cov --locked`; `rust-toolchain.toml`
already installs the `llvm-tools-preview` component it needs.

CI runs fmt, clippy, and the tests with coverage. It fails if line coverage drops below 85%, and
attaches the HTML report to each run as the `coverage-html` artifact. See [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) for requirements
and design, and [CLAUDE.md](CLAUDE.md) for architecture and code rules.
