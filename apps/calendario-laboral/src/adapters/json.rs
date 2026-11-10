//! Adaptador de salida: el `Calendar` como fichero JSON (RF-11, §6).
//!
//! Solo se guardan la configuración y los `Mark`; fines de semana, días por defecto y verano
//! se calculan al resolver. [`decode`] y [`encode`] son puras; [`load`] y [`save`] hacen la E/S.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::Weekday;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{
    Calendar, DEFAULT_VACATION_QUOTA, DateRange, DomainError, Hours, Mark, MarkId, MarkKind, Shift,
    WEEK, parse_date,
};

/// La versión del formato que este programa lee y escribe.
pub const VERSION: u32 = 1;

/// El texto del fichero no es un calendario válido.
#[derive(Debug, Error)]
pub enum DecodeError {
    /// El texto no es JSON con los campos esperados.
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    /// El fichero lo escribió otra versión.
    #[error("versión {0} no soportada; se esperaba la {VERSION}")]
    Version(u32),
    /// Un día de `weekend` o de `reduced_days` no es un nombre de día en inglés.
    #[error("día de la semana no válido: «{0}»; usa monday, tuesday… sunday")]
    Weekday(String),
    /// El contenido incumple una regla del dominio.
    #[error(transparent)]
    Domain(#[from] DomainError),
}

/// No se ha podido leer o escribir el fichero del calendario.
#[derive(Debug, Error)]
pub enum StoreError {
    /// Ha fallado la lectura.
    #[error("no se puede leer «{}»: {source}", path.display())]
    Read {
        /// El fichero.
        path: PathBuf,
        /// La causa.
        source: io::Error,
    },
    /// El contenido no es un calendario válido.
    #[error("el fichero «{}» no es válido: {source}", path.display())]
    Decode {
        /// El fichero.
        path: PathBuf,
        /// La causa.
        source: DecodeError,
    },
    /// Ha fallado la escritura.
    #[error("no se puede escribir «{}»: {source}", path.display())]
    Write {
        /// El fichero.
        path: PathBuf,
        /// La causa.
        source: io::Error,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct FileDto {
    version: u32,
    #[serde(default = "default_weekend")]
    weekend: Vec<String>,
    #[serde(default = "default_reduced_days")]
    reduced_days: Vec<String>,
    #[serde(default)]
    hours: HoursDto,
    #[serde(default)]
    years: Vec<YearDto>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HoursDto {
    laborable: f64,
    reducido: f64,
    laborable_verano: f64,
    reducido_verano: f64,
}

impl Default for HoursDto {
    fn default() -> Self {
        Self::from(Hours::default())
    }
}

impl From<Hours> for HoursDto {
    fn from(hours: Hours) -> Self {
        Self {
            laborable: hours.of(Shift::Working),
            reducido: hours.of(Shift::Reduced),
            laborable_verano: hours.of(Shift::SummerWorking),
            reducido_verano: hours.of(Shift::SummerReduced),
        }
    }
}

impl HoursDto {
    const fn of(&self, shift: Shift) -> f64 {
        match shift {
            Shift::Working => self.laborable,
            Shift::Reduced => self.reducido,
            Shift::SummerWorking => self.laborable_verano,
            Shift::SummerReduced => self.reducido_verano,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct YearDto {
    year: i32,
    #[serde(default = "default_quota")]
    vacation_quota: u32,
    #[serde(default)]
    marks: Vec<MarkDto>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MarkDto {
    id: u32,
    kind: KindDto,
    from: String,
    to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum KindDto {
    Reducido,
    Festivo,
    Vacaciones,
}

impl From<KindDto> for MarkKind {
    fn from(kind: KindDto) -> Self {
        match kind {
            KindDto::Reducido => Self::Reduced,
            KindDto::Festivo => Self::Holiday,
            KindDto::Vacaciones => Self::Vacation,
        }
    }
}

impl From<MarkKind> for KindDto {
    fn from(kind: MarkKind) -> Self {
        match kind {
            MarkKind::Reduced => Self::Reducido,
            MarkKind::Holiday => Self::Festivo,
            MarkKind::Vacation => Self::Vacaciones,
        }
    }
}

fn weekday_names(days: &[Weekday]) -> Vec<String> {
    days.iter()
        .map(|day| weekday_name(*day).to_owned())
        .collect()
}

fn default_weekend() -> Vec<String> {
    weekday_names(Calendar::default().weekend())
}

fn default_reduced_days() -> Vec<String> {
    weekday_names(Calendar::default().reduced_days())
}

fn parse_weekdays(names: &[String]) -> Result<Vec<Weekday>, DecodeError> {
    names.iter().map(|name| parse_weekday(name)).collect()
}

const fn default_quota() -> u32 {
    DEFAULT_VACATION_QUOTA
}

const fn weekday_name(day: Weekday) -> &'static str {
    match day {
        Weekday::Mon => "monday",
        Weekday::Tue => "tuesday",
        Weekday::Wed => "wednesday",
        Weekday::Thu => "thursday",
        Weekday::Fri => "friday",
        Weekday::Sat => "saturday",
        Weekday::Sun => "sunday",
    }
}

fn parse_weekday(name: &str) -> Result<Weekday, DecodeError> {
    WEEK.into_iter()
        .find(|day| weekday_name(*day) == name)
        .ok_or_else(|| DecodeError::Weekday(name.to_owned()))
}

impl TryFrom<MarkDto> for Mark {
    type Error = DomainError;

    fn try_from(dto: MarkDto) -> Result<Self, Self::Error> {
        Ok(Self {
            id: MarkId(dto.id),
            kind: dto.kind.into(),
            range: DateRange::new(parse_date(&dto.from)?, parse_date(&dto.to)?)?,
            description: dto.description,
        })
    }
}

impl From<&Mark> for MarkDto {
    fn from(mark: &Mark) -> Self {
        Self {
            id: mark.id.0,
            kind: mark.kind.into(),
            from: mark.range.from().to_string(),
            to: mark.range.to().to_string(),
            description: mark.description.clone(),
        }
    }
}

impl TryFrom<FileDto> for Calendar {
    type Error = DecodeError;

    fn try_from(dto: FileDto) -> Result<Self, Self::Error> {
        if dto.version != VERSION {
            return Err(DecodeError::Version(dto.version));
        }
        let weekend = parse_weekdays(&dto.weekend)?;
        let reduced_days = parse_weekdays(&dto.reduced_days)?;
        let hours = Shift::ALL
            .into_iter()
            .try_fold(Hours::default(), |hours, shift| {
                hours.with(shift, dto.hours.of(shift))
            })?;
        let base = Self::default()
            .with_weekend(&weekend)
            .with_reduced_days(&reduced_days)
            .with_hours(hours);
        Ok(dto.years.into_iter().try_fold(base, |calendar, year| {
            let calendar = calendar
                .with_year(year.year)?
                .with_quota(year.year, year.vacation_quota)?;
            year.marks.into_iter().try_fold(calendar, |calendar, mark| {
                calendar.restore_mark(mark.try_into()?)
            })
        })?)
    }
}

impl From<&Calendar> for FileDto {
    fn from(calendar: &Calendar) -> Self {
        Self {
            version: VERSION,
            weekend: weekday_names(calendar.weekend()),
            reduced_days: weekday_names(calendar.reduced_days()),
            hours: calendar.hours().into(),
            years: calendar
                .years()
                .iter()
                .map(|year| YearDto {
                    year: year.number(),
                    vacation_quota: year.vacation_quota(),
                    marks: year.marks().iter().map(MarkDto::from).collect(),
                })
                .collect(),
        }
    }
}

/// Lee un calendario de un texto JSON, comprobando todas las reglas.
pub fn decode(text: &str) -> Result<Calendar, DecodeError> {
    serde_json::from_str::<FileDto>(text)?.try_into()
}

/// Escribe un calendario como JSON indentado, terminado en salto de línea.
pub fn encode(calendar: &Calendar) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&FileDto::from(calendar)).map(|json| json + "\n")
}

/// Carga el calendario de `path`, o el calendario vacío por defecto si el fichero no existe.
pub fn load(path: &Path) -> Result<Calendar, StoreError> {
    match fs::read_to_string(path) {
        Ok(text) => decode(&text).map_err(|source| StoreError::Decode {
            path: path.to_owned(),
            source,
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Calendar::default()),
        Err(source) => Err(StoreError::Read {
            path: path.to_owned(),
            source,
        }),
    }
}

/// Guarda el calendario en `path`. Escribe un fichero temporal al lado y lo renombra, así un
/// fallo nunca deja un calendario a medio escribir (RN-13).
pub fn save(path: &Path, calendar: &Calendar) -> Result<(), StoreError> {
    let temporary = PathBuf::from(
        [path.as_os_str(), ".tmp".as_ref()]
            .into_iter()
            .collect::<OsString>(),
    );
    encode(calendar)
        .map_err(io::Error::other)
        .and_then(|json| fs::write(&temporary, json))
        .and_then(|()| fs::rename(&temporary, path))
        .map_err(|source| StoreError::Write {
            path: path.to_owned(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::{date, marked};

    const EXAMPLE: &str = r#"{
  "version": 1,
  "weekend": ["saturday", "sunday"],
  "hours": { "laborable": 8.5, "reducido": 6.0, "laborable_verano": 8.5, "reducido_verano": 6.0 },
  "years": [
    {
      "year": 2026,
      "vacation_quota": 25,
      "marks": [
        { "id": 1, "kind": "festivo", "from": "2026-03-03", "to": "2026-03-03", "description": "Fiesta local" },
        { "id": 2, "kind": "vacaciones", "from": "2026-06-05", "to": "2026-06-12" }
      ]
    }
  ]
}"#;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("calendario-{}-{name}.json", std::process::id()))
    }

    #[test]
    fn rf_11_reads_the_example_of_the_requirements() {
        let calendar = decode(EXAMPLE).expect("valid example");
        let marks: Vec<Mark> = calendar.marks().cloned().collect();
        assert_eq!(calendar.weekend(), [Weekday::Sat, Weekday::Sun]);
        assert_eq!(calendar.reduced_days(), [Weekday::Fri]);
        assert_eq!(calendar.hours(), Hours::default());
        assert_eq!(
            calendar.year(2026).map(|year| year.vacation_quota()),
            Ok(25)
        );
        assert_eq!(
            marks,
            [
                Mark {
                    id: MarkId(1),
                    kind: MarkKind::Holiday,
                    range: DateRange::day(date(2026, 3, 3)),
                    description: Some("Fiesta local".to_owned()),
                },
                Mark {
                    id: MarkId(2),
                    kind: MarkKind::Vacation,
                    range: DateRange::new(date(2026, 6, 5), date(2026, 6, 12)).expect("range"),
                    description: None,
                },
            ]
        );
    }

    #[test]
    fn rf_11_encoding_and_decoding_gives_the_same_calendar() {
        let calendar = marked(&[
            (MarkKind::Holiday, "2026-03-03", "2026-03-03"),
            (MarkKind::Reduced, "2026-12-24", "2026-12-24"),
        ])
        .with_weekend(&[Weekday::Fri, Weekday::Sat])
        .with_reduced_days(&[Weekday::Mon, Weekday::Thu])
        .with_shift_hours(Shift::SummerReduced, 5.5)
        .and_then(|calendar| calendar.with_quota(2026, 23))
        .and_then(|calendar| calendar.with_year(2027))
        .expect("calendar");
        let json = encode(&calendar).expect("encoded");
        assert_eq!(decode(&json).ok(), Some(calendar));
        assert!(json.ends_with("}\n"), "{json}");
        assert!(
            json.contains("\"reduced_days\": [\n    \"monday\",\n    \"thursday\"\n  ]"),
            "{json}"
        );
        assert!(!json.contains("description"), "{json}");
    }

    #[test]
    fn rf_11_missing_fields_take_the_defaults() {
        let calendar = decode(r#"{ "version": 1, "years": [{ "year": 2026 }] }"#).expect("valid");
        assert_eq!(calendar.weekend(), [Weekday::Sat, Weekday::Sun]);
        assert_eq!(calendar.hours(), Hours::default());
        assert_eq!(
            calendar.year(2026).map(|year| year.vacation_quota()),
            Ok(25)
        );
    }

    #[test]
    fn rn_01_rn_02_rn_03_invalid_files_are_rejected() {
        let mark = |kind: &str, from: &str, to: &str| {
            format!(
                r#"{{ "version": 1, "years": [{{ "year": 2026, "marks": [{{ "id": 1, "kind": "{kind}", "from": "{from}", "to": "{to}" }}] }}] }}"#
            )
        };
        [
            ("{".to_owned(), "EOF while parsing an object at line 1 column 1"),
            (r#"{ "version": 2 }"#.to_owned(), "versión 2 no soportada; se esperaba la 1"),
            (
                r#"{ "version": 1, "weekend": ["sabado"] }"#.to_owned(),
                "día de la semana no válido: «sabado»; usa monday, tuesday… sunday",
            ),
            (
                r#"{ "version": 1, "reduced_days": ["viernes"] }"#.to_owned(),
                "día de la semana no válido: «viernes»; usa monday, tuesday… sunday",
            ),
            (
                r#"{ "version": 1, "hours": { "laborable": 30, "reducido": 6, "laborable_verano": 8.5, "reducido_verano": 6 } }"#
                    .to_owned(),
                "horas no válidas: 30; deben estar entre 0 y 24",
            ),
            (
                r#"{ "version": 1, "years": [{ "year": 2026 }, { "year": 2026 }] }"#.to_owned(),
                "el año 2026 ya existe en el calendario",
            ),
            (
                mark("festivo", "2026-02-30", "2026-02-30"),
                "fecha no válida: «2026-02-30»; usa el formato AAAA-MM-DD con una fecha que exista",
            ),
            (
                mark("festivo", "2026-03-05", "2026-03-03"),
                "rango no válido: 2026-03-05 es posterior a 2026-03-03",
            ),
            (
                mark("festivo", "2027-03-03", "2027-03-03"),
                "el año 2027 no existe en el calendario; añádelo con «calendario add-year 2027»",
            ),
            (
                mark("laborable", "2026-03-03", "2026-03-03"),
                "unknown variant `laborable`, expected one of `reducido`, `festivo`, `vacaciones` at line 1 column 82",
            ),
        ]
        .into_iter()
        .for_each(|(text, message)| {
            assert_eq!(
                decode(&text).map_err(|error| error.to_string()).err().as_deref(),
                Some(message),
                "{text}"
            );
        });
    }

    #[test]
    fn rf_11_saves_and_loads_a_file() {
        let path = temp_path("roundtrip");
        let calendar = marked(&[(MarkKind::Holiday, "2026-03-03", "2026-03-03")]);
        assert!(save(&path, &calendar).is_ok());
        assert_eq!(load(&path).ok(), Some(calendar));
        let leftovers = fs::exists(PathBuf::from(format!("{}.tmp", path.display())));
        assert_eq!(leftovers.ok(), Some(false));
        assert!(fs::remove_file(&path).is_ok());
    }

    #[test]
    fn rf_11_a_missing_file_is_an_empty_calendar() {
        assert_eq!(load(&temp_path("missing")).ok(), Some(Calendar::default()));
    }

    #[test]
    fn store_errors_name_the_file() {
        let directory = std::env::temp_dir();
        let unreadable = load(&directory).map_err(|error| error.to_string());
        assert!(
            unreadable
                .as_ref()
                .err()
                .is_some_and(|message| message.starts_with("no se puede leer «")),
            "{unreadable:?}"
        );
        let path = temp_path("invalid");
        assert!(fs::write(&path, "{ \"version\": 9 }").is_ok());
        let invalid = load(&path).map_err(|error| error.to_string());
        assert_eq!(
            invalid.err(),
            Some(format!(
                "el fichero «{}» no es válido: versión 9 no soportada; se esperaba la 1",
                path.display()
            ))
        );
        assert!(fs::remove_file(&path).is_ok());
        let unwritable = save(
            &directory.join("no-existe").join("x.json"),
            &Calendar::default(),
        )
        .map_err(|error| error.to_string());
        assert!(
            unwritable
                .err()
                .is_some_and(|message| message.starts_with("no se puede escribir «")),
        );
    }
}
