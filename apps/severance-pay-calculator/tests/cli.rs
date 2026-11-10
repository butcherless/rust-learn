//! Runs the compiled binary. Only with `--print` or `--help`, which never open the terminal UI.

use std::process::{Command, Output};

const PYTHON_OUTPUT: &str = include_str!("fixtures/python_output.txt");
const PYTHON_ARGS: [&str; 9] = [
    "--print",
    "--annual-pay",
    "53000",
    "--dismissal",
    "33",
    "--from",
    "2021-10-11",
    "--to",
    "2026-11-07",
];

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_severance-pay-calculator"))
        .args(args)
        .output()
        .expect("binary runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn print_with_the_python_values_matches_the_python_output() {
    let output = run(&PYTHON_ARGS);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(stdout(&output), PYTHON_OUTPUT);
}

#[test]
fn print_without_flags_uses_the_defaults_and_today() {
    let output = run(&["--print"]);
    assert!(output.status.success(), "{output:?}");
    [
        "Annual Pay ·············> 53,000",
        "Initial date  ··········> 2021-10-11",
    ]
    .into_iter()
    .for_each(|line| assert!(stdout(&output).contains(line), "missing {line:?}"));
}

#[test]
fn print_applies_the_flags() {
    let output = run(&[
        "--print",
        "--annual-pay",
        "36500",
        "--dismissal",
        "objective",
        "--from",
        "2025-01-01",
        "--to",
        "2025-12-31",
    ]);
    assert!(output.status.success(), "{output:?}");
    // 36,500 / 365 × 20 / 365 = 5.479… per day, × 365 days = 2,000.
    [
        "Type of dismissal  ·····> 20",
        "Severance per day pay ··> 5.479",
        "Days between dates  ····> 365",
        "Total severance pay·····> 2,000",
    ]
    .into_iter()
    .for_each(|line| assert!(stdout(&output).contains(line), "missing {line:?}"));
}

#[test]
fn exit_codes() {
    [
        (
            &["--print", "--from", "2026-01-02", "--to", "2026-01-01"][..],
            1,
            "error: current date is before the initial date",
        ),
        (
            &["--print", "--annual-pay", "0"][..],
            1,
            "error: annual pay must be a positive number",
        ),
        (&["--print", "--from", "2021-02-30"][..], 2, "invalid value"),
        (&["--print", "--dismissal", "30"][..], 2, "invalid value"),
    ]
    .into_iter()
    .for_each(|(args, code, message)| {
        let output = run(args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(code), "{args:?}: {stderr}");
        assert!(stderr.contains(message), "{args:?}: {stderr}");
    });
}

#[test]
fn help_lists_the_flags() {
    let help = stdout(&run(&["--help"]));
    ["--print", "--annual-pay", "--dismissal", "--from", "--to"]
        .into_iter()
        .for_each(|flag| assert!(help.contains(flag), "missing {flag}\n{help}"));
}
