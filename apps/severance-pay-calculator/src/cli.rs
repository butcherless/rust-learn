//! Command-line arguments. They fill in the UI's starting values, or the report printed with
//! `--print`.

use chrono::NaiveDate;
use clap::{Parser, ValueEnum};

use crate::domain::{DEFAULT_ANNUAL_PAY, DEFAULT_INITIAL_DATE, Dismissal, Input};

/// Calculates severance pay. Starts an interactive terminal UI unless `--print` is given.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Args {
    /// Print the report and exit, without the UI.
    #[arg(long)]
    pub print: bool,

    /// Annual gross pay.
    #[arg(long, value_name = "AMOUNT", default_value_t = DEFAULT_ANNUAL_PAY)]
    pub annual_pay: f64,

    /// Type of dismissal: days of salary per year worked. 20, 33 and 45 are accepted too.
    #[arg(long, value_enum, default_value_t = DismissalArg::Unfair)]
    pub dismissal: DismissalArg,

    /// First day of work.
    #[arg(long, value_name = "YYYY-MM-DD", default_value_t = DEFAULT_INITIAL_DATE)]
    pub from: NaiveDate,

    /// Last day counted [default: today].
    #[arg(long, value_name = "YYYY-MM-DD")]
    pub to: Option<NaiveDate>,
}

/// [`Dismissal`] as a command-line value. It lives here so the domain does not depend on clap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DismissalArg {
    /// 20 days per year.
    #[value(alias = "20")]
    Objective,
    /// 33 days per year.
    #[value(alias = "33")]
    Unfair,
    /// 45 days per year, for service before 12 February 2012.
    #[value(name = "unfair-pre2012", alias = "45")]
    UnfairPre2012,
}

impl From<DismissalArg> for Dismissal {
    fn from(arg: DismissalArg) -> Self {
        match arg {
            DismissalArg::Objective => Self::Objective,
            DismissalArg::Unfair => Self::Unfair,
            DismissalArg::UnfairPre2012 => Self::UnfairPre2012,
        }
    }
}

impl Args {
    /// The calculation's input, with `today` as the last day when `--to` is not given.
    pub fn input(&self, today: NaiveDate) -> Input {
        Input {
            annual_pay: self.annual_pay,
            dismissal: self.dismissal.into(),
            initial_date: self.from,
            current_date: self.to.unwrap_or(today),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::{date, python_input};

    fn input(args: &[&str]) -> Result<Input, clap::error::ErrorKind> {
        let today = date(2026, 11, 7);
        Args::try_parse_from(
            std::iter::once("severance-pay-calculator").chain(args.iter().copied()),
        )
        .map(|parsed| parsed.input(today))
        .map_err(|error| error.kind())
    }

    #[test]
    fn parses_flags() {
        let custom = Input {
            annual_pay: 40_000.5,
            dismissal: Dismissal::Objective,
            initial_date: date(2010, 1, 31),
            current_date: date(2020, 2, 29),
        };
        let all = [
            "--annual-pay",
            "40000.5",
            "--dismissal",
            "objective",
            "--from",
            "2010-01-31",
            "--to",
            "2020-02-29",
        ];
        [
            (&[][..], Ok(python_input())),
            (&all[..], Ok(custom)),
            (
                &["--dismissal", "45"][..],
                Ok(Input {
                    dismissal: Dismissal::UnfairPre2012,
                    ..python_input()
                }),
            ),
            (
                &["--dismissal", "unfair-pre2012"][..],
                Ok(Input {
                    dismissal: Dismissal::UnfairPre2012,
                    ..python_input()
                }),
            ),
            (
                &["--dismissal", "20"][..],
                Ok(Input {
                    dismissal: Dismissal::Objective,
                    ..python_input()
                }),
            ),
            (&["--dismissal", "33"][..], Ok(python_input())),
            (
                &["--dismissal", "30"][..],
                Err(clap::error::ErrorKind::InvalidValue),
            ),
            (
                &["--annual-pay", "53,000"][..],
                Err(clap::error::ErrorKind::ValueValidation),
            ),
            (
                &["--from", "2021-13-01"][..],
                Err(clap::error::ErrorKind::ValueValidation),
            ),
            (
                &["--to", "yesterday"][..],
                Err(clap::error::ErrorKind::ValueValidation),
            ),
        ]
        .into_iter()
        .for_each(|(args, expected)| assert_eq!(input(args), expected, "{args:?}"));
    }
}
