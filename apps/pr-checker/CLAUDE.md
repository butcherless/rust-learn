# CLAUDE.md — pr-checker

This file provides guidance to Claude Code (claude.ai/code) when working on the `pr-checker` project of this workspace.

A Rust CLI that lists the open pull requests of the GitHub repositories in `projects.toml`, as a colored table or, with `--tui`, in an interactive terminal UI. `README.md` covers usage; `IMPLEMENTATION_PLAN.md` holds the requirements (FR1-FR11), module map, HTTP mapping and status.

## Commands

Workspace-wide commands, CI and the shared conventions are in the root `CLAUDE.md`. Run these from the workspace root.

Build and run:

```bash
cargo build -p pr-checker                                                   # debug → target/debug/pr-checker
cargo build --release -p pr-checker                                         # optimized → target/release/pr-checker
GITHUB_TOKEN=$(gh auth token) cargo pr                                      # real API with apps/pr-checker/projects.toml
GITHUB_TOKEN=$(gh auth token) cargo pr --tui                                # interactive UI, needs a real terminal
GITHUB_TOKEN=$(gh auth token) cargo run -p pr-checker -- --config <path>    # another project list
GITHUB_TOKEN=$(gh auth token) ./target/release/pr-checker --config apps/pr-checker/projects.toml
```

Test:

```bash
cargo test -p pr-checker                                                    # offline, no token needed
cargo test -p pr-checker --lib view::                                       # unit tests of one module
cargo test -p pr-checker --test github_client_wiremock maps_error_statuses  # one integration test
cargo llvm-cov -p pr-checker                                                # coverage of this project only
```

`--config <path>` defaults to `./projects.toml`, relative to the current directory; the `cargo pr` alias passes `apps/pr-checker/projects.toml` so it works from the root. The alias already sets `--config`, so `cargo pr --config <path>` fails as a repeated flag; use `cargo run -p pr-checker` for another file.

`tui/mod.rs` (the event loop, 0%) and the success path of `main.rs` (it always calls the real API) are untested I/O shells that stay in the coverage report; keep logic out of them so the 90% minimum holds. Line coverage is about 91%, the lowest of the workspace.

## Architecture

Hexagonal, with dependencies pointing inward: `adapter → service → domain`.

- **`domain`**: types and the `PullRequestSource` port. It may import only `std` and `thiserror`.
- **`service::list_open_prs`**: generic over the port, and never fails as a whole: each `ProjectReport` holds its own `Result`. It uses `buffered`, not `buffer_unordered`, so output follows config order (FR5).
- **`adapter`**:
  - `config`: TOML file to `Vec<Project>`.
  - `github`: implements the port.
  - `view`: pure presentation model shared by both outputs. `rows` gives each row's text, `RowKind` and PR URL. Tests assert on it, not on ANSI output.
  - `cli`: arguments, and `render`, which maps rows to a comfy-table. comfy-table drops colors when stdout is not a TTY.
  - `tui`: the `--tui` UI.
- **`main.rs`**: composition root. Startup errors exit with 2, project failures with 1. It hardcodes `DEFAULT_BASE_URL`, so tests exercise `GithubClient::new(&server.uri(), ...)` directly.

`tui::app` follows The Elm Architecture, as ratatui recommends:
- `message(key)` maps a key to a `Message`.
- `App::update(self, Message) -> (App, Effect)` consumes the state and returns the next one.
- `App::view(&self)` only draws. The selection is plain data; `view` builds a fresh `TableState` from it.

`tui::run` is the only code with side effects. It performs each `Effect` (`Fetch`, `Open(url)`, `Quit`) and sends the result back as a `Message`, so tests never touch a terminal or a browser. Rendering is tested with ratatui's `TestBackend`.

## Conventions

The shared rules are in the root `CLAUDE.md`. In this project:

- Dependency injection uses generics with `S: PullRequestSource`.
- `PullRequestDto.state` is deliberately `pub` and unread, which avoids a `dead_code` warning while keeping the API's shape.

### Tests

- Tests must not need `GITHUB_TOKEN`. Use wiremock for `GithubClient` and an in-memory fake for `PullRequestSource`.
- Shared builders: `view::fixtures` (`#[cfg(test)]`) for unit tests, and `tests/common/mod.rs` for integration tests.
- The full status-to-error table is unit tested on `map_status`. Wiremock tests only check the wiring.
- `tests/cli_exit_codes.rs` runs the compiled binary (`CARGO_BIN_EXE_pr-checker`) for startup errors only, which stop before any HTTP call. Config files used by tests live in `tests/fixtures/`.

## Library versions

- **comfy-table 8** removed `load_preset` and `apply_modifier`. Use `load_style(PRESET.with_rounded_corners())`.
- **reqwest 0.13** puts `RequestBuilder::query` behind an optional feature that isn't enabled. Build query strings with `reqwest::Url` instead.
- **ratatui 0.30**: use `try_init` and `try_restore`; `init` panics.
- **crossterm** is a direct dependency only for `EventStream` (the `event-stream` feature). Keep it on ratatui's version (0.29), or two copies get compiled.
- **`serde_json`** is a dev-dependency only. Decode responses with reqwest's `.json()`.
