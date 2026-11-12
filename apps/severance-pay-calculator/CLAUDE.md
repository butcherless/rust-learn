# CLAUDE.md — severance-pay-calculator

This file provides guidance to Claude Code (claude.ai/code) when working on the `severance-pay-calculator` project of this workspace.

A Rust terminal UI (ratatui) that calculates Spanish severance pay. With `--print`, it prints the report to stdout instead. It is a port of the Python script `~/projects/calc_amount.py`. Its structure and conventions follow `apps/pr-checker`. `README.md` covers usage: options, keys and exit codes.

## Commands

Workspace-wide commands, CI and the shared conventions are in the root `CLAUDE.md`. Run these from the workspace root.

Build and run:

```bash
cargo build -p severance-pay-calculator                 # debug → target/debug/severance-pay-calculator
cargo build --release -p severance-pay-calculator       # optimized → target/release/severance-pay-calculator
cargo sev                                               # start the interactive UI (needs a real terminal; Esc quits)
cargo sev --print                                       # print the report for the defaults and today, no UI
cargo sev --print --annual-pay 40000 --dismissal 20 --from 2020-01-01 --to 2025-12-31
cargo run --release -p severance-pay-calculator         # optimized run
./target/release/severance-pay-calculator --print       # run a release build without cargo
```

Test:

```bash
cargo test -p severance-pay-calculator                  # unit tests, plus tests/cli.rs, which runs the binary
cargo test -p severance-pay-calculator --lib domain::   # tests of one module
cargo test -p severance-pay-calculator shows_the_python_report_grouped   # one test
cargo llvm-cov -p severance-pay-calculator              # coverage of this project only; --open for HTML
```

Coverage:
- `tui/mod.rs` (0%) and the UI branch of `main.rs` stay untested because they drive the real terminal, as in pr-checker. Keep logic out of them so the 90% minimum holds.
- Every other file is at or near 100%.

## Parity with the Python script

The calculation must give the same output as `calc_amount.py`:
- daily pay = `annual_pay / 365 * days_per_year / 365`
- days = `(current - initial).num_days() + 1`, which counts both dates
- total = days × daily pay

The arithmetic stays in `f64` so that rounding matches Python's floats. `domain::thousands` reproduces Python's `f"{x:,.Nf}"`.

The reference case is the Python screenshot from 2026-11-07: 53,000 · 33 · 13.128 · 2021-10-11 → 2026-11-07 · 1,854 days · 24,340. Tests pin it in `domain`, in `report` (the `--print` text, character by character) and in `tui::view` (the rendered screen). If one of them fails, the calculation changed, not just the presentation.

## Architecture

Dependencies point inward: `main → cli, tui → report → domain`, and all of them may use `sequence`.

- **`sequence`**:
  - `Cycle` is a trait with a `const ALL` and default `next`/`previous` that wrap around. `Dismissal` and `Field` implement it.
  - `separated` chains groups with a separator between them, standing in for the unstable `Iterator::intersperse`.
- **`domain`**:
  - `calculate(Input) -> Result<Report, CalcError>`. `Report` carries its `Input`.
  - `forecast(Input, years)` runs `calculate` on the current date and on the next `years` anniversaries of the initial date after it, with constant salary. Each anniversary is counted from the initial date, so a 29 February start falls on 28 February only in non-leap years.
  - `thousands` and `group_digits` format numbers.
  - `Dismissal` is a closed set of presets (20, 33 and 45 days per year) and implements `Display`.
  - The defaults live here, so the UI and `--print` agree.
  - `fixtures` (`#[cfg(test)]`) has `date` and `python_input`.
- **`report`** is the single source of what is shown.
  - `Concept` gives each value's label and formatted value.
  - `GROUPS` defines SALARY and PERIOD, and `Concept::Total` follows them.
  - `plain` builds the Python layout for `--print` from the same groups.
  - To add a value, add a `Concept` and list it in `GROUPS`. Both outputs and the panel sizes follow.
  - `FORECAST_YEARS` (5) and `horizon` give the forecast's `now`, `+1y`… labels.
- **`tui::app`** is the state, following The Elm Architecture: `message(key)` gives a `Message`, and `App::update(self, Message) -> (App, Effect)` gives the next state.
  - Fields stay raw `String`s and are parsed on every draw, so invalid input shows an error and never blocks typing.
  - Per-field behavior is defined on `Field`: `label` and `text_rules` (max length and accepted characters).
  - `App::value(field)` is what the view reads.
  - `fixtures` has `app`, `apply` and `typed`.
- **`tui::view`** is the drawing. It only reads `App`.
  - Panels sit side by side from 76 columns and stack below that.
  - The Forecast panel sits below them only when the terminal is tall enough: 30 rows side by side. Each bar has a green part for what is earned today (`█`) and a cyan part for the growth (`░`), scaled to the largest total. Today's total is green and bold, like the report's total; the forecast totals are cyan.
  - Panel sizes are computed from `GROUPS`, `Field::ALL`, `FORECAST_YEARS` and the widest dismissal text.
  - Colors: cyan input border, orange group headings, cyan values, green total, red errors, gray help.
- **`tui::mod`** is the only code with side effects: the terminal and the synchronous `crossterm::event::read` loop.
- **`cli`**: clap `Args`. The flags fill in both the UI's starting form and `--print`. `DismissalArg` keeps clap out of the domain and accepts 20, 33 and 45 as aliases.
- **`main.rs`**: the composition root. It reads the clock and passes today in. Exit codes: 1 when the calculation fails, 2 for invalid arguments (from clap).
- **`tests/cli.rs`** runs the binary, only with `--print` and `--help`. `tests/fixtures/python_output.txt` is shared with `report`'s unit test.

## Conventions

The shared rules are in the root `CLAUDE.md`. In this project:

- No `_` wildcard when matching UI enums either.
- Follow the Rust API guidelines: `Display` and `From` over ad hoc methods, and pass small `Copy` types by value.
- Rendering is tested with ratatui's `TestBackend` by asserting on the screen text, not on styles.

## Library versions

- **ratatui 0.30**: use `try_init` and `try_restore`; `init` panics.
- Import crossterm through `ratatui::crossterm`. There is no direct crossterm dependency, which avoids compiling two copies.
- **chrono**: `%d` in `parse_from_str` accepts one-digit days, so `2021-10-1` is a valid date.
