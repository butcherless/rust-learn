//! Ejecuta el binario `calendario` sobre un fichero temporal, como lo haría un usuario (§5).

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

/// Un fichero de calendario en el directorio temporal, que se borra al soltarlo.
struct TempFile(PathBuf);

impl TempFile {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("calendario-cli-{}-{name}.json", std::process::id()));
        let _ = fs::remove_file(&path);
        Self(path)
    }

    fn content(&self) -> String {
        fs::read_to_string(&self.0).unwrap_or_default()
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn run(file: &TempFile, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_calendario"))
        .args(args)
        .arg("--file")
        .arg(&file.0)
        .env_remove("CALENDARIO_FILE")
        .env_remove("CLICOLOR_FORCE")
        .output()
        .expect("the binary runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn rf_01_rf_02_rf_06_rf_11_marks_are_saved_and_shown_without_color_when_redirected() {
    let file = TempFile::new("flow");
    let added = run(&file, &["add-year", "2026"]);
    assert!(added.status.success());
    assert_eq!(
        stdout(&added),
        "Año 2026 añadido con un cupo de 25 días de vacaciones.\n"
    );
    let marked = run(&file, &["mark", "vacaciones", "2026-06-05", "2026-06-12"]);
    assert_eq!(
        stdout(&marked),
        "Marca #1 añadida: vacaciones 2026-06-05 → 2026-06-12.\n"
    );
    let month = stdout(&run(&file, &["month", "2026", "6"]));
    assert!(!month.contains('\u{1b}'), "{month}");
    assert!(
        month.starts_with(
            "               Junio 2026\nLu    Ma    Mi    Ju    Vi    Sá    Do\n 1 L   2 L   3 L   4 L   5 V   6 S   7 S\n 8 V   9 V  10 V  11 V  12 V  13 S  14 S\n"
        ),
        "{month}"
    );
    assert!(
        month.ends_with("Vacaciones restantes: 19 / 25\n"),
        "{month}"
    );
    assert!(
        file.content().contains("\"kind\": \"vacaciones\""),
        "{}",
        file.content()
    );
}

#[test]
fn rn_20_a_terminal_with_color_shows_bars_and_redirected_output_letters() {
    let file = TempFile::new("marker");
    run(&file, &["add-year", "2026"]);
    let colored = Command::new(env!("CARGO_BIN_EXE_calendario"))
        .args(["month", "2026", "junio", "--file"])
        .arg(&file.0)
        .env("CLICOLOR_FORCE", "1")
        .env_remove("NO_COLOR")
        .output()
        .expect("the binary runs");
    let colored = stdout(&colored);
    assert!(
        colored.contains(" 1 \u{1b}[34m▌\u{1b}[0m   2 "),
        "{colored}"
    );
    assert!(!colored.contains(" 1 L"), "{colored}");
    let plain = stdout(&run(&file, &["month", "2026", "junio"]));
    assert!(plain.contains(" 1 L   2 L"), "{plain}");
    assert!(!plain.contains('▌'), "{plain}");
}

#[test]
fn rf_03_rf_04_rf_05_list_day_and_unmark() {
    let file = TempFile::new("unmark");
    run(&file, &["add-year", "2026"]);
    run(
        &file,
        &["mark", "festivo", "2026-03-03", "--desc", "Fiesta local"],
    );
    assert_eq!(
        stdout(&run(&file, &["list", "--year", "2026"])),
        "#1   festivo    2026-03-03  Fiesta local\n"
    );
    assert_eq!(
        stdout(&run(&file, &["day", "2026-03-03"])),
        "2026-03-03 (martes) · F Festivo · 0,0 h · marca #1 · Fiesta local\n"
    );
    assert_eq!(
        stdout(&run(&file, &["unmark", "1"])),
        "Marca #1 eliminada: festivo 2026-03-03.\n"
    );
    assert_eq!(stdout(&run(&file, &["list"])), "No hay marcas.\n");
}

#[test]
fn rf_07_rf_08_rf_09_rf_10_configuration_changes_the_views() {
    let file = TempFile::new("config");
    run(&file, &["add-year", "2026"]);
    assert_eq!(
        stdout(&run(&file, &["hours", "laborable", "8"])),
        "Jornada laborable: 8,0 h.\n"
    );
    assert_eq!(
        stdout(&run(&file, &["quota", "2026", "20"])),
        "Cupo de vacaciones de 2026: 20 días.\n"
    );
    let summary = stdout(&run(&file, &["summary", "2026"]));
    // Sin marcas de vacaciones: las vacaciones por defecto toman 20 días de julio, 4 de ellos
    // viernes. Fuera del verano quedan 174 días de 8 h y 43 viernes de 6 h; en verano, 19 días
    // de 8,5 h y 5 viernes de 6 h (RN-18).
    assert!(summary.contains("Días laborables: 241\n"), "{summary}");
    assert!(
        summary.contains("Horas laborables: 1841,5 h\n"),
        "{summary}"
    );
    assert!(
        summary.contains("Vacaciones restantes: 0 / 20\n"),
        "{summary}"
    );
    let year = stdout(&run(&file, &["year", "2026"]));
    // Marco superior, 4 filas de meses de 10 líneas con su rejilla, una en blanco, la nota de
    // vacaciones por defecto, la leyenda y el cupo (RN-21).
    assert_eq!(year.lines().count(), 1 + 4 * 10 + 4, "{year}");
    // RF-14: 20 días de vacaciones por defecto desde el 1 de julio; julio queda con 3 días.
    assert!(
        summary
            .lines()
            .any(|line| line == "  Julio             3      23,0     1003,0"),
        "{summary}"
    );
    assert!(
        summary.contains("  Total           241    1841,5\n"),
        "{summary}"
    );
}

#[test]
fn rn_06_rn_11_duplicates_and_excess_are_reported() {
    let file = TempFile::new("warnings");
    run(&file, &["add-year", "2026"]);
    run(&file, &["mark", "vacaciones", "2026-06-01", "2026-06-30"]);
    let excess = run(&file, &["mark", "vacaciones", "2026-07-01", "2026-07-07"]);
    assert!(excess.status.success());
    assert_eq!(
        stderr(&excess),
        "aviso: las vacaciones de 2026 superan el cupo en 2 días\n"
    );
    let before = file.content();
    let repeated = run(&file, &["mark", "vacaciones", "2026-07-01", "2026-07-07"]);
    assert!(repeated.status.success());
    assert_eq!(
        stdout(&repeated),
        "Ya existe una marca idéntica (#2); no se ha añadido.\n"
    );
    assert_eq!(file.content(), before);
}

#[test]
fn rn_13_invalid_operations_fail_and_leave_the_file_untouched() {
    let file = TempFile::new("errors");
    run(&file, &["add-year", "2026"]);
    let before = file.content();
    [
        (
            &["mark", "festivo", "2026-12-28", "2027-01-03"][..],
            "error: el rango 2026-12-28 → 2027-01-03 cruza de año; márcalo con un rango por año\n",
        ),
        (
            &["mark", "festivo", "2026-03-05", "2026-03-03"][..],
            "error: rango no válido: 2026-03-05 es posterior a 2026-03-03\n",
        ),
        (
            &["day", "2027-01-05"][..],
            "error: el año 2027 no existe en el calendario; añádelo con «calendario add-year 2027»\n",
        ),
        (&["add-year", "2026"][..], "error: el año 2026 ya existe en el calendario\n"),
        (&["unmark", "9"][..], "error: no existe ninguna marca con id 9\n"),
        (
            &["hours", "reducido", "25"][..],
            "error: horas no válidas: 25; deben estar entre 0 y 24\n",
        ),
    ]
    .into_iter()
    .for_each(|(args, message)| {
        let output = run(&file, args);
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        assert_eq!(stderr(&output), message, "{args:?}");
        assert_eq!(file.content(), before, "{args:?}");
    });
}

#[test]
fn rn_01_invalid_arguments_exit_with_2() {
    let file = TempFile::new("arguments");
    let output = run(&file, &["mark", "festivo", "2026-02-30"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("fecha no válida: «2026-02-30»"),
        "{}",
        stderr(&output)
    );
    assert!(!file.0.exists());
}

#[test]
fn a_corrupt_file_is_reported_and_kept() {
    let file = TempFile::new("corrupt");
    fs::write(&file.0, "no es json").expect("write");
    let output = run(&file, &["add-year", "2026"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).starts_with("error: el fichero «"),
        "{}",
        stderr(&output)
    );
    assert_eq!(file.content(), "no es json");
}

#[test]
fn the_file_can_come_from_the_environment() {
    let file = TempFile::new("env");
    let output = Command::new(env!("CARGO_BIN_EXE_calendario"))
        .args(["add-year", "2030"])
        .env("CALENDARIO_FILE", &file.0)
        .output()
        .expect("the binary runs");
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(file.content().contains("\"year\": 2030"));
}

#[test]
fn help_is_in_spanish() {
    let file = TempFile::new("help");
    let help = stdout(&run(&file, &["--help"]));
    assert!(
        help.contains("Gestiona el calendario laboral de uno o varios años"),
        "{help}"
    );
    assert!(help.contains("Comandos:"), "{help}");
    assert!(help.contains("Añade un año al calendario"), "{help}");
    assert!(help.contains("Uso: calendario"), "{help}");
    assert!(help.contains("Opciones:"), "{help}");
    let mark = stdout(&run(&file, &["mark", "--help"]));
    assert!(mark.contains("Argumentos:"), "{mark}");
    assert!(mark.contains("--desc <TEXTO>"), "{mark}");
    assert!(!mark.contains("Options"), "{mark}");
}
