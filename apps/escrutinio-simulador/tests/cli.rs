//! Ejecuta el binario compilado sobre `tests/data`: las mesas de Ceuta y Melilla de los ficheros
//! reales de Interior, más sus filas de CERA y el total de España de `07`. Nunca publica en Kafka:
//! usa `--solo-comprobar` o argumentos que fallan antes de conectar.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_escrutinio-simulador"))
        .args(args)
        .output()
        .expect("binary runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Una copia de `tests/data` en una carpeta temporal, con `change` aplicado a sus ficheros.
fn broken_copy(name: &str, change: impl Fn(&str, Vec<u8>) -> Option<Vec<u8>>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "escrutinio-simulador-{}-{name}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("temp dir");
    fs::read_dir(DATA).expect("data dir").for_each(|entry| {
        let entry = entry.expect("entry");
        let file = entry.file_name().to_string_lossy().into_owned();
        let bytes = fs::read(entry.path()).expect("data file");
        if let Some(bytes) = change(&file, bytes) {
            fs::write(dir.join(&file), bytes).expect("copy");
        }
    });
    dir
}

fn check(dir: &Path) -> Output {
    run(&["--datos", &dir.to_string_lossy(), "--solo-comprobar"])
}

#[test]
fn help_lists_the_options() {
    let output = run(&["--help"]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    [
        "--datos",
        "--kafka",
        "--topic",
        "--topic-dlq",
        "--duracion",
        "--semilla",
        "--solo-comprobar",
        "2023",
    ]
    .iter()
    .for_each(|part| assert!(stdout.contains(part), "missing {part:?}\n{stdout}"));
}

#[test]
fn invalid_arguments_exit_with_2() {
    [
        vec![],
        vec!["--datos", DATA, "--duracion", "0s"],
        vec!["--datos", DATA, "--duracion", "2d"],
        vec!["--datos", DATA, "--semilla", "x"],
    ]
    .iter()
    .for_each(|args| assert_eq!(run(args).status.code(), Some(2), "{args:?}"));
}

#[test]
fn checks_the_real_extract_of_ceuta_and_melilla() {
    let output = check(Path::new(DATA));
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "Comprobadas 183 mesas en 2 circunscripciones: 115.485 de censo y 61.057 votantes (52,87 %).\n"
    );
}

#[test]
fn fails_with_1_and_explains_why_when_the_data_is_wrong() {
    let drop_line = |prefix: &'static str| {
        move |file: &str, bytes: Vec<u8>| {
            Some(if file.starts_with(prefix) {
                let text = String::from_utf8_lossy(&bytes).into_owned();
                text.lines()
                    .skip(1)
                    .map(|l| format!("{l}\n"))
                    .collect::<String>()
                    .into_bytes()
            } else {
                bytes
            })
        }
    };
    let without = |prefix: &'static str| {
        move |file: &str, bytes| (!file.starts_with(prefix)).then_some(bytes)
    };
    let cases: Vec<(&str, PathBuf, &str)> = vec![
        (
            "missing folder",
            PathBuf::from("/no/existe"),
            "no se pudo leer la carpeta /no/existe",
        ),
        (
            "missing file",
            broken_copy("missing", without("10")),
            "falta el fichero 10*.DAT",
        ),
        (
            "one station less",
            broken_copy("count", drop_line("09")),
            "hay votos de la mesa 51-001-01-001-A, que no existe",
        ),
        (
            "votes that do not add up",
            broken_copy("votes", |file, mut bytes| {
                if file.starts_with("10") {
                    bytes[29..36].copy_from_slice(b"9999999");
                }
                Some(bytes)
            }),
            "los votos por candidatura de la mesa 51-001-01-001-A suman",
        ),
        (
            "short line",
            broken_copy("short", |file, bytes| {
                Some(if file.starts_with("07") {
                    b"0220\n".to_vec()
                } else {
                    bytes
                })
            }),
            "07022307.DAT, línea 1: tiene 4 caracteres y se esperaban al menos 77",
        ),
    ];
    cases.into_iter().for_each(|(name, dir, message)| {
        let output = check(&dir);
        assert_eq!(output.status.code(), Some(1), "{name}");
        assert!(
            stderr(&output).starts_with("error: "),
            "{name}: {}",
            stderr(&output)
        );
        assert!(
            stderr(&output).contains(message),
            "{name}: {}",
            stderr(&output)
        );
        if dir.starts_with(std::env::temp_dir()) {
            fs::remove_dir_all(&dir).expect("clean up");
        }
    });
}
