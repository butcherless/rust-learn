//! Ejecuta el binario compilado. Solo con `--help`, `--version` o argumentos no válidos, que nunca
//! abren la interfaz de terminal.

use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_escrutinio"))
        .args(args)
        .output()
        .expect("binary runs")
}

#[test]
fn help_lists_the_options() {
    let output = run(&["--help"]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    [
        "--participacion",
        "--intervalo",
        "--semilla",
        "Elecciones generales España 2026",
    ]
    .iter()
    .for_each(|part| assert!(stdout.contains(part), "missing {part:?}\n{stdout}"));
}

#[test]
fn invalid_arguments_exit_with_2() {
    [
        vec!["--participacion", "0"],
        vec!["--participacion", "101"],
        vec!["--intervalo", "10"],
        vec!["--semilla", "x"],
    ]
    .iter()
    .for_each(|args| assert_eq!(run(args).status.code(), Some(2), "{args:?}"));
}
