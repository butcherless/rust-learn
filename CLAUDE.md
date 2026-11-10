# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

A Cargo workspace that holds small proof-of-concept Rust projects. Each one is an independent package with its own `main`, tests and docs; they share one `Cargo.lock`, one `target/`, the toolchain and CI.

## Layout

- `apps/<name>/`: one binary package per project, with its own `README.md` and `CLAUDE.md` (architecture, library notes, project-specific rules). Read it before working on that project.
  - `apps/pr-checker`: lists the open pull requests of GitHub repositories, as a table or a TUI.
  - `apps/severance-pay-calculator`: calculates Spanish severance pay, in a TUI or as a printed report.
- `libs/<name>/`: shared libraries, without a `main`. None exist yet. Create one only when two projects really need the same code, keep it pure domain (no I/O), and declare it in `[workspace.dependencies]`.
- Root `Cargo.toml` is a virtual manifest: `members = ["apps/*"]` (add `"libs/*"` with the first library) and `[workspace.package]` (`version`, `edition`, `publish = false`), which members inherit with `field.workspace = true`. Dependencies stay in each member until a second one shares them; then move the version to `[workspace.dependencies]` and use `dep.workspace = true` (today: clap, ratatui, thiserror). A member can add features on top with `dep = { workspace = true, features = [...] }`.
- Profiles (`[profile.*]`) only take effect in the root `Cargo.toml`.
- `.cargo/config.toml` holds one run alias per app (`cargo pr`, `cargo sev`). Add one for each new app.

## Commands

Run from the root. `-p` takes the package name (`[package] name`), not the directory.

```bash
cargo test --workspace                                        # every member, offline
cargo test -p pr-checker                                      # one member
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo llvm-cov --workspace                                    # tests with a per-file coverage summary; --open for HTML
GITHUB_TOKEN=$(gh auth token) cargo pr                        # run pr-checker with apps/pr-checker/projects.toml
cargo sev --print                                             # run severance-pay-calculator
```

Binaries run with the current directory as their working directory, so relative paths (config, data files) resolve against wherever `cargo` was started. Tests that need a file use `env!("CARGO_MANIFEST_DIR")`.

CI (`.github/workflows/ci.yml`) runs fmt, clippy and `cargo llvm-cov --workspace --fail-under-lines 90` on pushes to `main` and on PRs, and uploads the HTML report as the `coverage-html` artifact. `cargo llvm-cov` replaces `cargo test` there: it runs the same unit and integration tests (but not doctests, which need nightly). There is no separate `cargo build` step, because clippy and the tests compile every target. The minimum applies to the whole workspace, so a new project needs tests that keep it. Raise the minimum when coverage grows rather than lowering it. Dependabot (`directory: "/"`) covers every member.

## Conventions

These apply to every project; each project's `CLAUDE.md` adds its own.

- No `unwrap`, `expect` or `panic!` outside tests, and no `_` wildcard when matching domain enums.
- Prefer a functional style: iterator chains, `Option`/`Result` combinators, and functions that take a value and return a new one instead of mutating shared state.
- Dependency injection uses generics over a port trait, not `dyn` or the `async-trait` crate.
- `anyhow` is allowed only in `main.rs`; library modules use `thiserror` enums.
- `thiserror` messages must not include their source, because `main` prints the full error chain with `{:#}` and the text would appear twice.

### Tests

- Tests must not use the real network or need tokens. Use wiremock for HTTP clients and in-memory fakes for ports.
- Share test builders instead of copying them: a `#[cfg(test)]` fixtures module for unit tests, and `tests/common/mod.rs` (included with `mod common;`) for integration tests.
- Prefer table-driven tests over near-identical test functions: an array of cases, run with `for_each`, and an `assert_eq!` message naming the case.

## Git

Work, commit and push directly on `main`. Don't create feature branches unless the user asks for one.
