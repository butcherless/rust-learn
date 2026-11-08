//! Cada regla de negocio (RN) y cada requisito Must o Should (RF) de
//! `requisitos-calendario-laboral.md` debe tener al menos un test con su ID en el nombre,
//! como `rn_05_…` o `rf_02_…`. Añadir un requisito sin test hace fallar este fichero.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const REQUIREMENTS: &str = "requisitos-calendario-laboral.md";

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Los IDs de la primera columna de las tablas RN y RF, sin los Could.
fn required_ids(document: &str) -> BTreeSet<String> {
    document
        .lines()
        .filter(|line| line.starts_with("| RN-") || line.starts_with("| RF-"))
        .filter(|line| !line.trim_end().ends_with("| Could |"))
        .filter_map(|line| line.split('|').nth(1))
        .map(|id| id.trim().to_owned())
        .collect()
}

fn rust_files(directory: &Path) -> Vec<PathBuf> {
    fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .flat_map(|path| {
            if path.is_dir() {
                rust_files(&path)
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                vec![path]
            } else {
                Vec::new()
            }
        })
        .collect()
}

/// Los IDs que nombran las funciones con `#[test]`: `rn_09_rn_10_x` cubre RN-09 y RN-10.
fn tested_ids(source: &str) -> BTreeSet<String> {
    let lines: Vec<&str> = source.lines().map(str::trim).collect();
    lines
        .windows(2)
        .filter(|pair| pair[0] == "#[test]")
        .filter_map(|pair| pair[1].strip_prefix("fn "))
        .filter_map(|rest| rest.split('(').next())
        .flat_map(|name| {
            let parts: Vec<&str> = name.split('_').collect();
            parts
                .windows(2)
                .filter(|pair| {
                    matches!(pair[0], "rn" | "rf")
                        && pair[1].len() == 2
                        && pair[1].chars().all(|character| character.is_ascii_digit())
                })
                .map(|pair| format!("{}-{}", pair[0].to_uppercase(), pair[1]))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn every_requirement_has_a_test() {
    let document = fs::read_to_string(manifest_dir().join(REQUIREMENTS)).expect("requirements");
    let required = required_ids(&document);
    let tested: BTreeSet<String> = ["src", "tests"]
        .into_iter()
        .flat_map(|directory| rust_files(&manifest_dir().join(directory)))
        .filter_map(|path| fs::read_to_string(path).ok())
        .flat_map(|source| tested_ids(&source))
        .collect();
    let missing: Vec<&String> = required.difference(&tested).collect();
    assert!(required.len() >= 28, "{required:?}");
    assert!(missing.is_empty(), "requisitos sin test: {missing:?}");
}

#[test]
fn ids_are_read_from_tables_and_test_names() {
    let document =
        "| RN-01 | Fechas |\n| RF-12 | Importar | Could |\n| RF-07 | Año | Should |\ntexto RN-99";
    assert_eq!(
        required_ids(document),
        BTreeSet::from(["RN-01".to_owned(), "RF-07".to_owned()])
    );
    let source = "#[test]\nfn rn_09_rn_10_rf_08_summary() {}\nfn rn_11_not_a_test() {}\n#[test]\nfn rf_1_short() {}";
    assert_eq!(
        tested_ids(source),
        BTreeSet::from(["RN-09".to_owned(), "RN-10".to_owned(), "RF-08".to_owned()])
    );
}
