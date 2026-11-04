# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

A Rust CLI that lists the open pull requests of the GitHub repositories in `projects.toml` as a colored table. See `README.md` for usage and limitations, and `IMPLEMENTATION_PLAN.md` for requirements (FR1-FR10), the module map, HTTP mapping and status.

## Commands

```bash
cargo build
cargo test                                          # offline, no token needed
cargo test --test github_client_wiremock maps_404   # one integration test, filtered by name
cargo test --lib render::                           # unit tests of one module
cargo clippy --all-targets -- -D warnings
cargo fmt --check
GITHUB_TOKEN=<token> cargo run -- --config <path>   # real GitHub API; --config defaults to ./projects.toml
```

CI (`.github/workflows/ci.yml`) only runs `cargo build` and `cargo test`, so run clippy and fmt locally. It triggers on pushes to `main` and on PRs (opened, new commits, reopened), with read-only token permissions, a Cargo cache keyed on `Cargo.lock`, and cancellation of superseded runs.

## Architecture

Hexagonal, with dependencies pointing inward: `adapter → service → domain`.

- **`domain`** defines the types and the `PullRequestSource` port. It may import only `std` and `thiserror`.
- **`service::list_open_prs`** is generic over the port and never fails as a whole: each `ProjectReport` holds its own `Result`. It uses `buffered`, not `buffer_unordered`, because output must follow config order (FR5).
- **`adapter`** holds `config` (TOML to `Vec<Project>`), `github` (implements the port) and `cli` (arguments and table).
- **`main.rs`** is the composition root. It maps startup errors to exit code 2 and project failures to exit code 1.

`render::rows` is pure and holds all display logic, so tests assert on it rather than on ANSI output. `render` only applies colors, and comfy-table drops them when stdout is not a TTY.

`main.rs` hardcodes `DEFAULT_BASE_URL`, so the binary can't be pointed at a mock server. Tests exercise `GithubClient` directly through `GithubClient::new(&server.uri(), ...)`.

## Conventions

- Tests must not use the real network or need `GITHUB_TOKEN`: use wiremock for the client and an in-memory fake for the port.
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
