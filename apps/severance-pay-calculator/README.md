# severance-pay-calculator

A terminal UI that calculates Spanish severance pay from the annual gross pay, the type of
dismissal and the period worked, following the Estatuto de los Trabajadores. It shows the salary,
the period and the days of salary used, the total, and a five-year forecast. With `--print`, it
prints the report to stdout instead.

It is a tool for a quick estimate, not legal advice.

## Quick start

From the workspace root (see its [README](../../README.md#requirements) for requirements):

```bash
cargo sev                       # interactive UI, needs a real terminal
cargo sev --print               # print the report for the defaults and today
cargo sev --print --annual-pay 40000 --dismissal 20 --from 2020-01-01 --to 2025-12-31
```

To run the compiled binary instead:

```bash
cargo build --release -p severance-pay-calculator
./target/release/severance-pay-calculator --print
```

## Options

The options fill in both the UI's starting form and the `--print` report.

| Option | Effect | Default |
|---|---|---|
| `--print` | Print the report and exit, without the UI | |
| `--annual-pay <AMOUNT>` | Annual gross pay | `53000` |
| `--dismissal <TYPE>` | `objective` (20 days per year) or `unfair` (33). `20` and `33` are accepted too | `unfair` |
| `--from <YYYY-MM-DD>` | First day of work | `2021-10-11` |
| `--to <YYYY-MM-DD>` | Last day worked | today |

## Calculation

It follows the [Estatuto de los Trabajadores](https://www.boe.es/buscar/act.php?id=BOE-A-2015-11430)
(Real Decreto Legislativo 2/2015), as in force in November 2026:

| Dismissal | Days of salary per year worked | Cap | Article |
|---|---|---|---|
| Objective | 20 | 12 monthly payments (360 days) | 53.1.b |
| Unfair | 33 | 24 monthly payments (720 days) | 56.1 |

- daily salary = annual gross pay / 365. The annual pay should include the prorated extra
  payments and the salary supplements.
- service = months from `--from` to `--to`, both worked. Periods shorter than a year are prorated
  by months, and a started month counts as a whole one, as the courts apply it: from 2021-10-11
  to 2026-11-07 is 5 years and 1 month.
- days of salary = days per year × service in months / 12, up to the cap. The report marks a
  capped value with `max`.
- total = days of salary × daily salary.

The forecast repeats the calculation on each of the next five anniversaries of the initial date,
with the same salary. On an anniversary a new month has started, so it counts too.

Not covered: the 45 days per year for service before 12 February 2012 (transitional provision 11
ET), and the taxation of the severance.

## Interactive UI keys

| Key | Action |
|---|---|
| `Tab` / `↓` / `Enter` | Next field |
| `Shift+Tab` / `↑` | Previous field |
| `→` / `Space`, `←` | Next or previous option of the dismissal type |
| `Backspace` / `Delete` | Delete a character / clear the field |
| `Esc` / `q` / `Ctrl+C` | Quit |

Invalid input shows an error in place of the report and never blocks typing. The panels sit side
by side from 76 columns, and the forecast appears when the terminal is tall enough.

| Exit code | Meaning |
|---|---|
| `0` | Success |
| `1` | The calculation failed, for example because `--to` is before `--from` |
| `2` | Invalid arguments |

## Development

`cargo test -p severance-pay-calculator`. The workspace [README](../../README.md) has the rest of
the commands and CI, and [CLAUDE.md](CLAUDE.md) the architecture and code rules.
