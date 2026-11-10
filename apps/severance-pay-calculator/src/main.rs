use std::error::Error;
use std::process::ExitCode;

use clap::Parser;
use severance_pay_calculator::cli::Args;
use severance_pay_calculator::domain::{Input, calculate};
use severance_pay_calculator::{report, tui};

/// Exits with 1 when the calculation fails; clap exits with 2 on invalid arguments.
fn main() -> ExitCode {
    let args = Args::parse();
    let input = args.input(chrono::Local::now().date_naive());
    run(args.print, input).map_or_else(
        |error| {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        },
        |()| ExitCode::SUCCESS,
    )
}

fn run(print: bool, input: Input) -> Result<(), Box<dyn Error>> {
    if print {
        print!("{}", report::plain(&calculate(input)?));
    } else {
        tui::run(input)?;
    }
    Ok(())
}
