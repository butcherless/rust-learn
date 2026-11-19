//! The severance pay calculation under the Estatuto de los Trabajadores (Real Decreto Legislativo
//! 2/2015): art. 53.1.b for objective dismissals and art. 56.1 for unfair ones. Pure functions, no
//! I/O.

use std::fmt;

use chrono::{Datelike, Months, NaiveDate};
use thiserror::Error;

use crate::sequence::Cycle;

/// Days in a year: the daily salary is the annual pay over them.
pub const DAYS_PER_YEAR: f64 = 365.0;

/// Months in a year: periods shorter than a year are prorated by months.
const MONTHS_PER_YEAR: f64 = 12.0;

/// Days of salary in a monthly payment, which the caps are counted in.
const DAYS_PER_MONTHLY_PAYMENT: u32 = 30;

/// The default annual pay.
pub const DEFAULT_ANNUAL_PAY: f64 = 53_000.0;

/// The default initial date. The fallback is unreachable: the date is valid.
pub const DEFAULT_INITIAL_DATE: NaiveDate = match NaiveDate::from_ymd_opt(2021, 10, 11) {
    Some(date) => date,
    None => NaiveDate::MIN,
};

/// The type of dismissal, which sets how many days of salary are paid per year worked and the cap.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Dismissal {
    /// Objective dismissal (art. 53.1.b ET): 20 days per year, up to 12 monthly payments.
    Objective,
    /// Unfair dismissal (art. 56.1 ET): 33 days per year, up to 24 monthly payments.
    #[default]
    Unfair,
}

impl Dismissal {
    /// Days of salary paid per year worked.
    pub fn days_per_year(self) -> u32 {
        match self {
            Self::Objective => 20,
            Self::Unfair => 33,
        }
    }

    /// The most days of salary the severance can reach: 12 monthly payments for an objective
    /// dismissal and 24 for an unfair one, of 30 days each.
    pub fn max_days(self) -> u32 {
        let monthly_payments = match self {
            Self::Objective => 12,
            Self::Unfair => 24,
        };
        monthly_payments * DAYS_PER_MONTHLY_PAYMENT
    }

    /// A short human name.
    pub fn label(self) -> &'static str {
        match self {
            Self::Objective => "objective",
            Self::Unfair => "unfair",
        }
    }
}

impl Cycle for Dismissal {
    const ALL: &'static [Self] = &[Self::Objective, Self::Unfair];
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
    /// The last day worked.
    pub current_date: NaiveDate,
}

impl Input {
    /// The default values, with `today` as the current date.
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
    /// The annual pay over 365 days.
    pub daily_salary: f64,
    /// Months of service, a started month counting as a whole one.
    pub months: u32,
    /// Days of salary the severance is worth, after the cap.
    pub severance_days: f64,
    /// Total severance pay.
    pub total: f64,
}

impl Report {
    /// Whether the severance reached the dismissal's cap.
    pub fn capped(&self) -> bool {
        self.severance_days >= f64::from(self.input.dismissal.max_days())
    }
}

/// Why an input cannot be calculated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CalcError {
    #[error("annual pay must be a positive number")]
    NonPositivePay,
    #[error("current date is before the initial date")]
    DatesReversed,
}

/// Calculates the severance pay: days per year × years of service, prorated by months, up to the
/// dismissal's cap, times the daily salary.
pub fn calculate(input: Input) -> Result<Report, CalcError> {
    if !(input.annual_pay.is_finite() && input.annual_pay > 0.0) {
        return Err(CalcError::NonPositivePay);
    }
    if input.current_date < input.initial_date {
        return Err(CalcError::DatesReversed);
    }
    let daily_salary = input.annual_pay / DAYS_PER_YEAR;
    let months = months_of_service(input.initial_date, input.current_date);
    let severance_days = (f64::from(input.dismissal.days_per_year()) * f64::from(months)
        / MONTHS_PER_YEAR)
        .min(f64::from(input.dismissal.max_days()));
    Ok(Report {
        input,
        daily_salary,
        months,
        severance_days,
        total: severance_days * daily_salary,
    })
}

/// The months of service from `initial` to `last`, both worked, counting the started month as a
/// whole one, as the courts prorate "by months" the periods shorter than a year. Month `k` starts
/// `k` months after `initial`, so the result is how many of those starts fall on or before `last`.
/// `last` must not be before `initial`.
pub fn months_of_service(initial: NaiveDate, last: NaiveDate) -> u32 {
    let calendar_months = 12 * i64::from(last.year() - initial.year()) + i64::from(last.month())
        - i64::from(initial.month());
    // Not negative, because `last` is not before `initial`.
    let calendar_months = u32::try_from(calendar_months).unwrap_or(0);
    initial
        .checked_add_months(Months::new(calendar_months))
        .filter(|start| *start <= last)
        .map_or(calendar_months, |_| calendar_months + 1)
}

