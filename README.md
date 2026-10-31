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

## Quick start

Requires [Rust](https://www.rust-lang.org/tools/install). The stable toolchain is selected by
`rust-toolchain.toml`.

```bash
export GITHUB_TOKEN=<your token>
cargo run
```

If you use the GitHub CLI, you can get the token from
[`gh auth token`](https://cli.github.com/manual/gh_auth_token) instead:

```bash
GITHUB_TOKEN=$(gh auth token) cargo run
```

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
| `2` | Startup error (configuration or token) |

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

The code follows a hexagonal layout, with dependencies pointing inward:

| Path | Role |
|---|---|
| `src/domain` | Pure types and the `PullRequestSource` port |
| `src/service` | The use case: concurrent fetching and reports |
| `src/adapter/config.rs` | Reads `projects.toml` |
| `src/adapter/github` | GitHub REST client |
| `src/adapter/cli` | Arguments and table rendering |
| `src/main.rs` | Wires everything together |

The client calls [List pull requests](https://docs.github.com/en/rest/pulls/pulls#list-pull-requests)
with [API version](https://docs.github.com/en/rest/about-the-rest-api/api-versions) `2022-11-28`,
which GitHub supports through March 10, 2028.
