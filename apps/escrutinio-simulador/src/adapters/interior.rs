//! Los ficheros de resultados del Ministerio del Interior (requisitos de escrutinio, «Resultados
//! de 2023 por mesa»): texto de ancho fijo en Latin-1, con posiciones desde 1.
//!
//! `read` busca en una carpeta los ficheros `09` (mesas), `10` (votos por mesa), `03`
//! (candidaturas) y `07` (totales oficiales) y los convierte en filas del dominio. Los `parse_*`
//! leen una línea y son puros.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::domain::station::{Candidacy, StationKey, StationRow, VoteRow};

/// Las filas de los cuatro ficheros.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Files {
    pub stations: Vec<StationRow>,
    pub votes: Vec<VoteRow>,
    pub candidacies: Vec<Candidacy>,
    /// Mesas de cada provincia según los totales oficiales.
    pub expected: BTreeMap<String, u32>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("tiene {found} caracteres y se esperaban al menos {expected}")]
    Short { expected: usize, found: usize },
    #[error("«{0}» no es un número")]
    NotANumber(String),
}

#[derive(Debug, Error)]
pub enum ReadError {
    #[error("no se pudo leer la carpeta {}", .0.display())]
    Dir(PathBuf, #[source] io::Error),
    #[error("falta el fichero {prefix}*.DAT en {}", dir.display())]
    Missing { prefix: &'static str, dir: PathBuf },
    #[error("hay más de un fichero {prefix}*.DAT en {}", dir.display())]
    Ambiguous { prefix: &'static str, dir: PathBuf },
    #[error("no se pudo leer {}", .0.display())]
    File(PathBuf, #[source] io::Error),
    #[error("{}, línea {line}", file.display())]
    Parse {
        file: PathBuf,
        line: usize,
        #[source]
        reason: ParseError,
    },
}

/// Lee los cuatro ficheros de `dir`.
pub fn read(dir: &Path) -> Result<Files, ReadError> {
    let names = fs::read_dir(dir)
        .and_then(|entries| {
            entries
                .map(|entry| entry.map(|e| e.file_name().to_string_lossy().into_owned()))
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(|error| ReadError::Dir(dir.to_path_buf(), error))?;
    Ok(Files {
        stations: lines(dir, &names, "09", parse_station)?,
        votes: lines(dir, &names, "10", parse_vote)?,
        candidacies: lines(dir, &names, "03", parse_candidacy)?,
        expected: lines(dir, &names, "07", parse_total)?
            .into_iter()
            .flatten()
            .collect(),
    })
}

/// Lee el único fichero `<prefix>*.DAT` de `dir` y convierte cada línea no vacía con `parse`.
fn lines<T>(
    dir: &Path,
    names: &[String],
    prefix: &'static str,
    parse: fn(&[u8]) -> Result<T, ParseError>,
) -> Result<Vec<T>, ReadError> {
    let matching: Vec<&String> = names
        .iter()
        .filter(|name| name.starts_with(prefix) && name.to_ascii_uppercase().ends_with(".DAT"))
        .collect();
    let path = match matching.as_slice() {
        [name] => dir.join(name),
        [] => {
            return Err(ReadError::Missing {
                prefix,
                dir: dir.to_path_buf(),
            });
        }
        [..] => {
            return Err(ReadError::Ambiguous {
                prefix,
                dir: dir.to_path_buf(),
            });
        }
    };
    let bytes = fs::read(&path).map_err(|error| ReadError::File(path.clone(), error))?;
    bytes
        .split(|&b| b == b'\n')
        .map(|line| line.strip_suffix(b"\r").unwrap_or(line))
        .enumerate()
        .filter(|(_, line)| !line.is_empty())
        .map(|(index, line)| {
            parse(line).map_err(|reason| ReadError::Parse {
                file: path.clone(),
                line: index + 1,
                reason,
            })
        })
        .collect()
}

/// Una línea de `09`: la mesa y sus totales.
pub fn parse_station(line: &[u8]) -> Result<StationRow, ParseError> {
    check_len(line, 86)?;
    Ok(StationRow {
        key: key(line),
        census: number(line, 31, 37)?,
        blank: number(line, 66, 72)?,
        invalid: number(line, 73, 79)?,
        candidacy_votes: number(line, 80, 86)?,
    })
}

/// Una línea de `10`: los votos de una candidatura en una mesa.
pub fn parse_vote(line: &[u8]) -> Result<VoteRow, ParseError> {
    check_len(line, 36)?;
    Ok(VoteRow {
        key: key(line),
        candidacy: number(line, 24, 29)?,
        votes: number(line, 30, 36)?,
    })
}

/// Una línea de `03`: código, siglas y nombre de una candidatura.
pub fn parse_candidacy(line: &[u8]) -> Result<Candidacy, ParseError> {
    check_len(line, 64)?;
    Ok(Candidacy {
        code: number(line, 9, 14)?,
        acronym: text(line, 15, 64),
        name: text(line, 65, 214),
    })
}

/// Una línea de `07`: las mesas de una provincia, o `None` si la línea es de otro ámbito
/// (comunidad, España o distrito electoral).
pub fn parse_total(line: &[u8]) -> Result<Option<(String, u32)>, ParseError> {
    check_len(line, 77)?;
    let community = text(line, 10, 11);
    let province = text(line, 12, 13);
    let district = text(line, 14, 14);
    match (community.as_str(), province.as_str(), district.as_str()) {
        ("99", _, _) | (_, "99", _) => Ok(None),
        (_, _, "9") => Ok(Some((province, number(line, 73, 77)?))),
        _ => Ok(None),
    }
}

fn key(line: &[u8]) -> StationKey {
    StationKey {
        province: text(line, 12, 13),
        municipality: text(line, 14, 16),
        district: text(line, 17, 18),
        section: text(line, 19, 22),
        table: text(line, 23, 23),
    }
}

fn check_len(line: &[u8], expected: usize) -> Result<(), ParseError> {
    if line.len() < expected {
        Err(ParseError::Short {
            expected,
            found: line.len(),
        })
    } else {
        Ok(())
    }
}

/// Las posiciones `from`–`to` (desde 1, ambas incluidas), o lo que haya si la línea es más corta.
fn field(line: &[u8], from: usize, to: usize) -> &[u8] {
    line.get(from - 1..to.min(line.len())).unwrap_or_default()
}

/// Un campo de texto en Latin-1, sin los espacios de relleno: cada byte es un carácter.
fn text(line: &[u8], from: usize, to: usize) -> String {
    field(line, from, to)
        .iter()
        .map(|&b| char::from(b))
        .collect::<String>()
        .trim()
        .to_owned()
}

fn number(line: &[u8], from: usize, to: usize) -> Result<u32, ParseError> {
    let raw = text(line, from, to);
    raw.parse().map_err(|_| ParseError::NotANumber(raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATION: &[u8] =
        b"022023071010400101001 A00004290000429000000000000000000187000024900000010000002000032800000000000000S";
    const VOTE: &[u8] = b"022023071010400101001 A0000020000122";
    const PROVINCE: &[u8] =
        b"02202307118519Ceuta                                             0008311700096000";

    #[test]
    fn parses_a_station() {
        assert_eq!(
            parse_station(STATION),
            Ok(StationRow {
                key: StationKey {
                    province: "04".to_owned(),
                    municipality: "001".to_owned(),
                    district: "01".to_owned(),
                    section: "001".to_owned(),
                    table: "A".to_owned(),
                },
                census: 429,
                blank: 1,
                invalid: 2,
                candidacy_votes: 328,
            })
        );
    }

    #[test]
    fn parses_a_vote() {
        let vote = parse_vote(VOTE);
        assert_eq!(vote.as_ref().map(|v| (v.candidacy, v.votes)), Ok((2, 122)));
        assert_eq!(
            vote.map(|v| v.key.to_string()),
            Ok("04-001-01-001-A".to_owned())
        );
    }

    #[test]
    fn parses_a_candidacy_in_latin_1() {
        let line = [
            b"02202307000002PSOE".as_slice(),
            &[b' '; 46],
            b"PARTIDO SOCIALISTA OBRERO ESPA\xd1OL",
        ]
        .concat();
        assert_eq!(
            parse_candidacy(&line),
            Ok(Candidacy {
                code: 2,
                acronym: "PSOE".to_owned(),
                name: "PARTIDO SOCIALISTA OBRERO ESPAÑOL".to_owned(),
            })
        );
    }

    #[test]
    fn keeps_only_the_provincial_totals() {
        let with = |community: &[u8], province: &[u8], district: u8| {
            let mut line = PROVINCE.to_vec();
            line[9..11].copy_from_slice(community);
            line[11..13].copy_from_slice(province);
            line[13] = district;
            parse_total(&line)
        };
        assert_eq!(parse_total(PROVINCE), Ok(Some(("51".to_owned(), 96))));
        [
            ("national total", with(b"99", b"99", b'9')),
            ("community total", with(b"01", b"99", b'9')),
            ("electoral district", with(b"18", b"51", b'1')),
        ]
        .into_iter()
        .for_each(|(name, parsed)| assert_eq!(parsed, Ok(None), "{name}"));
    }

    #[test]
    fn rejects_short_lines_and_non_numbers() {
        assert_eq!(
            parse_vote(b"0220"),
            Err(ParseError::Short {
                expected: 36,
                found: 4
            })
        );
        let mut line = VOTE.to_vec();
        line[29] = b'x';
        assert_eq!(
            parse_vote(&line),
            Err(ParseError::NotANumber("x000122".to_owned()))
        );
    }
}
