//! What the report shows, how each value is formatted, and how concepts are grouped. Shared by
//! `--print` and the terminal UI, so both show the same values.

use std::iter::once;

use crate::domain::{Report, group_digits, thousands};
use crate::sequence::separated;

/// The report's title.
pub const TITLE: &str = "SEVERANCE PAY CALCULATOR REPORT";
pub const GOOD_LUCK: &str = "Good luck!";
pub const DISCLAIMER: &str =
    "Disclaimer: Humans and AI can make mistakes. Check your data, please :-)";

/// A value the report shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Concept {
    AnnualPay,
    DaysPerYear,
    DailyPay,
    InitialDate,
    CurrentDate,
    DaysWorked,
    Total,
}

/// Related concepts under a title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Group {
    pub title: &'static str,
    pub concepts: &'static [Concept],
}

/// The report's groups, in order. [`Concept::Total`] follows them.
pub const GROUPS: [Group; 2] = [
    Group {
        title: "SALARY",
        concepts: &[Concept::AnnualPay, Concept::DaysPerYear, Concept::DailyPay],
    },
    Group {
        title: "PERIOD",
        concepts: &[
            Concept::InitialDate,
            Concept::CurrentDate,
            Concept::DaysWorked,
        ],
    },
];

impl Concept {
    /// The concept's name in the grouped table.
    pub fn label(self) -> &'static str {
        match self {
            Self::AnnualPay => "Annual pay",
            Self::DaysPerYear => "Days per year",
            Self::DailyPay => "Severance per day",
            Self::InitialDate => "Initial date",
            Self::CurrentDate => "Current date",
            Self::DaysWorked => "Days worked",
            Self::Total => "TOTAL SEVERANCE PAY",
        }
    }

    /// The concept's name in the Python program's output, with its dot leaders.
    fn python_label(self) -> &'static str {
        match self {
            Self::AnnualPay => "Annual Pay ·············>",
            Self::DaysPerYear => "Type of dismissal  ·····>",
            Self::DailyPay => "Severance per day pay ··>",
            Self::InitialDate => "Initial date  ··········>",
            Self::CurrentDate => "Current date  ··········>",
            Self::DaysWorked => "Days between dates  ····>",
            Self::Total => "Total severance pay·····>",
        }
    }

    /// The concept's value in `report`, formatted as the Python program does.
    pub fn value(self, report: &Report) -> String {
        let input = &report.input;
        match self {
            Self::AnnualPay => thousands(input.annual_pay, 0),
            Self::DaysPerYear => input.dismissal.days_per_year().to_string(),
            Self::DailyPay => thousands(report.daily_pay, 3),
            Self::InitialDate => input.initial_date.to_string(),
            Self::CurrentDate => input.current_date.to_string(),
            Self::DaysWorked => group_digits(&report.days.to_string()),
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

/// The whole report as plain text, laid out like the Python program's output: a leading blank
/// line, then the title, each group, the total and the closing notes, separated by blank lines.
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
                .map(|concept| format!("{} {}", concept.python_label(), concept.value(report)))
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
    use crate::domain::calculate;
    use crate::domain::fixtures::python_input;

    /// The Python program's output from the screenshot, character by character. Shared with
    /// `tests/cli.rs`.
    const PYTHON_OUTPUT: &str = include_str!("../tests/fixtures/python_output.txt");

    #[test]
    fn plain_text_matches_the_python_output() {
        let report = calculate(python_input()).expect("valid input");
        assert_eq!(plain(&report), PYTHON_OUTPUT);
    }

    #[test]
    fn groups_the_python_values() {
        let report = calculate(python_input()).expect("valid input");
        let values: Vec<Vec<String>> = GROUPS
            .iter()
            .map(|group| group.concepts.iter().map(|c| c.value(&report)).collect())
            .collect();
        assert_eq!(
            values,
            [
                vec!["53,000", "33", "13.128"],
                vec!["2021-10-11", "2026-11-07", "1,854"]
            ]
        );
        assert_eq!(Concept::Total.value(&report), "24,340");
    }

    #[test]
    fn labels_horizons() {
        let labels: Vec<_> = (0..3).map(horizon).collect();
        assert_eq!(labels, ["now", "+1y", "+2y"]);
    }
}
