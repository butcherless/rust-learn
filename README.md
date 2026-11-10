# rust-learn

> Learn and have fun learning a new language to solve problems in a different way

A [Cargo workspace](https://doc.rust-lang.org/cargo/reference/workspaces.html) of small
proof-of-concept Rust projects. Each project is an independent package with its own `main`, tests
and README, and they share one `Cargo.lock`, one `target/` folder, the toolchain and CI.

## Projects

| Project | Description | Run from the root |
|---|---|---|
| [pr-checker](apps/pr-checker) | Lists the open pull requests of a set of GitHub repositories, as a table or an interactive terminal UI | `cargo pr` |
| [severance-pay-calculator](apps/severance-pay-calculator) | Calculates Spanish severance pay, in an interactive terminal UI or as a printed report (`--print`) | `cargo sev` |
| [calendario-laboral](apps/calendario-laboral) | Calendario laboral de consola: festivos, vacaciones, jornadas y horas por año, guardado en JSON (binario `calendario`) | `cargo cal` |

## Usage

Requires [Rust](https://www.rust-lang.org/tools/install); `rust-toolchain.toml` selects the stable
toolchain. Run every command from this folder; `-p` takes the package name.

```bash
cargo build                                    # build every project
cargo run -p <project> -- <args>               # run one project (or its alias, e.g. cargo pr)
cargo test --workspace                         # test every project, offline
cargo test -p <project>                        # test one project
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo llvm-cov --workspace --open              # tests with an HTML coverage report
```

The run aliases live in [`.cargo/config.toml`](.cargo/config.toml).

## Layout

```
Cargo.toml          workspace manifest: members and shared package fields
.cargo/config.toml  run aliases
apps/<project>/     one binary package per project
libs/<library>/     shared libraries, created when two projects need the same code
```

CI runs fmt, clippy and the tests with coverage for the whole workspace, and fails if line coverage
drops below 90%. See [CLAUDE.md](CLAUDE.md) for the workspace conventions.
