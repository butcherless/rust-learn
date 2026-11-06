# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

A Rust CLI that lists the open pull requests of the GitHub repositories in `projects.toml` as a colored table. See `README.md` for usage and limitations, and `IMPLEMENTATION_PLAN.md` for requirements (FR1-FR10), the module map, HTTP mapping and status.

## Commands

```bash
cargo build
cargo test                                          # offline, no token needed
cargo test --test github_client_wiremock maps_error_statuses   # one integration test, filtered by name
cargo test --lib view::                             # unit tests of one module
cargo clippy --all-targets -- -D warnings
cargo fmt --check
GITHUB_TOKEN=<token> cargo run -- --config <path>   # real GitHub API; --config defaults to ./projects.toml
GITHUB_TOKEN=<token> cargo run -- --tui             # interactive terminal UI (needs a real terminal)
```

CI (`.github/workflows/ci.yml`) runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` (no separate `cargo build`: those two compile every target); `rust-toolchain.toml` installs the clippy and rustfmt components. It triggers on pushes to `main` and on PRs (opened, new commits, reopened), with read-only token permissions, a Cargo cache keyed on `Cargo.lock`, and cancellation of superseded runs.

## Architecture

Hexagonal, with dependencies pointing inward: `adapter → service → domain`.

- **`domain`** defines the types and the `PullRequestSource` port. It may import only `std` and `thiserror`.
- **`service::list_open_prs`** is generic over the port and never fails as a whole: each `ProjectReport` holds its own `Result`. It uses `buffered`, not `buffer_unordered`, because output must follow config order (FR5).
- **`adapter`** holds `config` (TOML to `Vec<Project>`), `github` (implements the port), `view` (presentation model shared by both outputs), `cli` (arguments and table) and `tui` (the `--tui` terminal UI).
- **`main.rs`** is the composition root. It maps startup errors to exit code 2 and project failures to exit code 1.

`view::rows` is pure and holds all display logic (text, `RowKind`, PR URL, `HEADER`, the shared `ORANGE`), so tests assert on it rather than on ANSI output. `cli::render` and `tui::app` only map it to their library's widgets and colors; comfy-table drops colors when stdout is not a TTY. Shared unit-test builders live in `view::fixtures` (`#[cfg(test)]`).

`tui::app` follows The Elm Architecture recommended by ratatui: `message(key)` maps keys to a `Message`, `App::update(self, Message) -> (App, Effect)` consumes the state and returns the next one, and `App::view(&self)` only draws. The selection is plain data; `view` builds a fresh `TableState` from it. `tui::run` is the only place with side effects: it performs each `Effect` (`Fetch`, `Open(url)`, `Quit`) and feeds the result back as a `Message`, so tests never open a browser or a terminal. Rendering is tested with ratatui's `TestBackend`. Use `ratatui::try_init`/`try_restore`, not `init`, which panics. `crossterm` is a direct dependency only for `EventStream` (`event-stream` feature) and must stay on the same version as ratatui's (0.29), or two copies get compiled.

`main.rs` hardcodes `DEFAULT_BASE_URL`, so the binary can't be pointed at a mock server. Tests exercise `GithubClient` directly through `GithubClient::new(&server.uri(), ...)`.

## Conventions

- Tests must not use the real network or need `GITHUB_TOKEN`: use wiremock for the client and an in-memory fake for the port.
- Test builders are shared, not copied: `view::fixtures` for unit tests, `tests/common/mod.rs` (included with `mod common;`) for integration tests. Prefer table-driven tests (an array of cases folded with `for_each` and an `assert_eq!` naming the case) over near-identical test functions. The status-to-error table is unit tested on `map_status`; wiremock tests only check the wiring.
- No `unwrap`, `expect` or `panic!` outside tests, and no `_` wildcard when matching domain enums.
- Dependency injection uses generics with `S: PullRequestSource`, not `dyn` or the `async-trait` crate.
- `anyhow` is allowed only in `main.rs`; library modules use `thiserror` enums.
- `thiserror` messages must not include their source, because `main` prints the full error chain with `{:#}` and the text would appear twice.
- `PullRequestDto.state` is deliberately `pub` and unread, which avoids a `dead_code` warning while keeping the API's shape.

## Library versions

- **comfy-table 8** removed `load_preset` and `apply_modifier`. Use `load_style(PRESET.with_rounded_corners())`.
- **reqwest 0.13** puts `RequestBuilder::query` behind an optional feature that isn't enabled. Build query strings with `reqwest::Url` instead.
- **`serde_json`** is a dev-dependency only. Decode responses with reqwest's `.json()`.

## Git

Work, commit and push directly on `main`. Don't create feature branches unless the user asks for one.
