# CLAUDE.md — pr-checker

This file provides guidance to Claude Code (claude.ai/code) when working on the `pr-checker` project of this workspace.

A Rust CLI that lists the open pull requests of the GitHub repositories in `projects.toml`, as a colored table or, with `--tui`, in an interactive terminal UI. `README.md` covers usage; `IMPLEMENTATION_PLAN.md` holds the requirements (FR1-FR11), module map, HTTP mapping and status.

## Commands

Workspace-wide commands, CI and the shared conventions are in the root `CLAUDE.md`. Run these from the workspace root.

Running it (`GITHUB_TOKEN`, `cargo pr`, `--tui`, `--config`) is in the README.

```bash
cargo build -p pr-checker                                                   # debug → target/debug/pr-checker
```

Test:

```bash
cargo test -p pr-checker                                                    # offline, no token needed
cargo test -p pr-checker --lib view::                                       # unit tests of one module
cargo test -p pr-checker --test github_client_wiremock maps_error_statuses  # one integration test
cargo llvm-cov -p pr-checker                                                # coverage of this project only
```

`tui/mod.rs` (the event loop) and the success path of `main.rs` (it always calls the real API) are untested I/O shells; keep logic out of them so the 90% minimum holds.

## Architecture

The layers and the module table are in `IMPLEMENTATION_PLAN.md`. Beyond them:

- `service::list_open_prs` never fails as a whole: each `ProjectReport` holds its own `Result`.
- Tests assert on `adapter::view::rows`, not on ANSI output. comfy-table drops colors when stdout is not a TTY.
- `main.rs` hardcodes `DEFAULT_BASE_URL`, so tests exercise `GithubClient::new(&server.uri(), ...)` directly.

`tui` follows The Elm Architecture, as ratatui recommends, split like `severance-pay-calculator`:
- `App` keeps the display rows, computed once when the reports load, so moving and drawing don't rebuild them.
- `view::draw` only draws, and builds a fresh `TableState` from the selection, which is plain data.
- `run` is the only code with side effects: it performs each `Effect` (`Fetch`, `Open(url)`, `Quit`) and sends the result back as a `Message`, so tests never touch a terminal or a browser.

## Conventions

The shared rules are in the root `CLAUDE.md`. In this project:

- Dependency injection uses generics with `S: PullRequestSource`.
- `PullRequestDto.state` is deliberately `pub` and unread, which avoids a `dead_code` warning while keeping the API's shape.

### Tests

- Wiremock for `GithubClient`, and an in-memory fake for `PullRequestSource`.
- Shared builders: `view::fixtures` and `tui::app::fixtures` (`#[cfg(test)]`) for unit tests, and `tests/common/mod.rs` for integration tests.
- The full status-to-error table is unit tested on `map_status`. Wiremock tests only check the wiring.
- `tests/cli_exit_codes.rs` runs the compiled binary (`CARGO_BIN_EXE_pr-checker`) for startup errors only, which stop before any HTTP call. Config files used by tests live in `tests/fixtures/`.

## Library versions

- **comfy-table 8** removed `load_preset` and `apply_modifier`. Use `load_style(PRESET.with_rounded_corners())`.
- **reqwest 0.13** puts `RequestBuilder::query` behind an optional feature that isn't enabled. Build query strings with `reqwest::Url` instead.
- **ratatui 0.30**: use `try_init` and `try_restore`; `init` panics.
- **crossterm** is a direct dependency only for `EventStream`: it takes the workspace version and adds the `event-stream` feature. Keep it on ratatui's version (0.29), or two copies get compiled.
- **`serde_json`** is a dev-dependency only. Decode responses with reqwest's `.json()`.
