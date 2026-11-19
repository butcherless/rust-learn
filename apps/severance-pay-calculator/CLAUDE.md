# CLAUDE.md — severance-pay-calculator

This file provides guidance to Claude Code (claude.ai/code) when working on the `severance-pay-calculator` project of this workspace.

A Rust terminal UI (ratatui) that calculates Spanish severance pay under the Estatuto de los Trabajadores. With `--print`, it prints the report to stdout instead. It started as a port of the Python script `~/projects/calc_amount.py`, but now follows the law instead (days per 365, not months, gave a slightly lower total, and there were no caps). Its structure and conventions follow `apps/pr-checker`. `README.md` covers usage: options, calculation (with the legal references), keys and exit codes.

## Commands

Run from the workspace root. The shared commands, CI and conventions are in the root `CLAUDE.md`, and running it in the README.

```bash
cargo build -p severance-pay-calculator                 # debug → target/debug/severance-pay-calculator
```

Test:

```bash
cargo test -p severance-pay-calculator                  # unit tests, plus tests/cli.rs, which runs the binary
cargo test -p severance-pay-calculator --lib domain::   # tests of one module
cargo test -p severance-pay-calculator shows_the_reference_report_grouped   # one test
cargo llvm-cov -p severance-pay-calculator              # coverage of this project only; --open for HTML
```

`tui/mod.rs` and the UI branch of `main.rs` stay untested because they drive the real terminal; keep logic out of them so the 90% minimum holds. Every other file is at or near 100%.

## Reference case

The calculation is in the README, with the articles it follows. If the law changes, update `domain` and the README's table together.

The reference case is the defaults on 2026-11-07: 53,000 · 145.21 per day · 2021-10-11 → 2026-11-07 · 5y 1m (61 months) · 33 × 61 / 12 = 167.75 days · 24,358. Tests pin it in `domain`, in `report` (the `--print` text, character by character, in `tests/fixtures/reference_output.txt`) and in `tui::view` (the rendered screen). If one of them fails, the calculation changed, not just the presentation.

## Architecture

Dependencies point inward: `main → cli, tui → report → domain`, and all of them may use `sequence`.

- **`sequence`**:
  - `Cycle` is a trait with a `const ALL` and default `next`/`previous` that wrap around. `Dismissal` and `Field` implement it.
  - `separated` chains groups with a separator between them, standing in for the unstable `Iterator::intersperse`.
- **`domain`**:
  - `calculate(Input) -> Result<Report, CalcError>`. `Report` carries its `Input`, and `capped()` says whether the days reached the dismissal's cap.
  - `months_of_service` counts the months whose start (`initial + k months`, which chrono clamps to the month's last day) falls on or before the last day, so a started month counts as a whole one. The arithmetic stays in `f64`.
  - `forecast(Input, years)` runs `calculate` on the current date and on the next `years` anniversaries of the initial date after it, with constant salary. Each anniversary is counted from the initial date, so a 29 February start falls on 28 February only in non-leap years.
  - `milestones(Input, step, months)` gives the reports on the first month start (the only days the total grows) that reaches each next multiple of `step`, within `months`. `milestone` is the multiple a report reaches.
  - `thousands`, `group_digits` and `years_and_months` format numbers.
  - `Dismissal` is a closed set of presets (objective and unfair) with their days per year and cap (`max_days`), and implements `Display`.
  - The defaults live here, so the UI and `--print` agree.
  - `fixtures` (`#[cfg(test)]`) has `date` and `reference_input`.
- **`report`** is the single source of what is shown.
  - `Concept` gives each value's label and formatted value.
  - `GROUPS` defines SALARY, PERIOD and SEVERANCE, and `Concept::Total` follows them.
  - `plain` builds the `--print` text from the same groups, with dot leaders generated from each label.
  - To add a value, add a `Concept` and list it in `GROUPS`. Both outputs and the panel sizes follow.
  - `FORECAST_YEARS` (5) and `horizon` give the forecast's `now`, `+1y`… labels. `MILESTONE_STEP` (1,000) and `MILESTONE_MONTHS` (12) set the milestones.
- **`tui::app`** is the state, following The Elm Architecture: `message(key)` gives a `Message`, and `App::update(self, Message) -> (App, Effect)` gives the next state.
  - Fields stay raw `String`s and are parsed on every draw, so invalid input shows an error and never blocks typing.
  - Per-field behavior is defined on `Field`: `label` and `text_rules` (max length and accepted characters).
  - `App::value(field)` is what the view reads.
  - `fixtures` has `app`, `apply` and `typed`.
- **`tui::view`** is the drawing. It only reads `App`.
  - Panels sit side by side from 76 columns, and the Forecast panel below them only from 33 rows; its milestones take 5 more rows (38 in all), whatever their count, and are left out first. The milestone timeline spans the panel's whole width for a year, independent of the bars' scale; `label_columns` drops labels that would overlap, keeping the last one. Each bar has a green part for what is earned today (`█`) and a cyan part for the growth (`░`), scaled to the largest total. Today's total is green and bold, like the report's total; the forecast totals are cyan.
  - Panel sizes are computed from `GROUPS`, `Field::ALL`, `FORECAST_YEARS` and the widest dismissal text.
  - Colors: cyan input border, green report border, orange forecast border, orange group headings, cyan values, green total, red errors, gray help.
- **`tui::mod`** is the only code with side effects: the terminal and the synchronous `crossterm::event::read` loop.
- **`cli`**: clap `Args`. The flags fill in both the UI's starting form and `--print`. `DismissalArg` keeps clap out of the domain and accepts 20 and 33 as aliases.
- **`main.rs`**: the composition root. It reads the clock and passes today in.
- **`tests/cli.rs`** runs the binary, only with `--print` and `--help`. `tests/fixtures/reference_output.txt` is shared with `report`'s unit test.

## Conventions

The shared rules are in the root `CLAUDE.md`. In this project:

- No `_` wildcard when matching UI enums either.
- Follow the Rust API guidelines: `Display` and `From` over ad hoc methods, and pass small `Copy` types by value.
- Rendering is tested with ratatui's `TestBackend` by asserting on the screen text, not on styles.

## Library versions

- **ratatui 0.30**: use `try_init` and `try_restore`; `init` panics.
- Import crossterm through `ratatui::crossterm`. There is no direct crossterm dependency, which avoids compiling two copies.
- **chrono**: `%d` in `parse_from_str` accepts one-digit days, so `2021-10-1` is a valid date.
