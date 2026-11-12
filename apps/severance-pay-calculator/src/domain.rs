//! The severance pay calculation, ported from `calc_amount.py`. Pure functions, no I/O.

use std::fmt;

use chrono::{Months, NaiveDate};
use thiserror::Error;

use crate::sequence::Cycle;

/// Days in a year, used both to get the daily salary and to prorate the severance.
pub const DAYS_PER_YEAR: f64 = 365.0;

/// The Python program's annual pay.
pub const DEFAULT_ANNUAL_PAY: f64 = 53_000.0;

/// The Python program's initial date. The fallback is unreachable: the date is valid.
pub const DEFAULT_INITIAL_DATE: NaiveDate = match NaiveDate::from_ymd_opt(2021, 10, 11) {
    Some(date) => date,
    None => NaiveDate::MIN,
};

/// The type of dismissal, which sets how many days of salary are paid per year worked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Dismissal {
    /// Objective dismissal: 20 days per year.
    Objective,
    /// Unfair dismissal: 33 days per year.
    #[default]
    Unfair,
    /// Unfair dismissal for service before 12 February 2012: 45 days per year.
    UnfairPre2012,
}

impl Dismissal {
    /// Days of salary paid per year worked.
    pub fn days_per_year(self) -> u32 {
        match self {
            Self::Objective => 20,
            Self::Unfair => 33,
            Self::UnfairPre2012 => 45,
        }
    }

    /// A short human name.
    pub fn label(self) -> &'static str {
        match self {
            Self::Objective => "objective",
            Self::Unfair => "unfair",
            Self::UnfairPre2012 => "unfair, pre-2012",
        }
    }
}

impl Cycle for Dismissal {
    const ALL: &'static [Self] = &[Self::Objective, Self::Unfair, Self::UnfairPre2012];
}

/// Shows the days and the name, such as `33 days · unfair`.
impl fmt::Display for Dismissal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} days · {}", self.days_per_year(), self.label())
    }
}

/// The data the calculation needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Input {
    pub annual_pay: f64,
    pub dismissal: Dismissal,
    pub initial_date: NaiveDate,
    pub current_date: NaiveDate,
}

impl Input {
    /// The Python program's values, with `today` as the current date.
    pub fn with_defaults(today: NaiveDate) -> Self {
        Self {
            annual_pay: DEFAULT_ANNUAL_PAY,
            dismissal: Dismissal::default(),
            initial_date: DEFAULT_INITIAL_DATE,
            current_date: today,
        }
    }
}

/// A calculation's result, together with the input it was calculated from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Report {
    pub input: Input,
    /// Severance earned per calendar day worked.
    pub daily_pay: f64,
    /// Days between both dates, counting both.
    pub days: i64,
    /// Total severance pay.
    pub total: f64,
}

/// Why an input cannot be calculated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CalcError {
    #[error("annual pay must be a positive number")]
    NonPositivePay,
    #[error("current date is before the initial date")]
    DatesReversed,
}

/// Calculates the severance pay exactly as the Python program does, operation by operation, so
/// floating-point rounding matches too.
pub fn calculate(input: Input) -> Result<Report, CalcError> {
    if !(input.annual_pay.is_finite() && input.annual_pay > 0.0) {
        return Err(CalcError::NonPositivePay);
    }
    if input.current_date < input.initial_date {
        return Err(CalcError::DatesReversed);
    }
    let daily_pay = input.annual_pay / DAYS_PER_YEAR * f64::from(input.dismissal.days_per_year())
        / DAYS_PER_YEAR;
    let days = (input.current_date - input.initial_date).num_days() + 1;
    Ok(Report {
        input,
        daily_pay,
        days,
        total: days as f64 * daily_pay,
    })
}

/// The reports for the current date and the next `years` anniversaries of the initial date after
/// it, with the same salary and dismissal type. Each anniversary is counted from the initial date,
/// so a 29 February start falls on 28 February in non-leap years and on 29 February again in leap
/// years.
pub fn forecast(input: Input, years: u32) -> Result<Vec<Report>, CalcError> {
    let anniversaries = (1..)
        .map_while(|year| {
            input
                .initial_date
                .checked_add_months(Months::new(12 * year))
        })
        .skip_while(|anniversary| *anniversary <= input.current_date)
        .take(usize::try_from(years).unwrap_or(usize::MAX));
    std::iter::once(input.current_date)
        .chain(anniversaries)
        .map(|current_date| {
            calculate(Input {
                current_date,
                ..input
            })
        })
        .collect()
}

/// Formats `value` with `decimals` decimals and a comma every three digits, like Python's
/// `f"{value:,.{decimals}f}"`.
pub fn thousands(value: f64, decimals: usize) -> String {
    group_digits(&format!("{value:.decimals$}"))
}

/// Puts a comma every three digits of a formatted number's integer part: `-1234.5` becomes
/// `-1,234.5`.
pub fn group_digits(number: &str) -> String {
    let (sign, unsigned) = number
        .strip_prefix('-')
        .map_or(("", number), |rest| ("-", rest));
    let (integer, fraction) = unsigned.split_at(unsigned.find('.').unwrap_or(unsigned.len()));
    let digits: Vec<char> = integer.chars().collect();
    let grouped = digits
        .rchunks(3)
        .rev()
        .map(String::from_iter)
        .collect::<Vec<_>>()
        .join(",");
    format!("{sign}{grouped}{fraction}")
}

