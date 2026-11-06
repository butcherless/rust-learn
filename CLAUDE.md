# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

A Rust CLI that lists the open pull requests of the GitHub repositories in `projects.toml`, as a colored table or, with `--tui`, in an interactive terminal UI. `README.md` covers usage; `IMPLEMENTATION_PLAN.md` holds the requirements (FR1-FR11), module map, HTTP mapping and status.

## Commands

```bash
cargo test                                                    # offline, no token needed
cargo test --lib view::                                       # unit tests of one module
cargo test --test github_client_wiremock maps_error_statuses  # one integration test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo llvm-cov                                                # tests with a per-file coverage summary; --open for HTML
GITHUB_TOKEN=$(gh auth token) cargo run                       # real API; --config <path> defaults to ./projects.toml
GITHUB_TOKEN=$(gh auth token) cargo run -- --tui              # interactive UI, needs a real terminal
```

CI (`.github/workflows/ci.yml`) runs fmt, clippy and `cargo llvm-cov --fail-under-lines 85` on pushes to `main` and on PRs, and uploads the HTML report as the `coverage-html` artifact. `cargo llvm-cov` replaces `cargo test` there: it runs the same unit and integration tests (but not doctests, which need nightly; there are none today). There is no separate `cargo build` step, because clippy and the tests compile every target. `main.rs` and `tui/mod.rs` are I/O shells with no tests and stay in the report at 0%; keep logic out of them so the minimum holds. Raise the minimum when coverage grows rather than lowering it.

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

- No `unwrap`, `expect` or `panic!` outside tests, and no `_` wildcard when matching domain enums.
- Prefer a functional style: iterator chains, `Option`/`Result` combinators, and functions that take a value and return a new one instead of mutating shared state.
- Dependency injection uses generics with `S: PullRequestSource`, not `dyn` or the `async-trait` crate.
- `anyhow` is allowed only in `main.rs`; library modules use `thiserror` enums.
- `thiserror` messages must not include their source, because `main` prints the full error chain with `{:#}` and the text would appear twice.
- `PullRequestDto.state` is deliberately `pub` and unread, which avoids a `dead_code` warning while keeping the API's shape.

### Tests

- Tests must not use the real network or need `GITHUB_TOKEN`. Use wiremock for the client and an in-memory fake for the port.
- Share test builders instead of copying them: `view::fixtures` (`#[cfg(test)]`) for unit tests, and `tests/common/mod.rs` (included with `mod common;`) for integration tests.
- Prefer table-driven tests over near-identical test functions: an array of cases, run with `for_each`, and an `assert_eq!` message naming the case.
- The full status-to-error table is unit tested on `map_status`. Wiremock tests only check the wiring.

## Library versions

- **comfy-table 8** removed `load_preset` and `apply_modifier`. Use `load_style(PRESET.with_rounded_corners())`.
- **reqwest 0.13** puts `RequestBuilder::query` behind an optional feature that isn't enabled. Build query strings with `reqwest::Url` instead.
- **ratatui 0.30**: use `try_init` and `try_restore`; `init` panics.
- **crossterm** is a direct dependency only for `EventStream` (the `event-stream` feature). Keep it on ratatui's version (0.29), or two copies get compiled.
- **`serde_json`** is a dev-dependency only. Decode responses with reqwest's `.json()`.

## Git

Work, commit and push directly on `main`. Don't create feature branches unless the user asks for one.
