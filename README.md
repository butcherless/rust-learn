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

- [Rust](https://www.rust-lang.org/tools/install). `rust-toolchain.toml` selects the stable
  toolchain with clippy, rustfmt and `llvm-tools-preview`.
- For coverage only, [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov): install it once
  with `brew install cargo-llvm-cov` or `cargo install cargo-llvm-cov --locked`.

## Build and run

Run every command from this folder. `-p` takes the package name, which is the folder name under
`apps/`.

```bash
cargo build                          # every project → target/debug/
cargo build --release -p <project>   # one project, optimized → target/release/
cargo run -p <project> -- <args>     # build and run one project; the args after `--` go to it
cargo pr | cargo sev | cargo cal     # the same through each project's alias
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
```

CI runs on pushes to `main` and on pull requests:

- **lint**: fmt and clippy for the whole workspace.
- **build-and-test**, one job per project (found with `cargo metadata`, so a new project joins
  without editing the workflow): the release build, then the tests with coverage. It fails if the
  project's line coverage drops below 90%, and attaches the HTML report as the
  `coverage-html-<project>` artifact.

Dependabot opens one weekly pull request with the minor and patch updates of every project, and
another for the GitHub Actions.

## Layout

```
Cargo.toml          workspace manifest: members, shared package fields and dependency versions
.cargo/config.toml  run aliases
apps/<project>/     one binary package per project
libs/<library>/     shared libraries, created when two projects need the same code
```

[CLAUDE.md](CLAUDE.md) holds the workspace conventions, and each project's `CLAUDE.md` its
architecture.
