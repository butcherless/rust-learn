# rust-learn project

## Goal

> Learn and have fun learning a new language to solve problems in a different way

## What it does

A command-line tool that reads a list of GitHub repositories from a TOML file, retrieves the open
pull requests of each one from the GitHub REST API, and prints them as a colored table.

| Row color | Meaning | Title cell |
|---|---|---|
| Green | The project has no open pull requests | `no open pull requests` |
| Orange | One open pull request (state `open` or `draft`) | The pull request title |
| Red | The project could not be fetched | The error message |

At most 2 repositories are fetched at the same time, and the table follows the order of the file.
A failing repository doesn't stop the others. Only the first 100 pull requests of each repository
are listed.

Colors are only shown when stdout is a terminal.

## Environment installation

https://www.rust-lang.org/tools/install

## Configuration

### `projects.toml`

Each entry must be `owner/repo`. An invalid entry stops the program with a message naming it. An
empty list is valid and prints an empty table.

```toml
projects = [
  "rust-lang/rust",
  "tokio-rs/tokio",
  "seanmonstar/reqwest",
]
```

### `GITHUB_TOKEN`

The tool authenticates with a GitHub token read from the `GITHUB_TOKEN` environment variable, and
won't start without it. To see private repositories, the token needs read access to their pull
requests.

```bash
export GITHUB_TOKEN=<your token>
```

## Compile code

```bash
cargo build
```

## Run code

Reads `./projects.toml` by default:

```bash
cargo run
```

To use another file:

```bash
cargo run -- --config path/to/projects.toml
```

### Exit codes

| Code | Meaning |
|---|---|
| `0` | Every project succeeded |
| `1` | At least one project failed (the table is still printed) |
| `2` | Startup error (configuration or token) |

## Test code

The tests use no real network and don't need `GITHUB_TOKEN`:

```bash
cargo test
```
