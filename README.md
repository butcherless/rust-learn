# rust-learn

> Learn and have fun learning a new language to solve problems in a different way

A [Cargo workspace](https://doc.rust-lang.org/cargo/reference/workspaces.html) of small
proof-of-concept Rust projects. Each project is an independent package with its own `main`, tests
and README, and they share one `Cargo.lock`, one `target/` folder, the toolchain and CI.

## Projects

| Project | Description | Run from the root |
|---|---|---|
| [pr-checker](apps/pr-checker) | Lists the open pull requests of a set of GitHub repositories, as a table or an interactive terminal UI | `cargo pr` |
| [severance-pay-calculator](apps/severance-pay-calculator) | Calculates Spanish severance pay, in an interactive terminal UI or as a printed report | `cargo sev` |
| [calendario-laboral](apps/calendario-laboral) | Calendario laboral de consola: festivos, vacaciones, jornadas y horas por año, guardado en JSON | `cargo cal` |

Each project's README explains how to use it.

## Requirements

- [Rust](https://www.rust-lang.org/tools/install). `rust-toolchain.toml` selects the toolchain,
  pinned to 1.99.1 for now (see the file), with clippy, rustfmt and `llvm-tools-preview`.
- For coverage only, [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov): install it once
  with `brew install cargo-llvm-cov` or `cargo install cargo-llvm-cov --locked`.
- For the dependency checks only, [cargo-deny](https://github.com/EmbarkStudios/cargo-deny):
  `brew install cargo-deny` or `cargo install cargo-deny --locked`.

## Build and run

Run every command from this folder. `-p` takes the package name, which is the folder name under
`apps/`.

```bash
cargo build                          # every project → target/debug/
cargo build --release -p <project>   # one project, optimized → target/release/
cargo run -p <project> -- <args>     # build and run one project; the args after `--` go to it
cargo pr                             # the same through each project's alias:
cargo sev                            #   pr, sev and cal
cargo cal
```

The aliases live in [`.cargo/config.toml`](.cargo/config.toml). `cargo pr` and `cargo cal` also pass
the project's data file (`--config` and `--file`), because programs resolve relative paths against
the folder `cargo` was started from.

## Development

```bash
cargo test --workspace                                # every project, offline
cargo test -p <project>                               # one project
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all                                       # --check only reports
cargo llvm-cov -p <project> --open                    # one project's tests with an HTML coverage report
cargo deny check                                      # advisories, licenses, duplicates and sources
```

CI runs on pushes to `main` and on pull requests:

- **lint**: fmt and clippy for the whole workspace.
- **deny**: cargo-deny checks the dependencies for
  security advisories, licenses, duplicate versions and sources, as set in `deny.toml`. It also
  runs every Monday.
- **build-and-test**, one job per project (found with `cargo metadata`, so a new project joins
  without editing the workflow): the release build, then the tests with coverage. It fails if the
  project's line coverage drops below 90%, and attaches the HTML report as the
  `coverage-html-<project>` artifact.

Dependabot opens one weekly pull request with the minor and patch updates of every project, and
another for the GitHub Actions.

## IntelliJ IDEA

With the Rust plugin (or RustRover), use **File → Open** on this folder, the one with the workspace
`Cargo.toml`, not on `apps/<project>`. The **Cargo** tool window then lists every project; after
moving or adding one, use its **Refresh Cargo Projects** button. `.idea/` is not versioned.

- **Run configurations:** a **Cargo** configuration with this folder as the working directory and a
  command such as `run -p calendario-laboral -- --file apps/calendario-laboral/calendario.json month`
  or `run -p pr-checker -- --config apps/pr-checker/projects.toml` (with `GITHUB_TOKEN` in its
  environment variables).
- **Tests:** the gutter icons next to each test or module run it, and **Run with Coverage** shows
  the coverage in the editor.
- **Terminal UIs and colors:** `cargo pr --tui`, `cargo sev` and the windows of `cargo cal`
  (`month`, `year`, `summary`) need a real terminal. Use the built-in **Terminal**, not the Run console.

## Layout

```
Cargo.toml          workspace manifest: members, shared package fields and dependency versions
.cargo/config.toml  run aliases
apps/<project>/     one binary package per project
libs/<library>/     shared libraries, created when two projects need the same code
docs/               workspace documents, such as the recurring review tasks
```

[CLAUDE.md](CLAUDE.md) holds the workspace conventions, and each project's `CLAUDE.md` its
architecture. [docs/recurring-tasks.md](docs/recurring-tasks.md) lists the reviews to repeat
periodically: code and CI.