#[cfg(test)]
pub mod fixtures {
    use super::*;

    pub fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid test date")
    }

    /// The input of the Python program on 2026-11-07, as in the screenshot. Written out instead
    /// of using the defaults, so it also checks them.
    pub fn python_input() -> Input {
        Input {
            annual_pay: 53_000.0,
            dismissal: Dismissal::Unfair,
            initial_date: date(2021, 10, 11),
            current_date: date(2026, 11, 7),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{date, python_input};
    use super::*;

    #[test]
    fn matches_the_python_report() {
        let report = calculate(python_input()).expect("valid input");
        assert_eq!(thousands(report.daily_pay, 3), "13.128");
        assert_eq!(report.days, 1_854);
        assert_eq!(thousands(report.total, 0), "24,340");
        assert_eq!(report.input, python_input());
    }

    #[test]
    fn defaults_are_the_python_values() {
        assert_eq!(Input::with_defaults(date(2026, 11, 7)), python_input());
    }

    #[test]
    fn counts_both_dates() {
        let same_day = Input {
            current_date: date(2021, 10, 11),
            ..python_input()
        };
        assert_eq!(calculate(same_day).map(|r| r.days), Ok(1));
    }

    #[test]
    fn scales_with_the_dismissal_type() {
        Dismissal::ALL.iter().for_each(|&dismissal| {
            let report = calculate(Input {
                dismissal,
                ..python_input()
            })
            .expect("valid");
            let expected = 53_000.0 / 365.0 * f64::from(dismissal.days_per_year()) / 365.0;
            assert!((report.daily_pay - expected).abs() < 1e-12, "{dismissal}");
        });
    }

    #[test]
    fn rejects_invalid_input() {
        let with_pay = |annual_pay| Input {
            annual_pay,
            ..python_input()
        };
        [
            (with_pay(0.0), CalcError::NonPositivePay),
            (with_pay(-1.0), CalcError::NonPositivePay),
            (with_pay(f64::NAN), CalcError::NonPositivePay),
            (with_pay(f64::INFINITY), CalcError::NonPositivePay),
            (
                Input {
                    current_date: date(2021, 10, 10),
                    ..python_input()
                },
                CalcError::DatesReversed,
            ),
        ]
        .into_iter()
        .for_each(|(input, error)| assert_eq!(calculate(input), Err(error), "{input:?}"));
    }

    #[test]
    fn displays_dismissals() {
        [
            (Dismissal::Objective, "20 days · objective"),
            (Dismissal::Unfair, "33 days · unfair"),
            (Dismissal::UnfairPre2012, "45 days · unfair, pre-2012"),
        ]
        .into_iter()
        .for_each(|(dismissal, text)| assert_eq!(dismissal.to_string(), text));
    }

    #[test]
    fn groups_thousands() {
        [
            (0.0, 0, "0"),
            (999.0, 0, "999"),
            (53_000.0, 0, "53,000"),
            (1_234_567.891, 2, "1,234,567.89"),
            (-1_234.5, 1, "-1,234.5"),
            (13.128, 3, "13.128"),
        ]
        .into_iter()
        .for_each(|(value, decimals, expected)| {
            assert_eq!(thousands(value, decimals), expected, "{value}");
        });
        assert_eq!(group_digits(&1_854_i64.to_string()), "1,854");
    }

    #[test]
    fn forecasts_the_next_anniversaries_of_the_initial_date() {
        let reports = forecast(python_input(), 5).expect("valid input");
        let summary: Vec<_> = reports
            .iter()
            .map(|r| {
                (
                    r.input.current_date.to_string(),
                    r.days,
                    thousands(r.total, 0),
                )
            })
            .collect();
        let expected = [
            ("2026-11-07", 1_854, "24,340"),
            ("2027-10-11", 2_192, "28,777"),
            ("2028-10-11", 2_558, "33,582"),
            ("2029-10-11", 2_923, "38,374"),
            ("2030-10-11", 3_288, "43,165"),
            ("2031-10-11", 3_653, "47,957"),
        ]
        .map(|(date, days, total)| (date.to_owned(), days, total.to_owned()));
        assert_eq!(summary, expected);
    }

    #[test]
    fn forecast_moves_leap_days_and_propagates_errors() {
        let leap = Input {
            initial_date: date(2020, 2, 29),
            current_date: date(2027, 1, 1),
            ..python_input()
        };
        let dates: Vec<_> = forecast(leap, 2)
            .expect("valid input")
            .iter()
            .map(|r| r.input.current_date)
            .collect();
        assert_eq!(
            dates,
            [date(2027, 1, 1), date(2027, 2, 28), date(2028, 2, 29)]
        );
        let on_an_anniversary = Input {
            current_date: date(2026, 10, 11),
            ..python_input()
        };
        let dates: Vec<_> = forecast(on_an_anniversary, 1)
            .expect("valid input")
            .iter()
            .map(|r| r.input.current_date)
            .collect();
        assert_eq!(
            dates,
            [date(2026, 10, 11), date(2027, 10, 11)],
            "today's anniversary is not repeated"
        );
        let invalid = Input {
            annual_pay: 0.0,
            ..python_input()
        };
        assert_eq!(forecast(invalid, 5), Err(CalcError::NonPositivePay));
    }
}
