# severance-pay-calculator

A terminal UI that calculates Spanish severance pay from the annual gross pay, the type of
dismissal and the period worked. It shows the salary and period used, the total, and a five-year
forecast. With `--print`, it prints the report to stdout instead.

It is a port of a Python script (`calc_amount.py`) and gives the same figures.

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
| `--dismissal <TYPE>` | `objective` (20 days per year), `unfair` (33) or `unfair-pre2012` (45, for service before 12 February 2012). `20`, `33` and `45` are accepted too | `unfair` |
| `--from <YYYY-MM-DD>` | First day of work | `2021-10-11` |
| `--to <YYYY-MM-DD>` | Last day counted | today |

## Calculation

- daily pay = annual pay / 365 × days per year of the dismissal / 365
- days = days from `--from` to `--to`, both included
- total = days × daily pay

The forecast repeats the calculation on each of the next five anniversaries of the initial date, with the same salary.

## Interactive UI keys

| Key | Action |
|---|---|
| `Tab` / `↓` / `Enter` | Next field |
| `Shift+Tab` / `↑` | Previous field |
| `→` / `Space`, `←` | Next or previous option of the dismissal type |
| `Backspace` / `Delete` | Delete a character / clear the field |
| `Esc` / `Ctrl+C` | Quit |

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
