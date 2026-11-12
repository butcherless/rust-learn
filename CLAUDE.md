# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

A Cargo workspace that holds small proof-of-concept Rust projects. Each one is an independent package with its own `main`, tests and docs; they share one `Cargo.lock`, one `target/`, the toolchain and CI.

## Layout

- `apps/<name>/`: one binary package per project, with its own `README.md` and `CLAUDE.md` (architecture, library notes, project-specific rules). Read it before working on that project.
  - `apps/pr-checker`: lists the open pull requests of GitHub repositories, as a table or a TUI.
  - `apps/severance-pay-calculator`: calculates Spanish severance pay, in a TUI or as a printed report.
  - `apps/calendario-laboral` (binary `calendario`): a console work calendar stored in JSON. Its docs and user-facing text are in Spanish (RNF-07).
- `libs/<name>/`: shared libraries, without a `main`. None exist yet. Create one only when two projects really need the same code, keep it pure domain (no I/O), and declare it in `[workspace.dependencies]`.
- Root `Cargo.toml` is a virtual manifest: `members = ["apps/*"]` (add `"libs/*"` with the first library) and `[workspace.package]` (`version`, `edition`, `publish = false`), which members inherit with `field.workspace = true`. Dependencies stay in each member until a second one shares them; then move the version to `[workspace.dependencies]` and use `dep.workspace = true` (today: chrono, clap, crossterm, ratatui, serde, serde_json, thiserror). A member can add features on top with `dep = { workspace = true, features = [...] }`.
- Profiles (`[profile.*]`) only take effect in the root `Cargo.toml`.
- `rust-toolchain.toml` pins Rust 1.99.1, because 1.100.0 breaks the line coverage that CI checks (blank lines and test attributes count as uncovered). Try `cargo +stable llvm-cov -p <package>` on each new stable release and go back to `channel = "stable"` when it matches.
- Every crate has a single version in `Cargo.lock` except three transitive duplicates that come from upstream and can't be unified from here: `base64` 0.22/0.23 (wiremock vs hyper-util), `core-foundation` 0.9/0.10 (system-configuration vs rustls-platform-verifier, macOS only) and `syn` 2/3 (derive_more and strum vs clap_derive). Check with `cargo tree --workspace -d`; `cargo deny check` fails on any other duplicate (`deny.toml` lists the allowed ones).
- `docs/`: workspace documents. `docs/recurring-tasks.md` lists the periodic reviews (code against SOLID, official docs, functional style and maintained libraries; CI efficiency and security). When asked to apply it, present a coded, prioritized plan first and apply only the approved actions, one commit each.
- `.cargo/config.toml` holds one run alias per app (`cargo pr`, `cargo sev`, `cargo cal`).
- `.idea/` is ignored. The README explains how to open the workspace in IntelliJ.

## Commands

Run from the root. `-p` takes the package name (`[package] name`), not the directory. Each project's `CLAUDE.md` lists its own run flags and tests.

Build and run:

```bash
cargo build                                                   # every member, debug → target/debug/<package>
cargo build --release                                         # every member, optimized → target/release/<package>
cargo build -p <package>                                      # one member (add --release for optimized)
cargo run -p <package> -- <args>                              # build and run one app; args after `--` go to the program
cargo <alias> <args>                                          # same through the app's alias in .cargo/config.toml
./target/release/<package> <args>                             # run a release build without cargo
```

Run aliases:

```bash
GITHUB_TOKEN=$(gh auth token) cargo pr [--tui]                # pr-checker with apps/pr-checker/projects.toml
cargo sev [--print]                                           # severance-pay-calculator
cargo cal <subcomando>                                        # calendario-laboral with apps/calendario-laboral/calendario.json
```

Check:

```bash
cargo test --workspace                                        # every member, offline
cargo test -p <package>                                       # one member
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all                                               # format; --check only reports, as CI does
cargo llvm-cov -p <package> --fail-under-lines 90             # what CI checks for each member; --open for HTML
cargo deny check                                              # advisories, licenses, duplicates and sources (deny.toml)
```

All members build into the root `target/`, never into `apps/<name>/target/`.

Binaries run with the current directory as their working directory, so relative paths (config, data files) resolve against wherever `cargo` was started. Tests that need a file use `env!("CARGO_MANIFEST_DIR")`.

CI (`.github/workflows/ci.yml`) runs on pushes to `main` and on PRs, with `--locked`:
- `lint`: `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`, once.
- `deny`: `cargo deny check` with `deny.toml`: RustSec advisories, licenses (permissive ones only), duplicate versions and sources (crates.io only). A new license or duplicate has to be reviewed and added there. The workflow also runs every Monday, so new advisories show up without a push.
- `projects`: lists the members with `cargo metadata`, so a new app joins the matrix without editing the workflow.
- `build-and-test`, one job per member (`fail-fast: false`): `cargo build --release -p <package>`, then `cargo llvm-cov -p <package> --fail-under-lines 90`, which runs its unit and integration tests (not doctests, which need nightly), and the HTML report as the `coverage-html-<package>` artifact.

Workflow rules: actions are pinned to a commit SHA with the version in a comment, the toolchain and tools (`cargo-llvm-cov`, `cargo-deny`) have fixed versions, and matrix values reach scripts through `env`, never `${{ }}` inside `run`. After editing a workflow or `dependabot.yml`, run `zizmor .github/` (`brew install zizmor`); it should report no findings.

The 90% minimum applies to each project on its own, so a new project needs its own tests. Check it locally with `cargo llvm-cov -p <package> --fail-under-lines 90`. Raise the minimum when coverage grows rather than lowering it.

Dependabot (`.github/dependabot.yml`, `directory: "/"`) covers every member through the root `Cargo.lock`. It waits 7 days after a release (`cooldown`) and groups minor and patch bumps into one weekly PR, because separate ones would conflict in the shared lockfile, plus one for GitHub Actions. Major bumps and crossterm minor bumps (it must match ratatui's crossterm) stay manual. For 0.x crates a "minor" bump can break the API, so read the CI result of that PR before merging it.

## Adding a project

1. Put it in `apps/<name>/`, where `<name>` is its package name. Bring its history with `git filter-branch` or `git filter-repo` onto `apps/<name>/` and merge it with `--allow-unrelated-histories`, or copy it if it has none.
2. Delete its `Cargo.lock`, `target/`, `rust-toolchain.toml`, `.gitignore`, `.idea/` and CI files; the root ones apply. Add any file it must ignore (such as real data) to the root `.gitignore`.
3. In its `Cargo.toml`, inherit `version`, `edition` and `publish` with `.workspace = true`. Move each dependency that another member already uses to `[workspace.dependencies]`, at the latest version, and check `cargo tree --workspace -d`.
4. Add a run alias to `.cargo/config.toml`, passing any data file by its path from the root.
5. Give it a `README.md` (usage) and a `CLAUDE.md` (commands, architecture, project rules) that link to the root ones instead of repeating them. Add it to the projects table in the root `README.md` and to the list under Layout above.
6. Run fmt, clippy and `cargo llvm-cov -p <name> --fail-under-lines 90`. CI picks it up by itself.

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
