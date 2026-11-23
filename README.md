# rust-learn

> Learn and have fun learning a new language to solve problems in a different way

A [Cargo workspace](https://doc.rust-lang.org/cargo/reference/workspaces.html) of small
proof-of-concept Rust projects. Each is an independent package with its own `main`, tests and
README; they share one `Cargo.lock`, one `target/`, the toolchain and CI.

## Projects

| Project | Description | Run from the root |
|---|---|---|
| [pr-checker](apps/pr-checker) | Lists the open pull requests of GitHub repositories, as a table or a terminal UI | `cargo pr` |
| [severance-pay-calculator](apps/severance-pay-calculator) | Calculates Spanish severance pay, in a terminal UI or as a printed report | `cargo sev` |
| [calendario-laboral](apps/calendario-laboral) | A console work calendar (holidays, vacation, shifts, hours) stored in JSON, with year and month views | `cargo cal` |
| [escrutinio](apps/escrutinio) | A mock of the Spanish 2026 general election count by province, in a terminal UI | `cargo esc` |
| [escrutinio-simulador](apps/escrutinio-simulador) | Replays the 2023 election count polling station by polling station, publishing each one to Kafka | `cargo esc-sim` |

Each project's README explains its usage.

## Requirements

- Rust, through rustup (below). `rust-toolchain.toml` pins 1.99.1 (the file says why) with clippy,
  rustfmt and `llvm-tools`.
- Coverage only: [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov),
  `brew install cargo-llvm-cov` or `cargo install cargo-llvm-cov --locked`.
- Dependency checks only: [cargo-deny](https://github.com/EmbarkStudios/cargo-deny),
  `brew install cargo-deny` or `cargo install cargo-deny --locked`.

### Installing Rust (macOS and Linux)

The official installer is [rustup](https://rustup.rs). Avoid system or Homebrew `rust` packages:
they ignore `rust-toolchain.toml`.

1. A C linker: `xcode-select --install` on macOS; `sudo apt install build-essential curl` on
   Debian/Ubuntu or `sudo dnf install gcc curl` on Fedora.
2. rustup, with the default options:

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   . "$HOME/.cargo/env"       # PATH for this shell; new shells get it from the profile
   ```

3. From this folder, the pinned toolchain (any `cargo` command also installs it on first use):

   ```bash
   rustup toolchain install   # no arguments: what rust-toolchain.toml asks for
   rustc --version            # rustc 1.99.1
   ```

`rustup self update` updates rustup; `rustup self uninstall` removes it all.

## Build and run

Run every command from this folder. `-p` takes the package name, which is the folder under `apps/`.

```bash
cargo build                          # every project → target/debug/
cargo build --release -p <project>   # one project, optimized → target/release/
cargo run -p <project> -- <args>     # build and run one project; args after `--` go to it
cargo pr                             # the same through each project's alias
cargo sev
cargo cal
```

The aliases are in [`.cargo/config.toml`](.cargo/config.toml). `cargo pr` and `cargo cal` also pass
the data file (`--config`, `--file`), because programs resolve relative paths against the folder
`cargo` runs from.

## Development

```bash
cargo test --workspace                                # every project, offline
cargo test -p <project>                               # one project
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all                                       # --check only reports
cargo llvm-cov -p <project> --open                    # one project's tests, HTML coverage report
cargo deny check                                      # advisories, licenses, duplicates, sources
```

CI runs on pushes to `main` and on pull requests: fmt and clippy, `cargo deny` (also every Monday),
and per project the release build and the tests with coverage, which fail below 90% and attach the
HTML report. Dependabot opens one weekly pull request for minor and patch updates, and one for
GitHub Actions.

## IntelliJ IDEA

With the Rust plugin (or RustRover), **File → Open** this folder (the workspace `Cargo.toml`), not
`apps/<project>`. The **Cargo** tool window lists every project; after moving or adding one, click
**Refresh Cargo Projects**. `.idea/` is not versioned.

- **Run configurations:** a **Cargo** configuration with this folder as working directory and a
  command such as `run -p calendario-laboral -- --file apps/calendario-laboral/calendario.json month`
  or `run -p pr-checker -- --config apps/pr-checker/projects.toml` (with `GITHUB_TOKEN` in its
  environment).
- **Tests:** the gutter icons run a test or module; **Run with Coverage** shows coverage in the
  editor.
- **Terminal UIs:** `cargo pr --tui`, `cargo sev`, `cargo esc` and `cargo cal` `month`, `year` and `summary`
  need a real terminal: use the built-in **Terminal**, not the Run console.

## Layout

```
Cargo.toml          workspace manifest: members, shared package fields, dependency versions
.cargo/config.toml  run aliases
apps/<project>/     one binary package per project
libs/<library>/     shared libraries, once two projects need the same code
docs/               workspace documents, such as the recurring reviews
```

[CLAUDE.md](CLAUDE.md) holds the workspace conventions, and each project's `CLAUDE.md` its
architecture. [docs/recurring-tasks.md](docs/recurring-tasks.md) lists the periodic code and CI
reviews.
