//! What the report shows, how each value is formatted, and how concepts are grouped. Shared by
//! `--print` and the terminal UI, so both show the same values.

use std::iter::once;

use crate::domain::{Report, thousands, years_and_months};
use crate::sequence::separated;

/// The report's title.
pub const TITLE: &str = "SEVERANCE PAY CALCULATOR REPORT";
pub const GOOD_LUCK: &str = "Good luck!";
pub const DISCLAIMER: &str =
    "Disclaimer: Humans and AI can make mistakes. Check your data, please :-)";
/// Marks severance days that reached the dismissal's cap.
pub const CAPPED: &str = "max";

/// Width of a label with its dot leaders in the plain report.
const LEADER_WIDTH: usize = 24;

/// A value the report shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Concept {
    AnnualPay,
    DailySalary,
    InitialDate,
    CurrentDate,
    Service,
    DaysPerYear,
    SeveranceDays,
    Total,
}

/// Related concepts under a title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Group {
    pub title: &'static str,
    pub concepts: &'static [Concept],
}

/// The report's groups, in order. [`Concept::Total`] follows them.
pub const GROUPS: [Group; 3] = [
    Group {
        title: "SALARY",
        concepts: &[Concept::AnnualPay, Concept::DailySalary],
    },
    Group {
        title: "PERIOD",
        concepts: &[Concept::InitialDate, Concept::CurrentDate, Concept::Service],
    },
    Group {
        title: "SEVERANCE",
        concepts: &[Concept::DaysPerYear, Concept::SeveranceDays],
    },
];

impl Concept {
    /// The concept's name.
    pub fn label(self) -> &'static str {
        match self {
            Self::AnnualPay => "Annual pay",
            Self::DailySalary => "Daily salary",
            Self::InitialDate => "Initial date",
            Self::CurrentDate => "Current date",
            Self::Service => "Service",
            Self::DaysPerYear => "Days per year",
            Self::SeveranceDays => "Days of salary",
            Self::Total => "TOTAL SEVERANCE PAY",
        }
    }

    /// The label followed by dot leaders up to [`LEADER_WIDTH`], such as `Service ·····>`.
    fn leader(self) -> String {
        let label = self.label();
        let dots = LEADER_WIDTH.saturating_sub(label.chars().count() + 2);
        format!("{label} {}>", "·".repeat(dots))
    }

    /// The concept's value in `report`, formatted.
    pub fn value(self, report: &Report) -> String {
        let input = &report.input;
        match self {
            Self::AnnualPay => thousands(input.annual_pay, 0),
            Self::DailySalary => thousands(report.daily_salary, 2),
            Self::InitialDate => input.initial_date.to_string(),
            Self::CurrentDate => input.current_date.to_string(),
            Self::Service => years_and_months(report.months),
            Self::DaysPerYear => input.dismissal.days_per_year().to_string(),
            Self::SeveranceDays if report.capped() => {
                format!("{} {CAPPED}", thousands(report.severance_days, 2))
            }
            Self::SeveranceDays => thousands(report.severance_days, 2),
            Self::Total => thousands(report.total, 0),
        }
    }
}

/// How many anniversaries the forecast covers.
pub const FORECAST_YEARS: usize = 5;

/// A forecast point's offset from the current date: `now`, `+1y`, `+2y`…
pub fn horizon(years: usize) -> String {
    if years == 0 {
        "now".to_owned()
    } else {
        format!("+{years}y")
    }
}

/// The whole report as plain text: a leading blank line, then the title, each group, the total and
/// the closing notes, separated by blank lines.
pub fn plain(report: &Report) -> String {
    let stars = "*".repeat(TITLE.len());
    let header = vec![stars.clone(), TITLE.to_owned(), stars];
    let concepts = GROUPS
        .iter()
        .map(|group| group.concepts)
        .chain(once(&[Concept::Total][..]))
        .map(|concepts| {
            concepts
                .iter()
                .map(|concept| format!("{} {}", concept.leader(), concept.value(report)))
                .collect()
        });
    let notes = [GOOD_LUCK, DISCLAIMER].map(|note| vec![note.to_owned()]);
    once(String::new())
        .chain(separated(
            once(header).chain(concepts).chain(notes),
            String::new,
        ))
        .map(|line| line + "\n")
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::{date, reference_input};
    use crate::domain::{Input, calculate};

    /// The reference case's report, character by character. Shared with `tests/cli.rs`.
    const REFERENCE_OUTPUT: &str = include_str!("../tests/fixtures/reference_output.txt");

    #[test]
    fn plain_text_matches_the_reference_output() {
        let report = calculate(reference_input()).expect("valid input");
        assert_eq!(plain(&report), REFERENCE_OUTPUT);
    }

    #[test]
    fn groups_the_reference_values() {
        let report = calculate(reference_input()).expect("valid input");
        let values: Vec<Vec<String>> = GROUPS
            .iter()
            .map(|group| group.concepts.iter().map(|c| c.value(&report)).collect())
            .collect();
        assert_eq!(
            values,
            [
                vec!["53,000", "145.21"],
                vec!["2021-10-11", "2026-11-07", "5y 1m"],
                vec!["33", "167.75"],
            ]
        );
        assert_eq!(Concept::Total.value(&report), "24,358");
    }

    #[test]
    fn marks_capped_days() {
        let report = calculate(Input {
            initial_date: date(2000, 1, 1),
            ..reference_input()
        })
        .expect("valid input");
        assert_eq!(CAPPED, "max");
        assert_eq!(
            Concept::SeveranceDays.value(&report),
            format!("720.00 {CAPPED}")
        );
    }

    #[test]
    fn labels_horizons() {
        let labels: Vec<_> = (0..3).map(horizon).collect();
        assert_eq!(labels, ["now", "+1y", "+2y"]);
    }
}