/// Formats a number of months as years and months, such as `5y 1m`.
pub fn years_and_months(months: u32) -> String {
    format!("{}y {}m", months / 12, months % 12)
}

/// The reports for the current date and the next `years` anniversaries of the initial date after
/// it, with the same salary and dismissal type. Each anniversary is counted from the initial date,
/// so a 29 February start falls on 28 February in non-leap years and on 29 February again in leap
/// years.
pub fn forecast(input: Input, years: usize) -> Result<Vec<Report>, CalcError> {
    let anniversaries = (1..)
        .map_while(|year| {
            input
                .initial_date
                .checked_add_months(Months::new(12 * year))
        })
        .skip_while(|anniversary| *anniversary <= input.current_date)
        .take(years);
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

/// The reports for the first day on which the total reaches each next multiple of `step`, until
/// `months` months after the current date. The total only grows when a new month of service
/// starts, so only those days are candidates. Empty once the severance is capped.
pub fn milestones(input: Input, step: f64, months: u32) -> Result<Vec<Report>, CalcError> {
    let today = calculate(input)?;
    let level = |report: &Report| milestone(report, step);
    let end = months_later(input.current_date, months);
    (today.months..)
        .map_while(|month| input.initial_date.checked_add_months(Months::new(month)))
        .take_while(|start| end.is_some_and(|end| *start <= end))
        .map(|current_date| {
            calculate(Input {
                current_date,
                ..input
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|reports| {
            reports
                .into_iter()
                .scan(level(&today), |reached, report| {
                    let crossed = level(&report) > *reached;
                    *reached = reached.max(level(&report));
                    Some(crossed.then_some(report))
                })
                .flatten()
                .collect()
        })
}

/// The day `months` months after `date`, the last day [`milestones`] covers.
pub fn months_later(date: NaiveDate, months: u32) -> Option<NaiveDate> {
    date.checked_add_months(Months::new(months))
}

/// The largest multiple of `step` that the report's total reaches.
pub fn milestone(report: &Report, step: f64) -> f64 {
    (report.total / step).floor() * step
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
    // Only ASCII digits, so each byte is a character and every chunk is valid UTF-8.
    let grouped = integer
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(String::from_utf8_lossy)
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

    /// The reference case: the default values on 2026-11-07. Written out instead of using the
    /// defaults, so it also checks them.
    pub fn reference_input() -> Input {
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
    use super::fixtures::{date, reference_input};
    use super::*;

    #[test]
    fn calculates_the_reference_case() {
        let report = calculate(reference_input()).expect("valid input");
        assert_eq!(thousands(report.daily_salary, 2), "145.21");
        assert_eq!(report.months, 61, "5 years and a started month");
        assert_eq!(report.severance_days, 167.75, "33 × 61 / 12");
        assert_eq!(thousands(report.total, 0), "24,358");
        assert!(!report.capped());
        assert_eq!(report.input, reference_input());
    }

    #[test]
    fn defaults_are_the_reference_values() {
        assert_eq!(Input::with_defaults(date(2026, 11, 7)), reference_input());
    }

    #[test]
    fn counts_started_months_as_whole_ones() {
        [
            ("the first day", date(2021, 10, 11), date(2021, 10, 11), 1),
            (
                "a day short of a month",
                date(2021, 10, 11),
                date(2021, 11, 10),
                1,
            ),
            (
                "the first day of the second month",
                date(2021, 10, 11),
                date(2021, 11, 11),
                2,
            ),
            (
                "the day before the anniversary",
                date(2021, 10, 11),
                date(2022, 10, 10),
                12,
            ),
            (
                "the anniversary",
                date(2021, 10, 11),
                date(2022, 10, 11),
                13,
            ),
            (
                "the reference case",
                date(2021, 10, 11),
                date(2026, 11, 7),
                61,
            ),
            (
                "a year from the 1st",
                date(2025, 1, 1),
                date(2025, 12, 31),
                12,
            ),
            (
                "a short month clamps the start",
                date(2025, 1, 31),
                date(2025, 2, 27),
                1,
            ),
            (
                "from the clamped start",
                date(2025, 1, 31),
                date(2025, 2, 28),
                2,
            ),
        ]
        .into_iter()
        .for_each(|(case, initial, last, months)| {
            assert_eq!(months_of_service(initial, last), months, "{case}");
        });
    }

    #[test]
    fn caps_the_severance_days() {
        [
            (Dismissal::Unfair, date(2005, 1, 1), 720.0, true),
            (Dismissal::Objective, date(2005, 1, 1), 360.0, true),
            (Dismissal::Objective, date(2009, 11, 8), 340.0, false),
        ]
        .into_iter()
        .for_each(|(dismissal, initial_date, days, capped)| {
            let report = calculate(Input {
                dismissal,
                initial_date,
                ..reference_input()
            })
            .expect("valid input");
            assert_eq!(
                report.severance_days, days,
                "{dismissal} from {initial_date}"
            );
            assert_eq!(report.capped(), capped, "{dismissal} from {initial_date}");
            assert_eq!(
                report.total,
                days * (53_000.0 / 365.0),
                "{dismissal} from {initial_date}"
            );
        });
    }

    #[test]
    fn formats_years_and_months() {
        [
            (0, "0y 0m"),
            (1, "0y 1m"),
            (12, "1y 0m"),
            (61, "5y 1m"),
            (263, "21y 11m"),
        ]
        .into_iter()
        .for_each(|(months, text)| assert_eq!(years_and_months(months), text, "{months}"));
    }

    #[test]
    fn rejects_invalid_input() {
        let with_pay = |annual_pay| Input {
            annual_pay,
            ..reference_input()
        };
        [
            (with_pay(0.0), CalcError::NonPositivePay),
            (with_pay(-1.0), CalcError::NonPositivePay),
            (with_pay(f64::NAN), CalcError::NonPositivePay),
            (with_pay(f64::INFINITY), CalcError::NonPositivePay),
            (
                Input {
                    current_date: date(2021, 10, 10),
                    ..reference_input()
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
        assert_eq!(group_digits("-1854"), "-1,854");
    }

    #[test]
    fn forecasts_the_next_anniversaries_of_the_initial_date() {
        let reports = forecast(reference_input(), 5).expect("valid input");
        let summary: Vec<_> = reports
            .iter()
            .map(|r| {
                (
                    r.input.current_date.to_string(),
                    r.months,
                    thousands(r.total, 0),
                )
            })
            .collect();
        let expected = [
            ("2026-11-07", 61, "24,358"),
            ("2027-10-11", 73, "29,150"),
            ("2028-10-11", 85, "33,942"),
            ("2029-10-11", 97, "38,734"),
            ("2030-10-11", 109, "43,525"),
            ("2031-10-11", 121, "48,317"),
        ]
        .map(|(date, days, total)| (date.to_owned(), days, total.to_owned()));
        assert_eq!(summary, expected);
    }

    #[test]
    fn forecast_moves_leap_days_and_propagates_errors() {
        let leap = Input {
            initial_date: date(2020, 2, 29),
            current_date: date(2027, 1, 1),
            ..reference_input()
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
            ..reference_input()
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
            ..reference_input()
        };
        assert_eq!(forecast(invalid, 5), Err(CalcError::NonPositivePay));
    }

    #[test]
    fn milestones_are_the_first_days_past_each_thousand() {
        let summary: Vec<_> = milestones(reference_input(), 1_000.0, 12)
            .expect("valid input")
            .iter()
            .map(|r| {
                (
                    r.input.current_date.to_string(),
                    milestone(r, 1_000.0),
                    thousands(r.total, 0),
                )
            })
            .collect();
        // A month of service adds 33 / 12 days, about 399, from the 11th of each month.
        let expected = [
            ("2026-12-11", 25_000.0, "25,157"),
            ("2027-03-11", 26_000.0, "26,355"),
            ("2027-05-11", 27_000.0, "27,153"),
            ("2027-08-11", 28_000.0, "28,351"),
            ("2027-10-11", 29_000.0, "29,150"),
        ]
        .map(|(date, level, total)| (date.to_owned(), level, total.to_owned()));
        assert_eq!(summary, expected);
    }

    #[test]
    fn milestones_stop_at_the_cap_and_propagate_errors() {
        let capped = Input {
            initial_date: date(2000, 1, 1),
            ..reference_input()
        };
        assert_eq!(milestones(capped, 1_000.0, 12), Ok(vec![]));
        let invalid = Input {
            annual_pay: 0.0,
            ..reference_input()
        };
        assert_eq!(
            milestones(invalid, 1_000.0, 12),
            Err(CalcError::NonPositivePay)
        );
    }
}
