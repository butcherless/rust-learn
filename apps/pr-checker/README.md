# pr-checker

A command-line tool that lists the open pull requests of a set of GitHub repositories, as a colored
table or in an interactive terminal UI.

```
╭────────────────┬───────┬──────────────────────────────┬───────┬───────────╮
│ Project        ┆ PR #  ┆ Title                        ┆ State ┆ Mergeable │
╞════════════════╪═══════╪══════════════════════════════╪═══════╪═══════════╡
│ rust-lang/rust ┆ 12345 ┆ Fix borrowck ICE in closures ┆ open  ┆ yes       │  orange
│ rust-lang/rust ┆ 12350 ┆ Stabilize foo                ┆ draft ┆ no        │  orange
│ tokio-rs/tokio ┆ —     ┆ no open pull requests        ┆ —     ┆ —         │  green
│ acme/missing   ┆ —     ┆ error: repository not found  ┆ —     ┆ —         │  red
╰────────────────┴───────┴──────────────────────────────┴───────┴───────────╯
```

## Quick start

From the workspace root (see its [README](../../README.md#requirements) for requirements), the `cargo pr` alias
runs this project with `apps/pr-checker/projects.toml`:

```bash
export GITHUB_TOKEN=$(gh auth token)   # or: export GITHUB_TOKEN=<your token>
cargo pr                               # print the table
cargo pr --tui                         # open the interactive UI
```

[`gh auth token`](https://cli.github.com/manual/gh_auth_token) prints the GitHub CLI's token.

To run the compiled binary instead:

```bash
cargo build --release -p pr-checker
./target/release/pr-checker --config apps/pr-checker/projects.toml          # table
./target/release/pr-checker --config apps/pr-checker/projects.toml --tui    # interactive UI
```

## Usage

| Flag | Effect |
|---|---|
| *(none)* | Print the table once and exit. Works in scripts and pipes; colors only appear on a terminal. |
| `--tui` | Show the same rows in a full-screen view. Needs a real terminal. |
| `--config <path>` | Read the repositories from `<path>` instead of `./projects.toml`. |

Flags can be combined. The alias already passes `--config`, so for another file run
`cargo run -p pr-checker -- --tui --config path/to/projects.toml`; the `--` passes the flags after it
to the program instead of to `cargo`.

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
- Mergeable is `no` when the pull request has conflicts with its base branch. It is `unknown`
  while GitHub is still computing it, usually on the first fetch after a change; fetch again
  (`r` in the interactive UI) to see it.
- Each pull request costs one more API request, to get its Mergeable value.

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

Tests run offline, with no token or network: `cargo test -p pr-checker`. The workspace
[README](../../README.md) has the rest of the commands and CI.
[IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) holds the requirements and design, and
[CLAUDE.md](CLAUDE.md) the architecture and code rules.
